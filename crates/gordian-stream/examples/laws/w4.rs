//! W4: the reasoner's law from the hidden side (evaluator-side; nothing here reaches an arm).
//!
//! ```text
//! laws --w4 --seed-from 40000 --count 200 --world a --notices E1/sel_reanchor_privileged/notice_events.csv \
//!      --out-dir artifacts/runs/w4/hidden/a-heldout [--threads 3]
//! laws --w4 --replay calls.csv --out-dir DIR            (replay recorded Escalate calls, world A)
//! ```
//!
//! For each stream the example takes the kept re-anchor run's notice records (the anomalies the
//! later re-anchor noticer raised), rebuilds the shared rung's context for each at the ask
//! instants the unit fixes, and asks the **stream crate's own law** what each call would be worth:
//! a real `StreamAction::Escalate` is applied to a `StreamSimulator` of the stream (reasoner budget
//! raised so that nothing is refused) and the hidden fields of the call (`q`, `d`, `h`, `p0`, `p`)
//! are read back through `oracle::calls`. Nothing about the law is re-implemented except the
//! reasoner's *guess distribution* (needed for the answers' distribution in the verification item),
//! which is checked against the stream crate's `p0` for every call. The definitions (policies,
//! contexts, classes) are in `experiments/exploration/w4-sections/definitions.md`.
//!
//! Output files (all in `--out-dir`; hidden artifacts, never committed):
//!
//! - `w4-incidents.csv`: static facts of every incident, the stream's regime targets.
//! - `w4-anomalies.csv`: every notice with its incident, retirement and whether it is the primary.
//! - `w4-decisive.csv`: every decisive observation of hard incidents and decoys with its offsets
//!   and whether the primary anomaly's rung context admits it.
//! - `w4-asks.csv`: the expected value of each ask (policy, context, `delta`).
//! - `w4-answers.csv`, `w4-other.csv`: the answer distributions and the probe patterns that follow.
//! - `w4-regime.csv`: the public checker's open set on each incident's own phase-1 evidence, actual
//!   and rebuilt without the signature shift or without the added edge.
//! - `w4-constants.csv`: measured constants per stream.
//! - `w4-ctx16.csv`: the rung contexts at notice + 16 s, to compare with recorded ledgers.
//! - `w4-checks.csv`: the validation counters (the example exits non-zero if any fails).

use crate::real::{World, params};
use gordian_core::Instant;
use gordian_stream::oracle::{
    self, CallTrace, EvidenceRole, ObsLabel, RegimeDetail, StreamTruth, rebuilt_incident, reveal,
};
use gordian_stream::params::{MS, SEC};
use gordian_stream::{
    Diagnosis, HardKind, ObsId, ObsRef, Question, Stream, StreamAction, StreamHypothesis,
    StreamKind, StreamOutcome, StreamParams, StreamSimulator, Tier, generate,
};
use gordian_world::episode::PublicInfo;
use gordian_world::graph::dependents_mask;
use gordian_world::physics::{HIGH, SignalText, consistent_hypotheses, probe_result};
use gordian_world::{
    FaultKind, Hypothesis, Observation, Probe, ProbeKind, ProbeResult, ServiceId,
};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fixed ask delays after notice, seconds (the brief's).
const DELAYS_S: [u64; 6] = [4, 8, 12, 16, 20, 30];
/// The distractor penalties of item 2.
const DELTAS: [f64; 3] = [0.0, 0.0135, 0.05];
/// B5's delay, the one the verification scenario uses.
const B5_DELAY_S: u64 = 16;
/// The raised reasoner and probe budgets (hidden-side counterfactual asks are never refused).
const BIG: u64 = 1_000_000_000_000_000;
/// The rung's context parameters (`RungConfig::default`): lookback, burst window, cap, retention.
const LOOKBACK_NS: u64 = 2 * SEC;
const BURST_NS: u64 = 400 * MS;
const CTX_CAP: usize = 128;
const RETAIN_NS: u64 = 120 * SEC;
/// Latency of a call: `2 s + 2 ms` per reference.
const LAT_BASE_NS: u64 = 2 * SEC;
const LAT_REF_NS: u64 = 2 * MS;
const T0_NS: u64 = 6 * SEC;

fn usage() -> ExitCode {
    eprintln!(
        "usage: laws --w4 --seed-from N --count K --world a|c --notices FILE --out-dir DIR [--threads T]\n       laws --w4 --replay FILE --out-dir DIR"
    );
    ExitCode::from(2)
}

// ---- input: the kept run's notice records ------------------------------------------------------

#[derive(Debug, Clone)]
struct NoticeRow {
    anomaly: u32,
    anchor: u32,
    site: u32,
    anchor_at: u64,
    at: u64,
    incident: Option<u32>,
    anchor_correct: bool,
    site_correct: bool,
}

#[derive(Default)]
struct SeedNotices {
    notices: Vec<NoticeRow>,
    retire: HashMap<u32, u64>,
}

fn read_notices(path: &PathBuf) -> Result<HashMap<u64, SeedNotices>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty notice file")?.split(',').collect();
    let col = |name: &str| -> Result<usize, String> {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| format!("notice file has no column {name}"))
    };
    let (c_seed, c_event, c_anomaly, c_anchor, c_site) = (
        col("seed")?,
        col("event")?,
        col("anomaly")?,
        col("anchor")?,
        col("site")?,
    );
    let (c_anchor_at, c_at, c_incident, c_ac, c_sc) = (
        col("anchor_at_ns")?,
        col("at_ns")?,
        col("incident")?,
        col("anchor_correct")?,
        col("site_correct")?,
    );
    let mut out: HashMap<u64, SeedNotices> = HashMap::new();
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        let seed: u64 = f[c_seed].parse().map_err(|e| format!("seed: {e}"))?;
        let anomaly: u32 = f[c_anomaly].parse().map_err(|e| format!("anomaly: {e}"))?;
        let entry = out.entry(seed).or_default();
        match f[c_event] {
            "notice" => entry.notices.push(NoticeRow {
                anomaly,
                anchor: f[c_anchor].parse().map_err(|e| format!("anchor: {e}"))?,
                site: f[c_site].parse().map_err(|e| format!("site: {e}"))?,
                anchor_at: f[c_anchor_at].parse().map_err(|e| format!("anchor_at: {e}"))?,
                at: f[c_at].parse().map_err(|e| format!("at: {e}"))?,
                incident: if f[c_incident].is_empty() {
                    None
                } else {
                    Some(f[c_incident].parse().map_err(|e| format!("incident: {e}"))?)
                },
                anchor_correct: f[c_ac] == "true",
                site_correct: f[c_sc] == "true",
            }),
            "retire" => {
                entry
                    .retire
                    .insert(anomaly, f[c_at].parse().map_err(|e| format!("at: {e}"))?);
            }
            other => return Err(format!("unknown notice event {other}")),
        }
    }
    Ok(out)
}

// ---- small helpers -----------------------------------------------------------------------------

fn service_of(o: &Observation) -> Option<ServiceId> {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => Some(*service),
        Observation::Probed { .. } | Observation::Correction { .. } => None,
    }
}

/// As the reasoner's own `service_of`, which also names a probe's target and a correction's site.
fn service_of_any(o: &Observation) -> ServiceId {
    match o {
        Observation::Counter { service, .. }
        | Observation::Message { service, .. }
        | Observation::Snapshot { service, .. } => *service,
        Observation::Probed { probe, .. } => probe.target,
        Observation::Correction { site, .. } => *site,
    }
}

fn kind_abbr(k: FaultKind) -> &'static str {
    match k {
        FaultKind::ResourceExhausted => "RE",
        FaultKind::ConfigDrift => "CD",
        FaultKind::DependencyDown => "DD",
        FaultKind::CredentialExpired => "CE",
        FaultKind::Intermittent => "IN",
    }
}

fn kind_name(k: FaultKind) -> &'static str {
    match k {
        FaultKind::ResourceExhausted => "ResourceExhausted",
        FaultKind::ConfigDrift => "ConfigDrift",
        FaultKind::DependencyDown => "DependencyDown",
        FaultKind::CredentialExpired => "CredentialExpired",
        FaultKind::Intermittent => "Intermittent",
    }
}

fn hard_name(h: HardKind) -> &'static str {
    match h {
        HardKind::Compound => "compound",
        HardKind::Cascade => "cascade",
        HardKind::SplitBrain => "split_brain",
        HardKind::SlowLeak => "slow_leak",
    }
}

fn tier_name(t: Tier) -> &'static str {
    match t {
        Tier::Plain => "plain",
        Tier::Hard => "hard",
        Tier::Decoy => "decoy",
    }
}

fn answer_name(d: Diagnosis) -> String {
    match d {
        None => "none".to_string(),
        Some(StreamHypothesis {
            kind: StreamKind::Known(k),
            ..
        }) => format!("K:{}", kind_abbr(k)),
        Some(StreamHypothesis {
            kind: StreamKind::Hard(h),
            ..
        }) => format!("H:{}", hard_name(h)),
    }
}

fn known(h: Hypothesis) -> Diagnosis {
    h.map(|(kind, site)| StreamHypothesis {
        kind: StreamKind::Known(kind),
        site,
    })
}

fn opt<T: std::fmt::Display>(x: Option<T>) -> String {
    x.map_or_else(String::new, |v| v.to_string())
}

fn latency_ns(refs: usize) -> u64 {
    LAT_BASE_NS + LAT_REF_NS * refs as u64
}

/// The reasoner's guess distribution, re-implemented from the public rules exactly as
/// `reasoner::guess_distribution` reads them (the example checks its `p0` against the stream
/// crate's for every call it asks). A function of the public rules, the focus's service and the
/// context's observations; nothing hidden.
fn guess_distribution(
    public: &PublicInfo,
    focus_site: ServiceId,
    context: &[(Instant, Observation)],
) -> Vec<Diagnosis> {
    let services = &public.services;
    let mut upstream = vec![false; services.len()];
    let mut stack = vec![focus_site];
    while let Some(s) = stack.pop() {
        for d in &services[s.index()].depends_on {
            if !upstream[d.index()] {
                upstream[d.index()] = true;
                stack.push(*d);
            }
        }
    }
    let downstream = dependents_mask(services, focus_site);
    let near = |s: ServiceId| s == focus_site || upstream[s.index()] || downstream[s.index()];
    let mut evidence: Vec<(Instant, Observation)> = context
        .iter()
        .filter(|(_, o)| near(service_of_any(o)))
        .cloned()
        .collect();
    evidence.sort_by_key(|(at, _)| *at);
    let open: Vec<Hypothesis> = consistent_hypotheses(public, &evidence)
        .into_iter()
        .filter(|h| match h {
            None => true,
            Some((_, site)) => *site == focus_site || upstream[site.index()],
        })
        .collect();
    if !open.is_empty() {
        return open.into_iter().map(known).collect();
    }
    let mut fallback: Vec<Diagnosis> = vec![None];
    fallback.extend(
        FaultKind::ALL
            .into_iter()
            .map(|k| known(Some((k, focus_site)))),
    );
    fallback
}

/// The share of `guesses` that falls on `truth`, computed as the stream crate computes `p0`.
fn share_on(guesses: &[Diagnosis], truth: Diagnosis) -> f64 {
    guesses.iter().filter(|g| **g == truth).count() as f64 / guesses.len() as f64
}

// ---- the rung's context ------------------------------------------------------------------------

type Events = [(Instant, Observation)];

/// Whether the rung's context admits an observation about `service` at `at` for an anomaly at
/// `site` (with `region` its public dependents) anchored at `anchor_at`, ignoring the cap and the
/// retention: `rung.rs`, `admits_around`.
fn admits(region: &[bool], site: ServiceId, anchor_at: u64, service: ServiceId, at: u64) -> bool {
    service == site
        || (region.get(service.index()).copied().unwrap_or(false)
            && at <= anchor_at.saturating_add(BURST_NS))
}

/// The references `Rung::context` returns at instant `t`: held (delivered, retained), from
/// `anchor_at - 2 s`, admitted, in id order, capped to the first quarter and the most recent rest.
fn rung_context(
    events: &Events,
    region: &[bool],
    site: ServiceId,
    anchor_at: u64,
    t: u64,
) -> Vec<ObsRef> {
    let lo = anchor_at
        .saturating_sub(LOOKBACK_NS)
        .max(t.saturating_sub(RETAIN_NS));
    let end = events.partition_point(|(at, _)| at.0 <= t);
    let start = events[..end].partition_point(|(at, _)| at.0 < lo);
    let mut refs: Vec<ObsRef> = Vec::new();
    for (i, (at, obs)) in events[start..end].iter().enumerate() {
        if let Some(s) = service_of(obs)
            && admits(region, site, anchor_at, s, at.0)
        {
            refs.push(ObsRef::Passive(ObsId((start + i) as u32)));
        }
    }
    if refs.len() > CTX_CAP {
        let head = CTX_CAP / 4;
        let tail = CTX_CAP - head;
        let mut kept: Vec<ObsRef> = refs[..head].to_vec();
        kept.extend_from_slice(&refs[refs.len() - tail..]);
        refs = kept;
    }
    refs
}

/// The decisive observations of the incident delivered at or before `t`, in id order.
fn min_context(events: &Events, decisive: &[ObsId], t: u64) -> Vec<ObsRef> {
    decisive
        .iter()
        .filter(|o| events[o.0 as usize].0.0 <= t)
        .map(|o| ObsRef::Passive(*o))
        .collect()
}

// ---- asking the law ----------------------------------------------------------------------------

fn escalate(
    sim: &mut StreamSimulator,
    focus: ObsId,
    refs: Vec<ObsRef>,
    at: u64,
) -> Result<CallTrace, String> {
    let _ = sim.observe_until(Instant(at));
    let out = sim.apply(
        StreamAction::Escalate {
            context: refs,
            question: Question::Diagnose { focus },
        },
        Instant(at),
    );
    match out {
        StreamOutcome::Escalated { call, .. } => Ok(oracle::calls(&*sim)[call as usize]),
        other => Err(format!("escalation refused at {at}: {other:?}")),
    }
}

#[derive(Default, Clone)]
struct Checks {
    notices: u64,
    notice_bad: u64,
    stream_equal: u64,
    stream_bad: u64,
    q_calls: u64,
    q_bad: u64,
    p0_calls: u64,
    p0_bad: u64,
    support_calls: u64,
    support_bad: u64,
    hard_ans: u64,
    hard_ans_wrong: u64,
    informed_calls: u64,
    informed_wrong: u64,
    weight_checks: u64,
    weight_bad: u64,
    cap_hits: u64,
    decisive_checked: u64,
    decisive_total_bad: u64,
}

impl Checks {
    fn add(&mut self, o: &Checks) {
        self.notices += o.notices;
        self.notice_bad += o.notice_bad;
        self.stream_equal += o.stream_equal;
        self.stream_bad += o.stream_bad;
        self.q_calls += o.q_calls;
        self.q_bad += o.q_bad;
        self.p0_calls += o.p0_calls;
        self.p0_bad += o.p0_bad;
        self.support_calls += o.support_calls;
        self.support_bad += o.support_bad;
        self.hard_ans += o.hard_ans;
        self.hard_ans_wrong += o.hard_ans_wrong;
        self.informed_calls += o.informed_calls;
        self.informed_wrong += o.informed_wrong;
        self.weight_checks += o.weight_checks;
        self.weight_bad += o.weight_bad;
        self.cap_hits += o.cap_hits;
        self.decisive_checked += o.decisive_checked;
        self.decisive_total_bad += o.decisive_total_bad;
    }

    fn bad(&self) -> u64 {
        self.notice_bad
            + self.stream_bad
            + self.q_bad
            + self.p0_bad
            + self.support_bad
            + self.hard_ans_wrong
            + self.informed_wrong
            + self.weight_bad
            + self.decisive_total_bad
    }

    fn csv(&self) -> String {
        let rows = [
            ("notices_checked", self.notices, self.notice_bad),
            ("streams_equal_across_delta_and_budget", self.stream_equal, self.stream_bad),
            ("q_and_m_equal_recorded", self.q_calls, self.q_bad),
            ("p0_equal_recorded", self.p0_calls, self.p0_bad),
            ("uninformed_answer_in_guess_support", self.support_calls, self.support_bad),
            ("hard_kind_answers_wrong", self.hard_ans, self.hard_ans_wrong),
            ("informed_calls_wrong", self.informed_calls, self.informed_wrong),
            ("answer_weights_sum_and_right_mass", self.weight_checks, self.weight_bad),
            ("decisive_total_equals_labels", self.decisive_checked, self.decisive_total_bad),
        ];
        let mut s = String::from("check,checked,failed\n");
        for (name, n, bad) in rows {
            writeln!(s, "{name},{n},{bad}").unwrap();
        }
        writeln!(s, "contexts_hit_the_cap_of_128,{},0", self.cap_hits).unwrap();
        s
    }
}

/// What the example knows about one call after asking.
#[derive(Clone)]
struct Res {
    at: u64,
    refs: usize,
    present: usize,
    total: usize,
    m: usize,
    trace: CallTrace,
    feasible: bool,
}

impl Res {
    fn value(&self) -> f64 {
        if self.feasible { self.trace.p } else { 0.0 }
    }
}

struct Env<'a> {
    truth: &'a StreamTruth,
    events: &'a Events,
    streams: &'a [Stream],
    public: &'a PublicInfo,
    duration: u64,
}

impl Env<'_> {
    fn incident_present(&self, inc: Option<u32>, refs: &[ObsRef]) -> usize {
        match inc {
            None => 0,
            Some(id) => refs
                .iter()
                .filter(|r| match r {
                    ObsRef::Passive(o) => matches!(
                        self.truth.labels[o.0 as usize],
                        ObsLabel::Incident { id: i, role: EvidenceRole::Decisive } if i == id
                    ),
                    ObsRef::Probe(_) => false,
                })
                .count(),
        }
    }

    fn total_decisive(&self, inc: Option<u32>) -> usize {
        inc.map_or(0, |id| self.truth.incidents[id as usize].decisive.len())
    }

    fn deadline(&self, inc: Option<u32>) -> Option<u64> {
        inc.and_then(|id| self.truth.incidents[id as usize].deadline_ns)
    }

    /// Ask the law and cross-check the example's own `q` and `m` against the recorded ones.
    fn ask(
        &self,
        sim: &mut StreamSimulator,
        a: &Anom,
        refs: Vec<ObsRef>,
        at: u64,
        checks: &mut Checks,
    ) -> Result<Res, String> {
        let n_refs = refs.len();
        let present = self.incident_present(a.incident, &refs);
        let total = self.total_decisive(a.incident);
        let trace = escalate(sim, a.anchor, refs, at)?;
        let q_mine = match a.incident {
            None => 1.0,
            Some(_) if total == 0 => 1.0,
            Some(_) => present as f64 / total as f64,
        };
        checks.q_calls += 1;
        if trace.q != q_mine || trace.refs as usize != n_refs {
            checks.q_bad += 1;
        }
        checks.informed_calls += u64::from(trace.informed);
        if trace.informed && !trace.correct {
            checks.informed_wrong += 1;
        }
        if let Some(StreamHypothesis {
            kind: StreamKind::Hard(_),
            ..
        }) = trace.diagnosis
        {
            checks.hard_ans += 1;
            if !trace.correct {
                checks.hard_ans_wrong += 1;
            }
        }
        let feasible = match self.deadline(a.incident) {
            None => true,
            Some(dl) => at + latency_ns(n_refs) <= dl,
        };
        Ok(Res {
            at,
            refs: n_refs,
            present,
            total,
            m: n_refs - present,
            trace,
            feasible,
        })
    }
}

// ---- anomalies ---------------------------------------------------------------------------------

#[derive(Clone)]
struct Anom {
    id: u32,
    anchor: ObsId,
    site: ServiceId,
    anchor_at: u64,
    notice_at: u64,
    retire_at: Option<u64>,
    incident: Option<u32>,
    primary: bool,
    anchor_correct: bool,
    site_correct: bool,
}

impl Anom {
    fn live_at(&self, t: u64) -> bool {
        self.retire_at.is_none_or(|r| t < r)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Rung,
    Min,
}

impl Ctx {
    fn name(self) -> &'static str {
        match self {
            Ctx::Rung => "rung",
            Ctx::Min => "min",
        }
    }
}

#[derive(Clone, Copy)]
enum Task {
    Fixed { ctx: Ctx, pol: usize },
    Cand { ctx: Ctx, idx: usize },
}

/// One row of `w4-asks.csv`. `r` is `None` for a policy whose anomaly was not live at the ask
/// instant (no call is made; the row says so and carries no measure).
fn ask_row(
    out: &mut String,
    seed: u64,
    a: &Anom,
    (ctx, di, pol): (Ctx, usize, &str),
    ask_at: u64,
    r: Option<(&Res, usize)>,
) {
    let mut f: Vec<String> = vec![
        seed.to_string(),
        a.id.to_string(),
        a.incident.map_or(-1i64, i64::from).to_string(),
        ctx.name().to_string(),
        DELTAS[di].to_string(),
        pol.to_string(),
        u8::from(r.is_some()).to_string(),
        ask_at.to_string(),
    ];
    match r {
        Some((r, ncand)) => {
            f.push((r.at + latency_ns(r.refs)).to_string());
            f.push(u8::from(r.feasible).to_string());
            f.push(r.refs.to_string());
            f.push(r.present.to_string());
            f.push(r.total.to_string());
            f.push(r.m.to_string());
            f.push(format!("{:.9}", r.trace.d));
            f.push(format!("{:.9}", r.trace.h));
            f.push(format!("{:.9}", r.trace.p0));
            f.push(format!("{:.9}", r.trace.p));
            f.push(ncand.to_string());
        }
        None => f.extend(std::iter::repeat_n(String::new(), 11)),
    }
    writeln!(out, "{}", f.join(",")).unwrap();
}

const ASKS_HEADER: &str = "seed,anomaly,incident,ctx,delta,pol,live,ask_ns,ans_ns,feasible,refs,present,total,m,d,h,p0,p,ncand\n";

// ---- probes ------------------------------------------------------------------------------------

/// The six probes' results as flags: health check positive, resource usage positive, configuration
/// hash differs from the public hash, credential check positive, latency sample positive, error
/// sample positive. `hc == 2` would be an inconclusive health check (never generated here).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Obs6([u8; 6]);

impl Obs6 {
    fn text(&self) -> String {
        let c = |v: u8, on: char, off: char| match v {
            0 => off,
            1 => on,
            _ => '?',
        };
        format!(
            "{}{}{}{}{}{}",
            c(self.0[0], 'P', 'N'),
            c(self.0[1], 'P', 'N'),
            c(self.0[2], 'D', 'S'),
            c(self.0[3], 'P', 'N'),
            c(self.0[4], 'P', 'N'),
            c(self.0[5], 'P', 'N'),
        )
    }
}

fn flag(r: ProbeResult, public_hash: u64) -> u8 {
    match r {
        ProbeResult::Positive => 1,
        ProbeResult::Negative => 0,
        ProbeResult::ConfigHash(h) => u8::from(h != public_hash),
        ProbeResult::Inconclusive { .. } => 2,
    }
}

/// The patterns the public physics allows after `answer` at `site` (over the hidden parity bits).
fn predicted(public: &PublicInfo, answer: Diagnosis, site: ServiceId) -> Vec<Obs6> {
    let services = &public.services;
    let public_hash = services[site.index()].config_hash;
    let sentinel = public_hash ^ 0x5DEE_CE66_D1CE_4E5B;
    let truths: Vec<(Hypothesis, u64)> = match answer {
        Some(StreamHypothesis {
            kind: StreamKind::Known(k),
            site: s,
        }) => vec![(Some((k, s)), sentinel)],
        _ => vec![(None, 0)],
    };
    let mut out: Vec<Obs6> = Vec::new();
    for (truth, hash) in truths {
        for bits in [(false, false), (false, true), (true, false), (true, true)] {
            let mut v = [0u8; 6];
            for (j, kind) in ProbeKind::ALL.into_iter().enumerate() {
                let r = probe_result(services, truth, bits, hash, Probe { kind, target: site });
                v[j] = flag(r, public_hash);
            }
            let p = Obs6(v);
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Bit `s - 1` of the mask says whether probe subset `s` (1..=63, bit `j` of `s` = probe `j`)
/// accepts the observation: some allowed pattern agrees with it on every probe of the subset.
fn accept_mask(observed: Obs6, allowed: &[Obs6]) -> u64 {
    let mut mask = 0u64;
    for s in 1u32..64 {
        let ok = allowed
            .iter()
            .any(|p| (0..6).all(|j| (s >> j) & 1 == 0 || observed.0[j] == p.0[j]));
        if ok {
            mask |= 1u64 << (s - 1);
        }
    }
    mask
}

/// The probe-side simulator of one anomaly: fresh, so probes are asked in time order.
struct ProbeSim {
    sim: StreamSimulator,
    cache: HashMap<(u32, u64), Obs6>,
    hash: Vec<u64>,
}

impl ProbeSim {
    fn new(stream: &Stream, truth: &StreamTruth) -> Self {
        Self {
            sim: StreamSimulator::new(stream.clone()),
            cache: HashMap::new(),
            hash: truth.services.iter().map(|s| s.config_hash).collect(),
        }
    }

    fn observe(&mut self, target: ServiceId, t: u64) -> Result<Obs6, String> {
        if let Some(v) = self.cache.get(&(target.0, t)) {
            return Ok(*v);
        }
        let _ = self.sim.observe_until(Instant(t));
        let mut v = [0u8; 6];
        for (j, kind) in ProbeKind::ALL.into_iter().enumerate() {
            let out = self
                .sim
                .apply(StreamAction::Probe { kind, target }, Instant(t));
            match out {
                StreamOutcome::Probed {
                    observation: Observation::Probed { result, .. },
                    ..
                } => v[j] = flag(result, self.hash[target.index()]),
                other => return Err(format!("probe refused at {t}: {other:?}")),
            }
        }
        let o = Obs6(v);
        self.cache.insert((target.0, t), o);
        Ok(o)
    }
}

// ---- one stream --------------------------------------------------------------------------------

#[derive(Default)]
struct StreamOut {
    incidents: String,
    anomalies: String,
    decisive: String,
    asks: String,
    answers: String,
    other: String,
    regime: String,
    constants: String,
    ctx16: String,
    checks: Checks,
}

const INCIDENTS_HEADER: &str = "seed,incident,tier,family,mode,known_kind,duo,kind_a,kind_b,site,other,critical,onset_ns,first_obs_ns,deadline_ns,live_end_ns,difficulty,n_decisive,n_dec_site,n_dec_other,n_dec_else,dec_first_off_ns,dec_last_off_ns,epoch,sig_altered,edge_altered,recurrence_of,shift_kind,shift_to,edge_dependent,edge_dependency,n_services\n";
const ANOMALIES_HEADER: &str =
    "seed,anomaly,incident,anchor,site,anchor_at_ns,notice_at_ns,retire_at_ns,primary,anchor_correct,site_correct\n";
const DECISIVE_HEADER: &str =
    "seed,incident,obs,off_onset_ns,off_notice_ns,service,at_site,at_other,class,in_rung_ctx\n";
const ANSWERS_HEADER: &str = "seed,anomaly,incident,scenario,ask_ns,verify_ns,h,p0,ans_kind,ans_site_rel,right,weight,obs6,accept\n";
const OTHER_HEADER: &str =
    "seed,anomaly,incident,scenario,verify_ns,other_site,other_obs6,n_cands,other_in_cands\n";
const REGIME_HEADER: &str = "seed,incident,tier,onset_ns,sig_altered,edge_altered,n_open_act,truth_in_act,p0_act,n_open_cfp,truth_in_cfp,p0_cfp,n_open_cfg,truth_in_cfg,p0_cfg\n";
const CONSTANTS_HEADER: &str = "seed,n_services,beat_gap_min_ns,beat_gap_max_ns,beat_gap_sum_ns,beat_gap_n,pulse_n,pulse_benign_n,hard_dec_off_min_ns,hard_dec_off_max_ns,decoy_res_min_ns,decoy_res_max_ns,grace_min_ns,grace_max_ns,leak_cross_min_ns,leak_cross_max_ns,plain_dec_off_max_ns,dl_plain_min_ns,dl_plain_max_ns,dl_plain_crit_min_ns,dl_plain_crit_max_ns,dl_hard_min_ns,dl_hard_max_ns,dl_hard_crit_min_ns,dl_hard_crit_max_ns,regime0_at_ns,regime1_at_ns\n";
const CTX16_HEADER: &str = "seed,anomaly,incident,ask_ns,focus,refs\n";

fn family_mode(truth: &oracle::IncidentTruth) -> (&'static str, &'static str) {
    match (truth.tier, truth.shape.hard_kind) {
        (Tier::Plain, _) => ("known", ""),
        (_, Some(h)) => (
            hard_name(h),
            match truth.shape.contradicts_early {
                Some(true) => "contradict",
                Some(false) => "mimic",
                None => "",
            },
        ),
        (_, None) => ("unknown", ""),
    }
}

fn process_stream(
    world: World,
    seed: u64,
    seed_notices: &SeedNotices,
) -> Result<StreamOut, String> {
    let mut out = StreamOut::default();
    let ck = &mut out.checks;

    // The stream at the default budget, and at each delta with the budgets raised.
    let default_stream = generate(&params(world, seed));
    let default_truth = reveal(&default_stream);
    let mut streams: Vec<Stream> = Vec::new();
    for delta in DELTAS {
        let mut p: StreamParams = params(world, seed);
        p.reasoner.distractor_penalty = delta;
        p.budget.reasoner_ns = BIG;
        p.budget.probes = BIG;
        p.budget.probe_time_ns = BIG;
        let s = generate(&p);
        ck.stream_equal += 1;
        if s.events() != default_stream.events() || reveal(&s) != default_truth {
            ck.stream_bad += 1;
        }
        streams.push(s);
    }
    let truth = default_truth;
    let events = default_stream.events();
    let public = default_stream.public_info().world_public_info();
    let duration = truth.duration_ns;
    let env = Env {
        truth: &truth,
        events,
        streams: &streams,
        public: &public,
        duration,
    };
    for inc in &truth.incidents {
        let total_labels = truth
            .labels
            .iter()
            .filter(|l| matches!(l, ObsLabel::Incident { id, role: EvidenceRole::Decisive } if *id == inc.id))
            .count();
        ck.decisive_checked += 1;
        if total_labels != inc.decisive.len() {
            ck.decisive_total_bad += 1;
        }
    }

    // Anomalies from the kept run's notice records, validated against the regenerated stream.
    let mut anoms: Vec<Anom> = Vec::new();
    for n in &seed_notices.notices {
        ck.notices += 1;
        let ok = (n.anchor as usize) < events.len()
            && events[n.anchor as usize].0.0 == n.anchor_at
            && service_of(&events[n.anchor as usize].1) == Some(ServiceId(n.site))
            && truth.incident_of(ObsId(n.anchor)) == n.incident;
        if !ok {
            ck.notice_bad += 1;
            return Err(format!(
                "seed {seed}: notice {} does not match the regenerated stream",
                n.anomaly
            ));
        }
        anoms.push(Anom {
            id: n.anomaly,
            anchor: ObsId(n.anchor),
            site: ServiceId(n.site),
            anchor_at: n.anchor_at,
            notice_at: n.at,
            retire_at: seed_notices.retire.get(&n.anomaly).copied(),
            incident: n.incident,
            primary: false,
            anchor_correct: n.anchor_correct,
            site_correct: n.site_correct,
        });
    }
    anoms.sort_by_key(|a| (a.notice_at, a.id));
    let mut first: HashMap<u32, usize> = HashMap::new();
    for (i, a) in anoms.iter().enumerate() {
        if let Some(inc) = a.incident {
            first.entry(inc).or_insert(i);
        }
    }
    for &i in first.values() {
        anoms[i].primary = true;
    }

    // Static tables.
    let (shift_kind, shift_to, edge_dep, edge_dependency) = {
        let mut sk = (String::new(), String::new());
        let mut ed = (String::new(), String::new());
        for r in &truth.regimes {
            match r.detail {
                RegimeDetail::SignatureShift { kind, now_emits } => {
                    sk = (kind_name(kind).to_string(), kind_name(now_emits).to_string());
                }
                RegimeDetail::EdgeAdd {
                    dependent,
                    dependency,
                } => ed = (dependent.0.to_string(), dependency.0.to_string()),
                RegimeDetail::Nothing => {}
            }
        }
        (sk.0, sk.1, ed.0, ed.1)
    };
    let rebuilt: Vec<[Vec<(u64, Observation)>; 3]> = truth
        .incidents
        .iter()
        .map(|inc| {
            [(false, false), (true, false), (false, true)]
                .map(|(bp, zg)| rebuilt_incident(&default_stream, inc.id, bp, zg).expect("incident"))
        })
        .collect();
    for inc in &truth.incidents {
        let (family, mode) = family_mode(inc);
        let sh = &inc.shape;
        let site = inc.occupies[0];
        let (kind_a, kind_b) = match (sh.known_kind, sh.pair, sh.hard_kind, sh.mimics) {
            (Some(k), _, _, _) => (kind_name(k).to_string(), String::new()),
            (_, Some((a, b)), _, _) => (kind_name(a).to_string(), kind_name(b).to_string()),
            (_, _, Some(HardKind::Cascade), Some(a)) => (kind_name(a).to_string(), String::new()),
            _ => (String::new(), String::new()),
        };
        let first_obs = inc.observations.first().map(|o| events[o.0 as usize].0.0);
        let dec_at: Vec<u64> = inc.decisive.iter().map(|o| events[o.0 as usize].0.0).collect();
        let on_site = inc
            .decisive
            .iter()
            .filter(|o| service_of(&events[o.0 as usize].1) == Some(site))
            .count();
        let on_other = inc
            .decisive
            .iter()
            .filter(|o| sh.other.is_some() && service_of(&events[o.0 as usize].1) == sh.other)
            .count();
        let [held, base, zero] = &rebuilt[inc.id as usize];
        let epoch = truth
            .regimes
            .iter()
            .filter(|r| r.at_ns <= inc.onset_ns)
            .count();
        writeln!(
            out.incidents,
            "{seed},{},{},{family},{mode},{},{},{kind_a},{kind_b},{},{},{},{},{},{},{},{:.9},{},{on_site},{on_other},{},{},{},{epoch},{},{},{},{shift_kind},{shift_to},{edge_dep},{edge_dependency},{}",
            inc.id,
            tier_name(inc.tier),
            sh.known_kind.map_or("", kind_name),
            if sh.known_kind.is_some() { u8::from(sh.duo).to_string() } else { String::new() },
            site.0,
            opt(sh.other.map(|p| p.0)),
            u8::from(inc.critical),
            inc.onset_ns,
            opt(first_obs),
            opt(inc.deadline_ns),
            inc.live_end_ns,
            inc.difficulty,
            inc.decisive.len(),
            inc.decisive.len() - on_site - on_other,
            opt(dec_at.iter().min().map(|t| t - inc.onset_ns)),
            opt(dec_at.iter().max().map(|t| t - inc.onset_ns)),
            u8::from(base != held),
            u8::from(zero != held),
            opt(inc.recurrence_of),
            truth.services.len(),
        )
        .unwrap();
    }
    for a in &anoms {
        writeln!(
            out.anomalies,
            "{seed},{},{},{},{},{},{},{},{},{},{}",
            a.id,
            a.incident.map_or(-1i64, i64::from),
            a.anchor.0,
            a.site.0,
            a.anchor_at,
            a.notice_at,
            opt(a.retire_at),
            u8::from(a.primary),
            u8::from(a.anchor_correct),
            u8::from(a.site_correct),
        )
        .unwrap();
    }

    // Decisive evidence of hard incidents and decoys, with the primary anomaly's context.
    let mut regions: HashMap<u32, Vec<bool>> = HashMap::new();
    for a in &anoms {
        regions
            .entry(a.site.0)
            .or_insert_with(|| dependents_mask(&truth.services, a.site));
    }
    for inc in truth.incidents.iter().filter(|i| i.tier != Tier::Plain) {
        let prim = anoms.iter().find(|a| a.primary && a.incident == Some(inc.id));
        let site = inc.occupies[0];
        for o in &inc.decisive {
            let (at, obs) = &events[o.0 as usize];
            let svc = service_of(obs);
            let class = match obs {
                Observation::Counter { value, .. } if *value >= HIGH => "counter",
                Observation::Counter { .. } => "counter_benign",
                Observation::Message { text_id, .. } => {
                    if SignalText::from_text_id(*text_id).is_some() {
                        "message_cat"
                    } else {
                        "message_ext"
                    }
                }
                _ => "other",
            };
            let (in_ctx, off_notice) = match prim {
                Some(a) => {
                    let region = &regions[&a.site.0];
                    let lo = a.anchor_at.saturating_sub(LOOKBACK_NS);
                    let inside = svc.is_some_and(|s| admits(region, a.site, a.anchor_at, s, at.0))
                        && at.0 >= lo;
                    (
                        u8::from(inside).to_string(),
                        (at.0 as i64 - a.notice_at as i64).to_string(),
                    )
                }
                None => (String::new(), String::new()),
            };
            writeln!(
                out.decisive,
                "{seed},{},{},{},{off_notice},{},{},{},{class},{in_ctx}",
                inc.id,
                o.0,
                at.0 - inc.onset_ns,
                opt(svc.map(|s| s.0)),
                u8::from(svc == Some(site)),
                u8::from(svc.is_some() && svc == inc.shape.other),
            )
            .unwrap();
        }
    }

    // ---- the asks ----
    let mut b5_asks: Vec<(usize, Res, Vec<ObsRef>)> = Vec::new(); // anomaly index, d16 rung result
    let mut best_asks: Vec<(usize, Res, Vec<ObsRef>)> = Vec::new(); // primary, min ctx at opt (delta 0)
    for (ai, a) in anoms.iter().enumerate() {
        let region = &regions[&a.site.0];
        let n = a.notice_at;
        let inc = a.incident.map(|id| &truth.incidents[id as usize]);
        let dl = env.deadline(a.incident);
        let t_hi = {
            let mut t = duration;
            if let Some(d) = dl {
                t = t.min(d.saturating_sub(LAT_BASE_NS));
            }
            if let Some(r) = a.retire_at {
                t = t.min(r.saturating_sub(1));
            }
            t
        };
        let deltas: &[usize] = if a.primary { &[0, 1, 2] } else { &[0] };
        for &di in deltas {
            let mut sim = StreamSimulator::new(env.streams[di].clone());
            let mut tasks: Vec<(u64, Task)> = Vec::new();
            let ctxs: &[Ctx] = if a.primary {
                &[Ctx::Rung, Ctx::Min]
            } else {
                &[Ctx::Rung]
            };
            for &ctx in ctxs {
                for (pi, d) in DELAYS_S.iter().enumerate() {
                    if !a.primary && DELAYS_S[pi] != B5_DELAY_S {
                        continue;
                    }
                    tasks.push((n + d * SEC, Task::Fixed { ctx, pol: pi }));
                }
            }
            // Candidate instants of the scan.
            let mut cands: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
            if a.primary {
                let lo_i = events.partition_point(|(at, _)| at.0 <= n);
                let hi_i = events.partition_point(|(at, _)| at.0 <= t_hi);
                let mut rung_c: Vec<u64> = vec![n];
                for (at, obs) in &events[lo_i..hi_i.max(lo_i)] {
                    if let Some(s) = service_of(obs)
                        && admits(region, a.site, a.anchor_at, s, at.0)
                    {
                        rung_c.push(at.0);
                    }
                }
                rung_c.dedup();
                let mut min_c: Vec<u64> = vec![n];
                if let Some(inc) = inc {
                    let mut ts: Vec<u64> = inc
                        .decisive
                        .iter()
                        .map(|o| events[o.0 as usize].0.0)
                        .filter(|t| *t > n && *t <= t_hi)
                        .collect();
                    ts.sort_unstable();
                    ts.dedup();
                    min_c.extend(ts);
                }
                for (ci, list) in [rung_c, min_c].into_iter().enumerate() {
                    for (idx, t) in list.iter().enumerate() {
                        tasks.push((
                            *t,
                            Task::Cand {
                                ctx: if ci == 0 { Ctx::Rung } else { Ctx::Min },
                                idx,
                            },
                        ));
                    }
                    cands[ci] = list;
                }
            }
            tasks.sort_by_key(|(t, _)| *t);
            let mut cand_res: [Vec<Option<Res>>; 2] =
                [vec![None; cands[0].len()], vec![None; cands[1].len()]];
            for (t, task) in tasks {
                let ctx = match task {
                    Task::Fixed { ctx, .. } | Task::Cand { ctx, .. } => ctx,
                };
                let live = a.live_at(t) && t <= duration;
                if let Task::Fixed { pol, .. } = task
                    && !live
                {
                    let pname = format!("d{}", DELAYS_S[pol]);
                    ask_row(&mut out.asks, seed, a, (ctx, di, &pname), t, None);
                    continue;
                }
                let refs = match ctx {
                    Ctx::Rung => rung_context(events, region, a.site, a.anchor_at, t),
                    Ctx::Min => min_context(
                        events,
                        inc.map_or(&[][..], |i| &i.decisive[..]),
                        t,
                    ),
                };
                if ctx == Ctx::Rung && refs.len() == CTX_CAP {
                    out.checks.cap_hits += 1;
                }
                let refs_for_guess = refs.clone();
                let res = env.ask(&mut sim, a, refs, t, &mut out.checks)?;
                match task {
                    Task::Fixed { pol, .. } => {
                        let pname = format!("d{}", DELAYS_S[pol]);
                        ask_row(&mut out.asks, seed, a, (ctx, di, &pname), t, Some((&res, 0)));
                        if di == 0 {
                            // V4 on every fixed ask at delta 0.
                            check_guess(&env, a, &refs_for_guess, &res, &mut out.checks);
                        }
                        if di == 0 && ctx == Ctx::Rung && DELAYS_S[pol] == B5_DELAY_S {
                            let ids: Vec<String> = refs_for_guess
                                .iter()
                                .map(|r| match r {
                                    ObsRef::Passive(o) => o.0.to_string(),
                                    ObsRef::Probe(n) => format!("p{n}"),
                                })
                                .collect();
                            writeln!(
                                out.ctx16,
                                "{seed},{},{},{t},{},{}",
                                a.id,
                                a.incident.map_or(-1i64, i64::from),
                                a.anchor.0,
                                ids.join(";")
                            )
                            .unwrap();
                            b5_asks.push((ai, res.clone(), refs_for_guess));
                        }
                    }
                    Task::Cand { idx, .. } => {
                        let ci = usize::from(ctx == Ctx::Min);
                        cand_res[ci][idx] = Some(res);
                    }
                }
            }
            if a.primary {
                for (ci, ctx) in [Ctx::Rung, Ctx::Min].into_iter().enumerate() {
                    if cands[ci].is_empty() {
                        continue;
                    }
                    let mut best: Option<(usize, f64)> = None;
                    for (i, r) in cand_res[ci].iter().enumerate() {
                        let Some(r) = r else { continue };
                        if best.is_none_or(|(_, v)| r.value() > v) {
                            best = Some((i, r.value()));
                        }
                    }
                    let Some((bi, _)) = best else { continue };
                    let Some(r) = &cand_res[ci][bi] else {
                        continue;
                    };
                    ask_row(
                        &mut out.asks,
                        seed,
                        a,
                        (ctx, di, "opt"),
                        r.at,
                        Some((r, cands[ci].len())),
                    );
                    if di == 0 && ctx == Ctx::Min {
                        let refs = min_context(events, inc.map_or(&[][..], |i| &i.decisive[..]), r.at);
                        best_asks.push((ai, r.clone(), refs));
                    }
                }
            }
        }
    }

    // ---- the answers and the probes that follow them ----
    answers(&env, &anoms, &b5_asks, "b5", seed, &mut out)?;
    answers(&env, &anoms, &best_asks, "best", seed, &mut out)?;

    // ---- the regime tables: the public checker on each incident's own phase-1 evidence ----
    for inc in &truth.incidents {
        let site = inc.occupies[0];
        let mut cells: Vec<String> = Vec::new();
        for items in &rebuilt[inc.id as usize] {
            let ev: Vec<(Instant, Observation)> = items
                .iter()
                .filter(|(off, _)| *off <= T0_NS)
                .map(|(off, o)| (Instant(inc.onset_ns + off), o.clone()))
                .collect();
            let open = consistent_hypotheses(&public, &ev);
            let truth_h: Hypothesis = match inc.truth {
                Some(StreamHypothesis {
                    kind: StreamKind::Known(k),
                    site: s,
                }) => Some((k, s)),
                _ => None,
            };
            let in_open = match inc.truth {
                Some(StreamHypothesis {
                    kind: StreamKind::Hard(_),
                    ..
                }) => false,
                _ => open.contains(&truth_h),
            };
            let g = guess_distribution(&public, site, &ev);
            let p0 = share_on(&g, inc.truth);
            cells.push(format!("{},{},{:.9}", open.len(), u8::from(in_open), p0));
        }
        writeln!(
            out.regime,
            "{seed},{},{},{},{},{},{}",
            inc.id,
            tier_name(inc.tier),
            inc.onset_ns,
            u8::from(rebuilt[inc.id as usize][1] != rebuilt[inc.id as usize][0]),
            u8::from(rebuilt[inc.id as usize][2] != rebuilt[inc.id as usize][0]),
            cells.join(","),
        )
        .unwrap();
    }

    constants(&truth, events, seed, &mut out.constants);
    Ok(out)
}

/// V3 and V4 on one call: the example's `p0` from its own guess distribution equals the stream
/// crate's, and a realised uninformed answer lies in the support.
fn check_guess(env: &Env<'_>, a: &Anom, refs: &[ObsRef], res: &Res, ck: &mut Checks) {
    let ctx_obs: Vec<(Instant, Observation)> = refs
        .iter()
        .map(|r| match r {
            ObsRef::Passive(o) => env.events[o.0 as usize].clone(),
            ObsRef::Probe(_) => unreachable!("no probe is referenced"),
        })
        .collect();
    let focus_site = service_of_any(&env.events[a.anchor.0 as usize].1);
    let g = guess_distribution(env.public, focus_site, &ctx_obs);
    let truth = a
        .incident
        .and_then(|id| env.truth.incidents[id as usize].truth);
    ck.p0_calls += 1;
    if share_on(&g, truth) != res.trace.p0 {
        ck.p0_bad += 1;
    }
    if !res.trace.informed {
        ck.support_calls += 1;
        if !g.contains(&res.trace.diagnosis) {
            ck.support_bad += 1;
        }
    } else if res.trace.diagnosis != truth {
        ck.support_bad += 1;
    }
}

/// The answer distribution of each recorded ask and the probe patterns that follow it.
fn answers(
    env: &Env<'_>,
    anoms: &[Anom],
    asks: &[(usize, Res, Vec<ObsRef>)],
    scenario: &str,
    seed: u64,
    out: &mut StreamOut,
) -> Result<(), String> {
    for (ai, res, refs) in asks {
        let a = &anoms[*ai];
        let inc = a.incident.map(|id| &env.truth.incidents[id as usize]);
        let truth: Diagnosis = inc.and_then(|i| i.truth);
        let ctx_obs: Vec<(Instant, Observation)> = refs
            .iter()
            .map(|r| match r {
                ObsRef::Passive(o) => env.events[o.0 as usize].clone(),
                ObsRef::Probe(_) => unreachable!("no probe is referenced"),
            })
            .collect();
        let focus_site = service_of_any(&env.events[a.anchor.0 as usize].1);
        let g = guess_distribution(env.public, focus_site, &ctx_obs);
        let h = res.trace.h;
        let mut w: BTreeMap<Diagnosis, f64> = BTreeMap::new();
        *w.entry(truth).or_insert(0.0) += h;
        for x in &g {
            *w.entry(*x).or_insert(0.0) += (1.0 - h) / g.len() as f64;
        }
        out.checks.weight_checks += 1;
        let total: f64 = w.values().sum();
        let right: f64 = w.get(&truth).copied().unwrap_or(0.0);
        if (total - 1.0).abs() > 1e-9 || (right - res.trace.p).abs() > 1e-9 {
            out.checks.weight_bad += 1;
        }
        let verify_at = res.at + latency_ns(res.refs);
        if verify_at > env.duration {
            continue;
        }
        // The probes: at each answered site, and at the incident's other service.
        let mut psim = ProbeSim::new(&env.streams[0], env.truth);
        let target_of = |x: &Diagnosis| x.map_or(focus_site, |h| h.site);
        let mut targets: Vec<ServiceId> = w
            .iter()
            .filter(|(_, wt)| **wt > 0.0)
            .map(|(x, _)| target_of(x))
            .collect();
        targets.sort();
        targets.dedup();
        let other = inc.and_then(|i| i.shape.other);
        let mut obs_at: HashMap<u32, Obs6> = HashMap::new();
        for t in &targets {
            obs_at.insert(t.0, psim.observe(*t, verify_at)?);
        }
        for (x, wt) in &w {
            if *wt <= 0.0 {
                continue;
            }
            let site = target_of(x);
            let observed = obs_at[&site.0];
            let allowed = match x {
                Some(StreamHypothesis {
                    kind: StreamKind::Hard(_),
                    ..
                }) => vec![observed],
                _ => predicted(env.public, *x, site),
            };
            let mask = accept_mask(observed, &allowed);
            let rel = match x {
                None => "none",
                Some(h) if Some(h.site) == inc.map(|i| i.occupies[0]) && truth.is_some() => "truth",
                Some(h) if h.site == focus_site => "focus",
                Some(_) => "upstream",
            };
            writeln!(
                out.answers,
                "{seed},{},{},{scenario},{},{verify_at},{:.9},{:.9},{},{rel},{},{:.12},{},{mask:x}",
                a.id,
                a.incident.map_or(-1i64, i64::from),
                res.at,
                h,
                res.trace.p0,
                answer_name(*x),
                u8::from(*x == truth),
                wt,
                observed.text(),
            )
            .unwrap();
        }
        if let Some(o) = other {
            let obs = psim.observe(o, verify_at)?;
            // Candidates an arm could face: services outside the anomaly's site and its public
            // dependents with an abnormal observation between the anchor and the verification.
            let region = dependents_mask(&env.truth.services, a.site);
            let mut cands: Vec<u32> = Vec::new();
            let lo = env.events.partition_point(|(at, _)| at.0 < a.anchor_at);
            let hi = env.events.partition_point(|(at, _)| at.0 <= verify_at);
            for (_, ob) in &env.events[lo..hi] {
                let abnormal = match ob {
                    Observation::Counter { value, .. } => *value >= HIGH,
                    Observation::Message { text_id, .. } => matches!(
                        SignalText::from_text_id(*text_id),
                        Some(t) if t != SignalText::CheckHealth
                    ),
                    _ => false,
                };
                if !abnormal {
                    continue;
                }
                let Some(s) = service_of(ob) else { continue };
                if s != a.site && !region[s.index()] && !cands.contains(&s.0) {
                    cands.push(s.0);
                }
            }
            writeln!(
                out.other,
                "{seed},{},{},{scenario},{verify_at},{},{},{},{}",
                a.id,
                a.incident.map_or(-1i64, i64::from),
                o.0,
                obs.text(),
                cands.len(),
                u8::from(cands.contains(&o.0)),
            )
            .unwrap();
        }
    }
    Ok(())
}

/// Measured constants of one stream (item 5).
fn constants(truth: &StreamTruth, events: &Events, seed: u64, out: &mut String) {
    let mut gap_min = u64::MAX;
    let mut gap_max = 0u64;
    let mut gap_sum = 0u64;
    let mut gap_n = 0u64;
    let (mut pulse_n, mut pulse_benign) = (0u64, 0u64);
    let (mut hdo_min, mut hdo_max) = (u64::MAX, 0u64);
    let (mut res_min, mut res_max) = (u64::MAX, 0u64);
    let (mut gr_min, mut gr_max) = (u64::MAX, 0u64);
    let (mut lk_min, mut lk_max) = (u64::MAX, 0u64);
    let mut pdo_max = 0u64;
    let mut dl: [(u64, u64); 4] = [(u64::MAX, 0); 4];
    for inc in &truth.incidents {
        let site = inc.occupies[0];
        // Heartbeats: the incident's own readings at its site named ErrorRate or Saturation with
        // role Pulse or Closure (and a decoy's decisive recovery readings).
        let mut beats: Vec<u64> = Vec::new();
        for o in &inc.observations {
            let ObsLabel::Incident { role, .. } = truth.labels[o.0 as usize] else {
                continue;
            };
            let (at, ob) = &events[o.0 as usize];
            let is_beat = match role {
                EvidenceRole::Pulse | EvidenceRole::Closure => true,
                EvidenceRole::Decisive => inc.tier == Tier::Decoy,
                EvidenceRole::Presentation => false,
            };
            if !is_beat {
                continue;
            }
            if let Observation::Counter { service, value, .. } = ob
                && *service == site
            {
                beats.push(at.0);
                if role == EvidenceRole::Pulse && inc.shape.hard_kind != Some(HardKind::SlowLeak) {
                    pulse_n += 1;
                    if *value < HIGH {
                        pulse_benign += 1;
                    }
                }
            }
        }
        for w in beats.windows(2) {
            let g = w[1] - w[0];
            gap_min = gap_min.min(g);
            gap_max = gap_max.max(g);
            gap_sum += g;
            gap_n += 1;
        }
        let off = |o: &ObsId| events[o.0 as usize].0.0 - inc.onset_ns;
        match inc.tier {
            Tier::Hard => {
                for o in &inc.decisive {
                    hdo_min = hdo_min.min(off(o));
                    hdo_max = hdo_max.max(off(o));
                }
            }
            Tier::Plain => {
                for o in &inc.decisive {
                    pdo_max = pdo_max.max(off(o));
                }
            }
            Tier::Decoy => {
                let r = inc.live_end_ns - inc.onset_ns;
                res_min = res_min.min(r);
                res_max = res_max.max(r);
            }
        }
        if let Some(d) = inc.deadline_ns {
            let g = inc.live_end_ns - d;
            gr_min = gr_min.min(g);
            gr_max = gr_max.max(g);
            let cell = match (inc.tier, inc.critical) {
                (Tier::Plain, false) => Some(0),
                (Tier::Plain, true) => Some(1),
                (Tier::Hard, false) => Some(2),
                (Tier::Hard, true) => Some(3),
                _ => None,
            };
            if let Some(c) = cell {
                let v = d - inc.onset_ns;
                dl[c].0 = dl[c].0.min(v);
                dl[c].1 = dl[c].1.max(v);
            }
        }
        if inc.shape.hard_kind == Some(HardKind::SlowLeak) && inc.tier != Tier::Decoy {
            for o in &inc.observations {
                if let (at, Observation::Counter { service, name, value }) = &events[o.0 as usize]
                    && *service == site
                    && *name == gordian_world::CounterName::Saturation
                    && *value >= HIGH
                {
                    let v = at.0 - inc.onset_ns;
                    lk_min = lk_min.min(v);
                    lk_max = lk_max.max(v);
                    break;
                }
            }
        }
    }
    let f = |v: u64| if v == u64::MAX { String::new() } else { v.to_string() };
    let r0 = truth.regimes.first().map_or(0, |r| r.at_ns);
    let r1 = truth.regimes.get(1).map_or(0, |r| r.at_ns);
    writeln!(
        out,
        "{seed},{},{},{gap_max},{gap_sum},{gap_n},{pulse_n},{pulse_benign},{},{hdo_max},{},{res_max},{},{gr_max},{},{lk_max},{pdo_max},{},{},{},{},{},{},{},{},{r0},{r1}",
        truth.services.len(),
        f(gap_min),
        f(hdo_min),
        f(res_min),
        f(gr_min),
        f(lk_min),
        f(dl[0].0),
        dl[0].1,
        f(dl[1].0),
        dl[1].1,
        f(dl[2].0),
        dl[2].1,
        f(dl[3].0),
        dl[3].1,
    )
    .unwrap();
}

// ---- replay of recorded calls ------------------------------------------------------------------

fn replay(world: World, input: &PathBuf, out_dir: &PathBuf) -> Result<(), String> {
    let text = std::fs::read_to_string(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let mut by_seed: BTreeMap<u64, Vec<(u64, u32, Vec<u32>)>> = BTreeMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let seed: u64 = f[0].parse().map_err(|e| format!("{e}"))?;
        let at: u64 = f[1].parse().map_err(|e| format!("{e}"))?;
        let focus: u32 = f[2].parse().map_err(|e| format!("{e}"))?;
        let refs: Vec<u32> = if f[3].is_empty() {
            Vec::new()
        } else {
            f[3].split(';')
                .map(|x| x.parse().map_err(|e| format!("{e}")))
                .collect::<Result<_, _>>()?
        };
        by_seed.entry(seed).or_default().push((at, focus, refs));
    }
    let mut body = String::from("seed,at_ns,focus,incident,refs,q,m,h,p0,p,informed,correct\n");
    for (seed, calls) in by_seed {
        let mut p = params(world, seed);
        p.budget.reasoner_ns = BIG;
        let stream = generate(&p);
        let truth = reveal(&stream);
        let mut sim = StreamSimulator::new(stream);
        for (at, focus, refs) in calls {
            let n = refs.len();
            let refs: Vec<ObsRef> = refs.into_iter().map(|o| ObsRef::Passive(ObsId(o))).collect();
            let t = escalate(&mut sim, ObsId(focus), refs, at)?;
            let inc = truth.incident_of(ObsId(focus));
            let present = inc.map_or(0, |id| {
                (t.q * truth.incidents[id as usize].decisive.len() as f64).round() as usize
            });
            writeln!(
                body,
                "{seed},{at},{focus},{},{n},{:.12},{},{:.12},{:.12},{:.12},{},{}",
                inc.map_or(-1i64, i64::from),
                t.q,
                n.saturating_sub(present),
                t.h,
                t.p0,
                t.p,
                u8::from(t.informed),
                u8::from(t.correct),
            )
            .unwrap();
        }
    }
    std::fs::write(out_dir.join("w4-replay.csv"), body).map_err(|e| format!("{e}"))
}

// ---- main --------------------------------------------------------------------------------------

pub fn main() -> ExitCode {
    let (mut seed_from, mut count) = (40_000u64, 200u64);
    let mut world = World::A;
    let mut out_dir: Option<PathBuf> = None;
    let mut notices: Option<PathBuf> = None;
    let mut replay_in: Option<PathBuf> = None;
    let mut threads = 3usize;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "--w4" {
            continue;
        }
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--seed-from" => match value.parse() {
                Ok(v) => seed_from = v,
                Err(_) => return usage(),
            },
            "--count" => match value.parse() {
                Ok(v) => count = v,
                Err(_) => return usage(),
            },
            "--threads" => match value.parse() {
                Ok(v) => threads = v,
                Err(_) => return usage(),
            },
            "--world" => match value.as_str() {
                "a" => world = World::A,
                "c" => world = World::C,
                _ => return usage(),
            },
            "--notices" => notices = Some(PathBuf::from(value)),
            "--replay" => replay_in = Some(PathBuf::from(value)),
            "--out-dir" => out_dir = Some(PathBuf::from(value)),
            _ => return usage(),
        }
    }
    let Some(out_dir) = out_dir else {
        return usage();
    };
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("w4: cannot create {}: {e}", out_dir.display());
        return ExitCode::FAILURE;
    }
    if let Some(input) = replay_in {
        return match replay(world, &input, &out_dir) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("w4: replay: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let Some(notices) = notices else {
        return usage();
    };
    let table = match read_notices(&notices) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("w4: {e}");
            return ExitCode::FAILURE;
        }
    };
    let seeds: Vec<u64> = (seed_from..seed_from + count).collect();
    for s in &seeds {
        if !table.contains_key(s) {
            eprintln!("w4: the notice file has no record for seed {s}");
            return ExitCode::FAILURE;
        }
    }
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<Result<StreamOut, String>>>> =
        seeds.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= seeds.len() {
                        break;
                    }
                    let r = process_stream(world, seeds[i], &table[&seeds[i]]);
                    *slots[i].lock().unwrap() = Some(r);
                }
            });
        }
    });
    let mut files: Vec<(&str, String)> = vec![
        ("w4-incidents.csv", INCIDENTS_HEADER.to_string()),
        ("w4-anomalies.csv", ANOMALIES_HEADER.to_string()),
        ("w4-decisive.csv", DECISIVE_HEADER.to_string()),
        ("w4-asks.csv", ASKS_HEADER.to_string()),
        ("w4-answers.csv", ANSWERS_HEADER.to_string()),
        ("w4-other.csv", OTHER_HEADER.to_string()),
        ("w4-regime.csv", REGIME_HEADER.to_string()),
        ("w4-constants.csv", CONSTANTS_HEADER.to_string()),
        ("w4-ctx16.csv", CTX16_HEADER.to_string()),
    ];
    let mut checks = Checks::default();
    for (i, slot) in slots.into_iter().enumerate() {
        match slot.into_inner().unwrap() {
            Some(Ok(o)) => {
                files[0].1.push_str(&o.incidents);
                files[1].1.push_str(&o.anomalies);
                files[2].1.push_str(&o.decisive);
                files[3].1.push_str(&o.asks);
                files[4].1.push_str(&o.answers);
                files[5].1.push_str(&o.other);
                files[6].1.push_str(&o.regime);
                files[7].1.push_str(&o.constants);
                files[8].1.push_str(&o.ctx16);
                checks.add(&o.checks);
            }
            Some(Err(e)) => {
                eprintln!("w4: seed {}: {e}", seeds[i]);
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("w4: seed {} was not processed", seeds[i]);
                return ExitCode::FAILURE;
            }
        }
    }
    files.push(("w4-checks.csv", checks.csv()));
    for (name, body) in files {
        if let Err(e) = std::fs::write(out_dir.join(name), body) {
            eprintln!("w4: cannot write {name}: {e}");
            return ExitCode::FAILURE;
        }
    }
    if checks.bad() > 0 {
        eprintln!("w4: {} validation failures (see w4-checks.csv)", checks.bad());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
