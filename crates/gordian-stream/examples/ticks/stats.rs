// The statistics behind the `ticks` example (W1). Nothing here prints or reads a file; the
// functions take plain data so that `src/tests/ticks.rs` can test them on hand-written input
// and on generated streams without running the example.
//
// A tick is `floor(at_ns / tick_ns)`: the grid starts at the stream's instant zero. This is the
// simplest synchronous clock; a clock that starts at another phase would move some events across
// a tick boundary, which the same-tick shares below are sensitive to (see the report).

use gordian_stream::oracle::{EvidenceRole, IncidentTruth, ObsLabel, StreamTruth, reveal};
use gordian_stream::{HardKind, Stream, Tier};
use gordian_world::physics::{HIGH, SignalText};
use gordian_world::{Observation, Service};
use std::collections::BTreeMap;

pub const MS: u64 = 1_000_000;

/// The tick lengths of `docs/medium-ports.md` section 8, nanoseconds.
pub const TICK_NS: [u64; 3] = [100 * MS, 500 * MS, 2_000 * MS];

/// Whether the first world's public rules call `obs` abnormal. A copy of
/// `gordian_run::stream::arms::rung::is_abnormal` (that crate cannot be a dependency of this
/// one): a counter at or above `HIGH`, a catalogue message other than `CheckHealth`, or a
/// snapshot whose hash differs from the public graph's.
pub fn is_abnormal(obs: &Observation, services: &[Service]) -> bool {
    match obs {
        Observation::Counter { value, .. } => *value >= HIGH,
        Observation::Message { text_id, .. } => {
            matches!(SignalText::from_text_id(*text_id), Some(t) if t != SignalText::CheckHealth)
        }
        Observation::Snapshot {
            service,
            config_hash,
        } => services
            .get(service.index())
            .is_some_and(|s| s.config_hash != *config_hash),
        Observation::Probed { .. } | Observation::Correction { .. } => false,
    }
}

/// The tick an instant falls in.
pub fn tick_of(at_ns: u64, tick_ns: u64) -> u64 {
    at_ns / tick_ns
}

/// One passive observation, reduced to what the tick statistics read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ev {
    pub at_ns: u64,
    /// The service it is about; `None` for a probe or a correction (the stream has none).
    pub node: Option<u32>,
    /// Abnormal by the public rules.
    pub abnormal: bool,
}

/// A histogram of non-negative integers: value to frequency.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hist(pub BTreeMap<u64, u64>);

impl Hist {
    pub fn add(&mut self, value: u64, times: u64) {
        if times > 0 {
            *self.0.entry(value).or_insert(0) += times;
        }
    }

    pub fn merge(&mut self, other: &Hist) {
        for (&v, &n) in &other.0 {
            self.add(v, n);
        }
    }

    pub fn n(&self) -> u64 {
        self.0.values().sum()
    }

    /// Sum of value times frequency.
    pub fn total(&self) -> u64 {
        self.0.iter().map(|(v, n)| v * n).sum()
    }

    pub fn mean(&self) -> f64 {
        self.total() as f64 / self.n().max(1) as f64
    }

    /// Population variance.
    pub fn variance(&self) -> f64 {
        let m = self.mean();
        self.0
            .iter()
            .map(|(&v, &n)| (v as f64 - m).powi(2) * n as f64)
            .sum::<f64>()
            / self.n().max(1) as f64
    }

    /// The smallest value `v` such that at least a fraction `q` of the mass is at or below `v`.
    pub fn quantile(&self, q: f64) -> u64 {
        let n = self.n();
        let need = (q * n as f64).ceil().max(1.0) as u64;
        let mut seen = 0;
        for (&v, &f) in &self.0 {
            seen += f;
            if seen >= need {
                return v;
            }
        }
        self.max()
    }

    pub fn max(&self) -> u64 {
        self.0.keys().next_back().copied().unwrap_or(0)
    }

    /// The share of the mass at or below `v`.
    pub fn share_at_most(&self, v: u64) -> f64 {
        self.0.range(..=v).map(|(_, n)| n).sum::<u64>() as f64 / self.n().max(1) as f64
    }
}

/// The four distributions of events per tick for one tick length.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickHists {
    /// Events per tick, all observations.
    pub all: Hist,
    /// Events per tick, abnormal observations.
    pub abnormal: Hist,
    /// Events per (node, tick) cell, all observations. Empty cells are counted.
    pub node_all: Hist,
    /// Events per (node, tick) cell, abnormal observations.
    pub node_abnormal: Hist,
}

impl TickHists {
    pub fn merge(&mut self, other: &TickHists) {
        self.all.merge(&other.all);
        self.abnormal.merge(&other.abnormal);
        self.node_all.merge(&other.node_all);
        self.node_abnormal.merge(&other.node_abnormal);
    }
}

/// Events per tick and per (node, tick) cell over `[0, duration_ns)`, empty ticks and cells
/// counted. The number of ticks is `ceil(duration_ns / tick_ns)`, or one more than the last
/// event's tick if that is larger.
pub fn tick_hists(events: &[Ev], duration_ns: u64, n_nodes: usize, tick_ns: u64) -> TickHists {
    let last = events.iter().map(|e| tick_of(e.at_ns, tick_ns)).max();
    let n_ticks = duration_ns.div_ceil(tick_ns).max(last.map_or(0, |t| t + 1));
    let mut all: BTreeMap<u64, u64> = BTreeMap::new();
    let mut abn: BTreeMap<u64, u64> = BTreeMap::new();
    let mut cell_all: BTreeMap<(u32, u64), u64> = BTreeMap::new();
    let mut cell_abn: BTreeMap<(u32, u64), u64> = BTreeMap::new();
    for e in events {
        let t = tick_of(e.at_ns, tick_ns);
        *all.entry(t).or_insert(0) += 1;
        if e.abnormal {
            *abn.entry(t).or_insert(0) += 1;
        }
        if let Some(n) = e.node {
            *cell_all.entry((n, t)).or_insert(0) += 1;
            if e.abnormal {
                *cell_abn.entry((n, t)).or_insert(0) += 1;
            }
        }
    }
    let fill = |counts: Vec<u64>, cells: u64| {
        let mut h = Hist::default();
        for c in &counts {
            h.add(*c, 1);
        }
        h.add(0, cells - counts.len() as u64);
        h
    };
    let cells = n_ticks * n_nodes as u64;
    TickHists {
        all: fill(all.values().copied().collect(), n_ticks),
        abnormal: fill(abn.values().copied().collect(), n_ticks),
        node_all: fill(cell_all.values().copied().collect(), cells),
        node_abnormal: fill(cell_abn.values().copied().collect(), cells),
    }
}

/// One observation of an incident, for the lag statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncObs {
    pub at_ns: u64,
    pub node: Option<u32>,
    pub abnormal: bool,
    pub decisive: bool,
}

/// An incident's instants, in nanoseconds, before any tick grid is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncidentInstants {
    /// The incident's first observation (any kind).
    pub first_ns: u64,
    /// Its first abnormal observation, if it ever has one.
    pub first_abnormal_ns: Option<u64>,
    /// The first abnormal observation at the partner service, if the family has a partner and
    /// the partner ever alarms.
    pub partner_alarm_ns: Option<u64>,
}

/// The instants of one incident. `obs` must not be empty.
pub fn incident_instants(obs: &[IncObs], partner: Option<u32>) -> IncidentInstants {
    let first_ns = obs
        .iter()
        .map(|o| o.at_ns)
        .min()
        .expect("an incident has observations");
    let first_abnormal_ns = obs.iter().filter(|o| o.abnormal).map(|o| o.at_ns).min();
    let partner_alarm_ns = partner.and_then(|p| {
        obs.iter()
            .filter(|o| o.abnormal && o.node == Some(p))
            .map(|o| o.at_ns)
            .min()
    });
    IncidentInstants {
        first_ns,
        first_abnormal_ns,
        partner_alarm_ns,
    }
}

/// Ticks from instant `a` to instant `b` on the grid. `b` must not precede `a`.
pub fn lag_ticks(a_ns: u64, b_ns: u64, tick_ns: u64) -> u64 {
    tick_of(b_ns, tick_ns) - tick_of(a_ns, tick_ns)
}

/// The decisive observations of an incident that fall in the tick of its first observation:
/// `(in the first tick, all decisive)`.
pub fn decisive_in_first_tick(obs: &[IncObs], first_ns: u64, tick_ns: u64) -> (u64, u64) {
    let t0 = tick_of(first_ns, tick_ns);
    let total = obs.iter().filter(|o| o.decisive).count() as u64;
    let same = obs
        .iter()
        .filter(|o| o.decisive && tick_of(o.at_ns, tick_ns) == t0)
        .count() as u64;
    (same, total)
}

/// A hard family and mode, the row key of the lag tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group {
    pub family: HardKind,
    /// `Some(true)` Contradict, `Some(false)` Mimic, `None` for the leak (no phase-1 mode).
    pub contradict: Option<bool>,
}

impl Group {
    pub fn mode_name(&self) -> &'static str {
        match self.contradict {
            Some(true) => "contradict",
            Some(false) => "mimic",
            None => "none",
        }
    }
}

/// Lag samples for one group and tick length. A missing instant is counted, not dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LagSamples {
    pub incidents: u64,
    /// First observation to first abnormal observation.
    pub to_abnormal: Hist,
    pub no_abnormal: u64,
    /// First observation to the partner's first alarm.
    pub to_partner: Hist,
    pub no_partner_alarm: u64,
    /// First abnormal observation to the partner's first alarm.
    pub abnormal_to_partner: Hist,
    /// Sum over incidents of the share of decisive evidence in the first observation's tick.
    pub decisive_share_sum: f64,
    /// Incidents with at least one decisive observation in that tick.
    pub decisive_any: u64,
    /// Incidents with all their decisive observations in that tick.
    pub decisive_all: u64,
    /// Incidents with at least one decisive observation (the denominator of the three above).
    pub with_decisive: u64,
}

/// Add one incident to `s` at one tick length.
pub fn add_incident(s: &mut LagSamples, obs: &[IncObs], partner: Option<u32>, tick_ns: u64) {
    let i = incident_instants(obs, partner);
    s.incidents += 1;
    match i.first_abnormal_ns {
        Some(a) => s.to_abnormal.add(lag_ticks(i.first_ns, a, tick_ns), 1),
        None => s.no_abnormal += 1,
    }
    if partner.is_some() {
        match i.partner_alarm_ns {
            Some(p) => {
                s.to_partner.add(lag_ticks(i.first_ns, p, tick_ns), 1);
                // The partner's alarm is itself abnormal, so it is never before the first one.
                if let Some(a) = i.first_abnormal_ns {
                    s.abnormal_to_partner.add(lag_ticks(a, p, tick_ns), 1);
                }
            }
            None => s.no_partner_alarm += 1,
        }
    }
    let (same, total) = decisive_in_first_tick(obs, i.first_ns, tick_ns);
    if total > 0 {
        s.with_decisive += 1;
        s.decisive_share_sum += same as f64 / total as f64;
        s.decisive_any += u64::from(same > 0);
        s.decisive_all += u64::from(same == total);
    }
}

/// The same lags in whole milliseconds, tick-free: one sample per incident, for context.
pub fn add_incident_ms(ms: &mut MsSamples, obs: &[IncObs], partner: Option<u32>) {
    let i = incident_instants(obs, partner);
    if let Some(a) = i.first_abnormal_ns {
        ms.to_abnormal.add((a - i.first_ns) / MS, 1);
    }
    if let Some(p) = i.partner_alarm_ns {
        ms.to_partner.add((p - i.first_ns) / MS, 1);
        if let Some(a) = i.first_abnormal_ns {
            ms.abnormal_to_partner.add((p - a) / MS, 1);
        }
    }
}

/// Lag samples in milliseconds, independent of any tick grid.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MsSamples {
    pub to_abnormal: Hist,
    pub to_partner: Hist,
    pub abnormal_to_partner: Hist,
}

/// Reduce a stream's passive observations. The abnormal flag is the public rules' verdict.
pub fn stream_events(stream: &Stream) -> Vec<Ev> {
    let services = stream.public_info().services;
    stream
        .events()
        .iter()
        .map(|(at, obs)| Ev {
            at_ns: at.0,
            node: match obs {
                Observation::Counter { service, .. }
                | Observation::Message { service, .. }
                | Observation::Snapshot { service, .. } => Some(service.index() as u32),
                Observation::Probed { .. } | Observation::Correction { .. } => None,
            },
            abnormal: is_abnormal(obs, &services),
        })
        .collect()
}

/// Every observation of incident `inc`, in stream order, with its role. `evs` is
/// [`stream_events`] of the same stream.
pub fn incident_obs(evs: &[Ev], truth: &StreamTruth, inc: &IncidentTruth) -> Vec<IncObs> {
    inc.observations
        .iter()
        .map(|id| {
            let e = evs[id.0 as usize];
            let decisive = matches!(
                truth.labels[id.0 as usize],
                ObsLabel::Incident {
                    role: EvidenceRole::Decisive,
                    ..
                }
            );
            IncObs {
                at_ns: e.at_ns,
                node: e.node,
                abnormal: e.abnormal,
                decisive,
            }
        })
        .collect()
}

/// The group of a hard incident, or `None` for a plain incident or a decoy.
pub fn group_of(inc: &IncidentTruth) -> Option<Group> {
    if inc.tier != Tier::Hard {
        return None;
    }
    Some(Group {
        family: inc.shape.hard_kind?,
        contradict: inc.shape.contradicts_early,
    })
}

/// Everything the example accumulates over streams, per tick length.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    pub streams: u64,
    pub events: u64,
    pub abnormal_events: u64,
    pub duration_ns: u64,
    pub hists: Vec<TickHists>,
    pub lags: Vec<BTreeMap<Group, LagSamples>>,
    pub ms: BTreeMap<Group, MsSamples>,
}

impl Totals {
    pub fn new(tick_lengths: &[u64]) -> Totals {
        Totals {
            hists: vec![TickHists::default(); tick_lengths.len()],
            lags: vec![BTreeMap::new(); tick_lengths.len()],
            ..Totals::default()
        }
    }

    /// Add one stream.
    pub fn add_stream(&mut self, stream: &Stream, tick_lengths: &[u64]) {
        let truth = reveal(stream);
        let evs = stream_events(stream);
        self.streams += 1;
        self.events += evs.len() as u64;
        self.abnormal_events += evs.iter().filter(|e| e.abnormal).count() as u64;
        self.duration_ns += truth.duration_ns;
        for (k, &tick_ns) in tick_lengths.iter().enumerate() {
            let h = tick_hists(&evs, truth.duration_ns, truth.services.len(), tick_ns);
            self.hists[k].merge(&h);
        }
        for inc in &truth.incidents {
            let Some(g) = group_of(inc) else { continue };
            let obs = incident_obs(&evs, &truth, inc);
            let partner = inc.shape.other.map(|s| s.index() as u32);
            for (k, &tick_ns) in tick_lengths.iter().enumerate() {
                add_incident(self.lags[k].entry(g).or_default(), &obs, partner, tick_ns);
            }
            add_incident_ms(self.ms.entry(g).or_default(), &obs, partner);
        }
    }
}
