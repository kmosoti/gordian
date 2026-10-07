//! The public rules as pure functions of rows. Nothing here reads a table: [`super::program`] reads
//! what a rule needs from the relations, hands it over as plain values, and applies the answer.
//!
//! Each function states one rule of the rung's noticing, the later re-anchor, the ramp or the split,
//! in terms of an anomaly's attached observations ([`Row`]: when it was emitted, about which
//! service, in the order the anomaly took them) and its timing ([`Timing`]). They are an
//! independent re-expression of the hand-written rules (`noticer.rs`, `noticer_reanchor.rs`,
//! `noticer_ramp.rs`, `noticer_split.rs`), held to them by `tests/stream_dataflow.rs`; none of
//! those functions is called from here.

use crate::stream::arms::noticer_reanchor::Isolation;
use std::cmp::Reverse;

/// One attached observation as the rules see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// When it was emitted, nanoseconds.
    pub at: u64,
    /// The service it is about.
    pub svc: u32,
}

/// An anomaly's three running instants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// When the last observation was attached.
    pub last_abnormal: u64,
    /// When the last observation at the anomaly's own site was attached.
    pub last_site: u64,
    /// When the burst at the site that is current began.
    pub burst_open: u64,
}

impl Timing {
    /// The timing of an anomaly whose first observation is at `at`, at its own site.
    pub fn first(at: u64) -> Self {
        Self {
            last_abnormal: at,
            last_site: at,
            burst_open: at,
        }
    }

    /// The timing after one more observation about `svc` at `at` is attached to an anomaly sited
    /// at `site`. An observation at the site opens a new burst when the site had been silent for
    /// `burst_gap_ns` (or it is the anomaly's first); one elsewhere moves only the last instant.
    pub fn attach(self, at: u64, svc: u32, site: u32, burst_gap_ns: u64) -> Self {
        let mut t = self;
        t.last_abnormal = at;
        if svc == site {
            if at >= t.last_site.saturating_add(burst_gap_ns) {
                t.burst_open = at;
            }
            t.last_site = at;
        }
        t
    }

    /// The timing after the anchor moved to the first of `rows` (the attached observations that
    /// are left, the new anchor first, whose service is `site`): the last observation at the site
    /// among them, the burst opened at the anchor, the last instant unchanged.
    pub fn after_move(self, rows: &[Row], site: u32, anchor_at: u64) -> Self {
        Self {
            last_abnormal: self.last_abnormal,
            last_site: rows
                .iter()
                .rev()
                .find(|r| r.svc == site)
                .map_or(anchor_at, |r| r.at),
            burst_open: anchor_at,
        }
    }

    /// The timing recomputed from scratch over `rows` (an anomaly's observations after some left
    /// it, or the ones that formed a new anomaly): the last instant is the last row's, a burst
    /// opens at each row at the site that follows a silence of `burst_gap_ns` there.
    pub fn retime(rows: &[Row], site: u32, anchor_at: u64, burst_gap_ns: u64) -> Self {
        let mut last_site: Option<u64> = None;
        let mut open = anchor_at;
        for r in rows {
            if r.svc == site {
                if last_site.is_none_or(|l| r.at >= l.saturating_add(burst_gap_ns)) {
                    open = r.at;
                }
                last_site = Some(r.at);
            }
        }
        Self {
            last_abnormal: rows.last().map_or(anchor_at, |r| r.at),
            last_site: last_site.unwrap_or(anchor_at),
            burst_open: open,
        }
    }
}

// ---- attaching an abnormal observation ----------------------------------------------------------

/// Whether an anomaly whose last observation at its site was at `last_site` is still speaking at
/// `at`: within two burst gaps.
pub fn speaking(at: u64, last_site: u64, burst_gap_ns: u64) -> bool {
    at <= last_site.saturating_add(burst_gap_ns.saturating_mul(2))
}

/// Whether `at` falls in the burst window that began at `burst_open` at an upstream site.
pub fn propagating(at: u64, burst_open: u64, burst_ns: u64) -> bool {
    at >= burst_open && at <= burst_open.saturating_add(burst_ns)
}

/// The order among propagation candidates: the burst that began most recently, the earlier id on
/// a tie. Larger is better.
pub fn propagation_rank(burst_open: u64, id: u32) -> (u64, Reverse<u32>) {
    (burst_open, Reverse(id))
}

// ---- the rung's candidates ----------------------------------------------------------------------

/// The z-score's constants: the public graph's size and the rung's prior and window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreParams {
    /// How many services the public graph has.
    pub services: usize,
    /// The weight of the prior, nanoseconds of stream.
    pub prior_ns: u64,
    /// The prior rate of abnormal observations per service, milli-hertz.
    pub prior_mhz: u64,
    /// The window over which a candidate's abnormal observations are counted, nanoseconds.
    pub window_ns: u64,
}

/// The expected abnormal observations per service per second, learned from `abnormal_seen`
/// observations in `now` of stream, with the prior. Basic IEEE arithmetic, in the order the rung's
/// scorer writes it, so that it replays bit for bit.
pub fn baseline_hz(p: &ScoreParams, abnormal_seen: u64, now: u64) -> f64 {
    let services = p.services.max(1) as f64;
    let prior_s = p.prior_ns as f64 / 1e9;
    let prior_hz = p.prior_mhz as f64 / 1000.0;
    let t_s = now as f64 / 1e9;
    (abnormal_seen as f64 + prior_hz * services * prior_s) / (services * (t_s + prior_s))
}

/// The score of an anomaly with `n` observations attached in the last window.
pub fn z_score(p: &ScoreParams, abnormal_seen: u64, now: u64, n: u64) -> f64 {
    let mu = baseline_hz(p, abnormal_seen, now) * (p.window_ns as f64 / 1e9);
    (n as f64 - mu) / (mu + 1.0).sqrt()
}

/// Where an anomaly's anchor moves when it is noticed: the attached observation with the most
/// attached observations in the `cluster_ns` that follow it, contiguously (the earliest on a tie).
/// Index 0 means the anchor stays.
pub fn densest_start(rows: &[Row], cluster_ns: u64) -> usize {
    let mut chosen = 0;
    let mut best = 0;
    for k in 0..rows.len() {
        let until = rows[k].at.saturating_add(cluster_ns);
        let d = rows[k..].iter().take_while(|r| r.at <= until).count();
        if d > best {
            best = d;
            chosen = k;
        }
    }
    chosen
}

// ---- the later re-anchor ------------------------------------------------------------------------

/// The index the anchor moves to under the later re-anchor, if it applies: the anchor is isolated
/// (no other row strictly before its instant plus `gap_ns`, of the anchor's service unless
/// `isolation` is `Any`) and a later row, strictly after the anchor, begins a burst of at least
/// `min_burst` rows in the `burst_ns` from it.
pub fn later_burst_start(
    rows: &[Row],
    gap_ns: u64,
    min_burst: u32,
    burst_ns: u64,
    isolation: Isolation,
) -> Option<usize> {
    let first = *rows.first()?;
    let near = rows[1..].iter().any(|r| {
        r.at < first.at.saturating_add(gap_ns)
            && (isolation == Isolation::Any || r.svc == first.svc)
    });
    if near {
        return None;
    }
    (1..rows.len()).find(|&k| {
        let until = rows[k].at.saturating_add(burst_ns);
        rows[k].at > first.at
            && rows[k..].iter().take_while(|r| r.at <= until).count() >= min_burst as usize
    })
}

// ---- the split ----------------------------------------------------------------------------------

/// The rows (indices) a split moves out of an anomaly sited at `site` at `now`, and the instant
/// after which the answer would change with nothing else changing.
///
/// A *cluster* is a run of rows each at most `burst_ns` after the one before; a *burst* is a
/// cluster of at least `min_burst` rows. A later cluster with at least `min_burst` rows about
/// services other than `site` splits when it is complete (`burst_ns` has passed since its last
/// row), an earlier burst exists, and the silence from the latest earlier burst's last row to the
/// first foreign row is more than `gap_ns`; the foreign rows of that cluster move. The second
/// value is the earliest instant at which a cluster that satisfies everything but completeness
/// becomes complete: the rule is a function of the rows and of `now` only through completeness,
/// so an anomaly whose rows did not change need not be examined before it.
pub fn split_rule(
    rows: &[Row],
    site: u32,
    gap_ns: u64,
    min_burst: u32,
    burst_ns: u64,
    now: u64,
) -> (Option<Vec<usize>>, Option<u64>) {
    let min_burst = min_burst as usize;
    let mut clusters: Vec<(usize, usize)> = Vec::new();
    for i in 0..rows.len() {
        match clusters.last_mut() {
            Some((_, end)) if rows[i].at <= rows[i - 1].at.saturating_add(burst_ns) => *end = i + 1,
            _ => clusters.push((i, i + 1)),
        }
    }
    let mut due: Option<u64> = None;
    for (m, &(from, to)) in clusters.iter().enumerate().skip(1) {
        let foreign: Vec<usize> = (from..to).filter(|&j| rows[j].svc != site).collect();
        let Some(&first_foreign) = foreign.first() else {
            continue;
        };
        if foreign.len() < min_burst {
            continue;
        }
        let Some(&(_, earlier_end)) = clusters[..m].iter().rev().find(|(f, t)| t - f >= min_burst)
        else {
            continue;
        };
        let silence = rows[first_foreign]
            .at
            .saturating_sub(rows[earlier_end - 1].at);
        if silence <= gap_ns {
            continue;
        }
        let ready = rows[to - 1].at.saturating_add(burst_ns);
        if now <= ready {
            due = Some(due.map_or(ready, |d| d.min(ready)));
            continue;
        }
        return (Some(foreign), None);
    }
    (None, due)
}

// ---- the ramp's chains --------------------------------------------------------------------------

/// A chain of readings of one counter at one service, as the ramp rule sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainRow {
    /// The value of the chain's first reading.
    pub first_value: u64,
    /// The value of the last reading accepted.
    pub level: u64,
    /// How many readings it holds.
    pub count: u32,
    /// When the last was emitted.
    pub last_at: u64,
    /// Whether it has crossed and been linked to an anomaly.
    pub crossed: bool,
}

/// Whether a reading of `value` continues a chain whose level is `level`: at most `max_step` above
/// it and at most `max_drop` below, an equal value included.
pub fn continues(level: u64, value: u64, max_drop: u32, max_step: u32) -> bool {
    if value >= level {
        value - level <= u64::from(max_step)
    } else {
        level - value <= u64::from(max_drop)
    }
}

/// Whether a chain has outlived its silence: a reading at `at` is more than `gap_ns` after its
/// last.
pub fn expired(c: &ChainRow, at: u64, gap_ns: u64) -> bool {
    at > c.last_at.saturating_add(gap_ns)
}

/// The chain (index into `chains`, in order of creation) a reading of `value` continues: the one
/// with the most readings, the earliest created on a tie. `None` when it continues none.
pub fn continued_chain(
    chains: &[ChainRow],
    value: u64,
    max_drop: u32,
    max_step: u32,
) -> Option<usize> {
    let mut pick: Option<usize> = None;
    for (i, c) in chains.iter().enumerate() {
        if continues(c.level, value, max_drop, max_step)
            && pick.is_none_or(|p| c.count > chains[p].count)
        {
            pick = Some(i);
        }
    }
    pick
}

/// The chain to drop when a reading that continues none finds `max_chains` alive: an un-crossed
/// one with the fewest readings, the oldest last reading breaking a tie, the earliest created
/// after that.
pub fn eviction_victim(chains: &[ChainRow], max_chains: usize) -> Option<usize> {
    if chains.len() < max_chains {
        return None;
    }
    (0..chains.len()).min_by_key(|&j| (chains[j].crossed, chains[j].count, chains[j].last_at))
}

/// Whether a chain is a ramp: enough readings and a level far enough above its first reading.
pub fn is_ramp(c: &ChainRow, min_readings: u32, min_rise: u32) -> bool {
    let rise = i128::from(c.level) - i128::from(c.first_value);
    c.count >= min_readings && rise >= i128::from(min_rise)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(at_ms: u64, svc: u32) -> Row {
        Row {
            at: at_ms * 1_000_000,
            svc,
        }
    }

    const MS: u64 = 1_000_000;

    #[test]
    fn timing_follows_the_attach_rule() {
        let t = Timing::first(100);
        let t = t.attach(150, 7, 0, 2_000);
        assert_eq!(
            (t.last_abnormal, t.last_site, t.burst_open),
            (150, 100, 100)
        );
        let t = t.attach(300, 0, 0, 2_000);
        assert_eq!((t.last_site, t.burst_open), (300, 100));
        let t = t.attach(2_400, 0, 0, 2_000);
        assert_eq!((t.last_site, t.burst_open), (2_400, 2_400));
    }

    #[test]
    fn a_move_opens_the_burst_at_the_anchor_and_a_retime_at_the_latest_burst() {
        let rows = [r(0, 1), r(100, 1), r(5_000, 1), r(5_100, 2)];
        let t = Timing::first(0).attach(100 * MS, 1, 1, 2_000 * MS);
        let moved = t.after_move(&rows, 1, 0);
        assert_eq!((moved.last_site, moved.burst_open), (5_000 * MS, 0));
        let re = Timing::retime(&rows, 1, 0, 2_000 * MS);
        assert_eq!((re.last_site, re.burst_open), (5_000 * MS, 5_000 * MS));
        assert_eq!(re.last_abnormal, 5_100 * MS);
    }

    #[test]
    fn speaking_and_propagating_boundaries() {
        assert!(speaking(4_000, 0, 2_000) && !speaking(4_001, 0, 2_000));
        assert!(propagating(400, 0, 400) && !propagating(401, 0, 400) && !propagating(9, 10, 400));
        assert!(propagation_rank(5, 9) > propagation_rank(4, 1));
        assert!(propagation_rank(5, 1) > propagation_rank(5, 2));
    }

    #[test]
    fn the_score_is_the_rungs_formula() {
        let p = ScoreParams {
            services: 10,
            prior_ns: 30_000_000_000,
            prior_mhz: 100,
            window_ns: 8_000_000_000,
        };
        let mu: f64 = (0.0 + 0.1 * 10.0 * 30.0) / (10.0 * (0.0 + 30.0)) * 8.0;
        assert_eq!(z_score(&p, 0, 0, 5), (5.0 - mu) / (mu + 1.0).sqrt());
    }

    #[test]
    fn the_densest_burst_is_the_earliest_of_the_densest() {
        // a stray, then three in 300 ms: the burst is where the stray's window is outgrown.
        let rows = [r(0, 1), r(500, 1), r(600, 2), r(700, 1)];
        assert_eq!(densest_start(&rows, 400 * MS), 1);
        let tie = [r(0, 1), r(100, 1), r(5_000, 1), r(5_100, 1)];
        assert_eq!(densest_start(&tie, 400 * MS), 0);
        assert_eq!(densest_start(&[], 400 * MS), 0);
    }

    #[test]
    fn the_later_burst_needs_an_isolated_anchor() {
        let rows = [r(0, 1), r(500, 1), r(600, 2), r(700, 1)];
        assert_eq!(
            later_burst_start(&rows, 20 * MS, 2, 400 * MS, Isolation::Site),
            Some(1)
        );
        let near = [r(0, 1), r(10, 1), r(500, 1), r(600, 2)];
        assert_eq!(
            later_burst_start(&near, 20 * MS, 2, 400 * MS, Isolation::Site),
            None
        );
        let other = [r(0, 1), r(10, 2), r(500, 1), r(600, 2)];
        assert_eq!(
            later_burst_start(&other, 20 * MS, 2, 400 * MS, Isolation::Site),
            Some(2)
        );
        assert_eq!(
            later_burst_start(&other, 20 * MS, 2, 400 * MS, Isolation::Any),
            None
        );
    }

    #[test]
    fn a_split_waits_for_the_later_burst_to_complete_and_says_when() {
        let site = 0;
        let rows = [
            r(0, 0),
            r(100, 0),
            r(200, 0),
            r(10_000, 3),
            r(10_100, 4),
            r(10_200, 3),
        ];
        let (picks, due) = split_rule(&rows, site, 3_000 * MS, 3, 400 * MS, 10_300 * MS);
        assert_eq!(picks, None);
        assert_eq!(due, Some(10_600 * MS));
        let (picks, due) = split_rule(&rows, site, 3_000 * MS, 3, 400 * MS, 10_601 * MS);
        assert_eq!(picks, Some(vec![3, 4, 5]));
        assert_eq!(due, None);
        // closer than the gap: nothing, and nothing to wait for.
        let (picks, due) = split_rule(&rows, site, 9_900 * MS, 3, 400 * MS, 99_000 * MS);
        assert_eq!((picks, due), (None, None));
        // fewer foreign rows than a burst.
        let (picks, due) = split_rule(&rows, site, 3_000 * MS, 4, 400 * MS, 99_000 * MS);
        assert_eq!((picks, due), (None, None));
    }

    #[test]
    fn the_boundaries_of_the_split_the_retime_and_the_later_burst_are_exact() {
        let rows = [
            r(0, 0),
            r(100, 0),
            r(200, 0),
            r(10_000, 3),
            r(10_100, 4),
            r(10_200, 3),
        ];
        // The silence from the earlier burst's last row to the first foreign row is 9,800 ms: a gap
        // of exactly that does not split (it must be exceeded), one ms less does.
        assert_eq!(
            split_rule(&rows, 0, 9_800 * MS, 3, 400 * MS, 99_000 * MS),
            (None, None)
        );
        assert_eq!(
            split_rule(&rows, 0, 9_799 * MS, 3, 400 * MS, 99_000 * MS).0,
            Some(vec![3, 4, 5])
        );
        // The later burst completes 400 ms after its last row: at that instant it is not complete,
        // one ns later it is.
        assert_eq!(
            split_rule(&rows, 0, 3_000 * MS, 3, 400 * MS, 10_600 * MS),
            (None, Some(10_600 * MS))
        );
        assert_eq!(
            split_rule(&rows, 0, 3_000 * MS, 3, 400 * MS, 10_600 * MS + 1).0,
            Some(vec![3, 4, 5])
        );
        // A burst at the site opens again exactly one gap after the last row there.
        let at_gap = [r(0, 1), r(2_000, 1)];
        let t = Timing::retime(&at_gap, 1, 0, 2_000 * MS);
        assert_eq!(t.burst_open, 2_000 * MS);
        let inside = [r(0, 1), r(1_999, 1)];
        assert_eq!(Timing::retime(&inside, 1, 0, 2_000 * MS).burst_open, 0);
        // A burst "begins after the anchor": a row at the anchor's own instant does not.
        let same = [r(0, 1), r(0, 2), r(100, 2)];
        assert_eq!(
            later_burst_start(&same, 20 * MS, 2, 400 * MS, Isolation::Site),
            None
        );
    }

    #[test]
    fn chains_continue_cross_and_are_evicted_as_the_rule_says() {
        assert!(continues(10, 20, 4, 10) && !continues(10, 21, 4, 10));
        assert!(continues(10, 6, 4, 10) && !continues(10, 5, 4, 10));
        let c = |count, level, crossed, last_at| ChainRow {
            first_value: 0,
            level,
            count,
            last_at,
            crossed,
        };
        let chains = [c(2, 10, false, 5), c(3, 12, false, 6), c(3, 11, false, 7)];
        assert_eq!(continued_chain(&chains, 11, 4, 10), Some(1));
        assert_eq!(continued_chain(&chains, 90, 4, 10), None);
        let full = [
            c(3, 0, true, 1),
            c(2, 0, false, 9),
            c(2, 0, false, 4),
            c(2, 0, false, 4),
        ];
        assert_eq!(eviction_victim(&full, 4), Some(2));
        assert_eq!(eviction_victim(&full[..3], 4), None);
        assert!(expired(&c(1, 0, false, 10), 13, 2) && !expired(&c(1, 0, false, 10), 12, 2));
        let ramp = ChainRow {
            first_value: 10,
            level: 25,
            count: 5,
            last_at: 0,
            crossed: false,
        };
        assert!(is_ramp(&ramp, 5, 15) && !is_ramp(&ramp, 6, 15) && !is_ramp(&ramp, 5, 16));
        let falling = ChainRow { level: 3, ..ramp };
        assert!(!is_ramp(&falling, 2, 1));
    }
}
