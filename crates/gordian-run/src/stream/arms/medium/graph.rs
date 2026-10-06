//! The hand-designed noticing graph (work item M2, build item 4) and its parameters.
//!
//! Public information only: the public graph (which services depend on which), the public rules'
//! verdict on each observation, its kind, its value and its time. Nothing about tiers, families,
//! incidents or the hidden rules is encoded here; every number below is a parameter in the
//! manifest, chosen on the tuning streams (10000-10099), and its meaning is stated where it is used.
//!
//! # The graph, per service `n` (the parameters are [`MediumParams`]; each path can be switched off)
//!
//! | Cell | Archetype | Inputs | What it is for |
//! |---|---|---|---|
//! | `abn[n]` | `Sense`, count | abnormal observations about `n` | the public alarm signal at `n` |
//! | `notice[n]` | `Emit`, kind notice, lookback the larger of `lookback_ns` and `burst_lookback_ns`, refractory `refractory_ns` | `onset[n]`, `burst[n]`, `prop[n]` | the notice; its anchor is the earliest event in the support that reaches it |
//! | `onset[n]` (`onset`) | `Integrator`, leak from `onset_tau_ns`, threshold `onset_threshold`, reset, lookback `lookback_ns` | `abn[n]` (1), `abn[d]` for each dependent `d` of `n` (`dependent_weight`) | several alarms at `n` (and its dependents) within a short time |
//! | `kind[n, k]` (`burst`) | `Sense`, count | abnormal observations about `n` of kind `k`: error rate, latency, a message | one input per kind |
//! | `other[n]` (`burst`) | `Integrator`, no memory, threshold 1/2, lookback 0 | `Sense` cells of the other kinds (saturation, authentication failures, restarts, a snapshot) | the fourth kind, relayed |
//! | `burst[n]` (`burst`) | `Coincidence` (form `coincidence`), n = `burst_n`, consumed, window `burst_window_ns`, lookback `burst_lookback_ns` | `kind[n, *]`, `other[n]` | abnormal observations of `burst_n` distinct kinds at `n` within the window: in the ordered form, by their time inside the tick (`offset_ns`) |
//! | `confirm[n]`, `relay[n]` (`burst_confirm`) | `Latch`, hold `confirm_hold_ns`; `Integrator`, no memory | `abn[d]` of the confirming services (dependents, or all others); `burst[n]`, `confirm_delay_ticks` late | the burst reaches `notice[n]` only through `relay[n]`, gated by `confirm[n]`: a burst of two kinds counts when another service alarmed around it; a gate carries no references, so the anchor stays at `n` |
//! | `three[n]` (`burst3_window_ns`) | `Coincidence`, n = 3, window `burst3_window_ns` | `kind[n, *]`, `other[n]` | three kinds at `n`: a burst without confirmation |
//! | `dep[n]`, `prop[n]` (`propagation`) | relay; `Coincidence` (form `coincidence`), n = 2, lead, window `coincidence_window_ns` | `abn[n]` (slot 0), `abn[d]` of the dependents | an alarm at `n` and then one at a dependent (public rule 1) |
//! | `arrived[n, c]`, `value[n, c]` (`ramp`) | `Sense`, presence and sum | counter `c` at `n`, benign or not | that a reading came, and the reading itself (five counters) |
//! | `jump[n, c]` (`ramp`) | `Novelty`, rate 1 (its estimate is the last reading), band `ramp_jump`, one reading of warm-up | `value[n, c]` | the reading jumped from the last one by more than `ramp_jump` |
//! | `smooth[n, c]` (`ramp`) | `Integrator`, leak from `ramp_tau_ns`, threshold `ramp_threshold`, reset, lookback `ramp_lookback_ns` | `arrived[n, c]` (+1), `jump[n, c]` (minus `ramp_penalty` per `ramp_jump` of jump) | readings of one counter that come often and move little: a ramp, which noise (readings far apart, each drawn afresh) is not |
//! | `rampnotice[n]` (`ramp`) | `Emit`, kind notice, lookback `ramp_lookback_ns`, refractory `refractory_ns` | `smooth[n, c]` | the notice of a ramp |
//! | `hold[n]` | `Latch`, retiring, hold `hold_ns` | `notice[n]`, `rampnotice[n]`, and `abn[n]` while it holds | an open anomaly at `n`; it proposes `retire` when `n` has been quiet for the hold |
//!
//! Times are given in nanoseconds and converted by the medium at build time for the tick length
//! (the oscillome's `seconds`), so a change of tick does not change the program except by rounding
//! (the conversion table is in the report). The ordered coincidence's window is microseconds and
//! does not depend on the tick; the sliding one's is ticks (rounded up); the binned one's is a bin
//! of the 10 s rhythm, at least a tick.
//!
//! # The anchoring rule, as used here
//!
//! The emitter's anchor is the earliest event in its support; its support is the support of the
//! cell that fired it, which an `Integrator` or a `Coincidence` prunes to its lookback in ticks.
//! So the lookback of a path is set on its integrator or coincidence and on the emitter alike: it
//! is "the emitter's lookback" of the brief, the parameter anchor correctness is reported against.
//! A lookback of 0 cites only the tick that fired.

use super::adapters::{ABNORMAL_KIND, CH_COUNTER, DOMAIN, TAG_ABNORMAL, counter_tag};
use gordian_medium::{
    CellId, Gate, Limits, MediumBuilder, MediumSpec, Oscillome, Pattern, Prices, SenseMode,
    SynapseSpec, Tag, TimeTarget,
};
use gordian_world::graph::dependents_mask;
use gordian_world::{CounterName, Service};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kind of a proposal that is a notice.
pub const KIND_NOTICE: u16 = 1;
/// The kind of a proposal that is a retirement.
pub const KIND_RETIRE: u16 = 2;

/// The periods of the oscillome's rhythms when they are on: 10 s and 100 s (M1b's first set).
pub const RHYTHMS_NS: [u64; 2] = [10_000_000_000, 100_000_000_000];

/// Which form of `Coincidence` the burst and propagation cells use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoincidenceForm {
    /// None (neither the burst nor the propagation cells may exist).
    Off,
    /// M1's sliding window, in ticks.
    Sliding,
    /// Binned by the 10 s rhythm (needs rhythms): bins of the window, at least a tick.
    Binned,
    /// Ordered by event time (`offset_ns`), microseconds.
    Ordered,
}

/// Which services' alarms confirm a burst of two kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirm {
    /// No confirmation: the burst reaches the emitter directly.
    None,
    /// An alarm at a dependent of the service (the dependents `direct_dependents` names).
    Dependents,
    /// An alarm at any other service.
    All,
}

/// The medium noticer's parameters: the tick length, the graph's numbers, and the switches the
/// ablations turn. Written into a manifest as the `medium` noticer's fields.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediumParams {
    /// The base tick, nanoseconds (M2 sweeps 100 ms, 500 ms and 2 s). A whole number of
    /// microseconds, at most `u32::MAX`.
    pub tick_ns: u64,
    /// The control of the synthesis decision: the sense adapter delivers only observations the
    /// public rules call abnormal.
    #[serde(default)]
    pub abnormal_only: bool,
    /// Whether the onset integrator path exists.
    #[serde(default = "yes")]
    pub onset: bool,
    /// Time constant of the onset integrator's leak, nanoseconds.
    pub onset_tau_ns: u64,
    /// The onset integrator's threshold, in abnormal observations (a dependent's weighted).
    pub onset_threshold: f32,
    /// Weight of an abnormal observation at a dependent in the onset integrator of its upstream.
    pub dependent_weight: f32,
    /// Whether "dependent" means a direct dependent only (a service that names `n` in its
    /// `depends_on`) rather than every service that depends on `n`, directly or not.
    #[serde(default)]
    pub direct_dependents: bool,
    /// The onset path's anchor lookback (integrator, propagation coincidence), nanoseconds.
    pub lookback_ns: u64,
    /// Fewest nanoseconds between two notices of one emitter.
    pub refractory_ns: u64,
    /// The form of the burst and propagation coincidences.
    pub coincidence: CoincidenceForm,
    /// The propagation coincidence's window, nanoseconds.
    pub coincidence_window_ns: u64,
    /// Whether the burst cells exist.
    #[serde(default)]
    pub burst: bool,
    /// Distinct kinds of abnormal observation at one service that make a burst (2 to 4).
    #[serde(default = "two")]
    pub burst_n: u8,
    /// The burst coincidence's window, nanoseconds.
    #[serde(default)]
    pub burst_window_ns: u64,
    /// The burst path's anchor lookback, nanoseconds.
    #[serde(default)]
    pub burst_lookback_ns: u64,
    /// Which services' alarms must confirm a burst (gating it, `confirm_delay_ticks` late).
    #[serde(default = "no_confirm")]
    pub burst_confirm: Confirm,
    /// How long a confirming alarm keeps the gate open, nanoseconds.
    #[serde(default)]
    pub confirm_hold_ns: u64,
    /// How many ticks after a burst its confirmation is read (1 or more): a confirming alarm in
    /// the burst's tick, or up to this many ticks after it less one, has reached the latch by then.
    #[serde(default = "two")]
    pub confirm_delay_ticks: u8,
    /// With a confirmation: the window of an unconfirmed burst of three kinds (0: none).
    #[serde(default)]
    pub burst3_window_ns: u64,
    /// Whether the propagation cells exist.
    #[serde(default)]
    pub propagation: bool,
    /// Whether the ramp cells exist.
    pub ramp: bool,
    /// The step between two readings of one counter above which it is a jump, counter units.
    pub ramp_jump: f32,
    /// What a jump of `ramp_jump` costs the ramp integrator, in readings (a larger jump costs
    /// proportionally more).
    pub ramp_penalty: f32,
    /// Time constant of the ramp integrator's leak, nanoseconds.
    pub ramp_tau_ns: u64,
    /// The ramp integrator's threshold, in readings.
    pub ramp_threshold: f32,
    /// The ramp path's anchor lookback, nanoseconds.
    pub ramp_lookback_ns: u64,
    /// How long a service must be quiet before its anomalies retire, nanoseconds.
    pub hold_ns: u64,
    /// Whether the oscillome's rhythms (10 s, 100 s) exist.
    #[serde(default)]
    pub rhythms: bool,
}

fn yes() -> bool {
    true
}

fn two() -> u8 {
    2
}

fn no_confirm() -> Confirm {
    Confirm::None
}

impl Default for MediumParams {
    /// A starting point, not a tuned value: 100 ms ticks; an onset of three alarms within about
    /// 300 ms; the ramp path on; no burst or propagation cells.
    fn default() -> Self {
        Self {
            tick_ns: 100_000_000,
            abnormal_only: false,
            onset: true,
            onset_tau_ns: 300_000_000,
            onset_threshold: 3.0,
            dependent_weight: 1.0,
            direct_dependents: false,
            lookback_ns: 200_000_000,
            refractory_ns: 6_000_000_000,
            coincidence: CoincidenceForm::Off,
            coincidence_window_ns: 300_000_000,
            burst: false,
            burst_n: 2,
            burst_window_ns: 25_000_000,
            burst_lookback_ns: 0,
            burst_confirm: Confirm::None,
            confirm_hold_ns: 300_000_000,
            confirm_delay_ticks: 2,
            burst3_window_ns: 0,
            propagation: false,
            ramp: true,
            ramp_jump: 8.0,
            ramp_penalty: 3.0,
            ramp_tau_ns: 3_000_000_000,
            ramp_threshold: 2.0,
            ramp_lookback_ns: 3_000_000_000,
            hold_ns: 6_000_000_000,
            rhythms: false,
        }
    }
}

impl MediumParams {
    /// Check the parameters, and that they build a valid medium on a small public graph.
    pub fn validate(&self) -> Result<(), String> {
        let t = self.tick_ns;
        if t == 0 || t > u64::from(u32::MAX) || !t.is_multiple_of(1_000) {
            return Err(
                "noticer medium: tick_ns must be a positive whole number of \
                        microseconds of at most u32::MAX"
                    .to_owned(),
            );
        }
        let finite = [
            self.onset_threshold,
            self.dependent_weight,
            self.ramp_jump,
            self.ramp_penalty,
            self.ramp_threshold,
        ];
        if finite.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err(
                "noticer medium: thresholds, weights and rates must be finite and \
                        non-negative"
                    .to_owned(),
            );
        }
        if self.onset_threshold <= 0.0 || (self.ramp && self.ramp_threshold <= 0.0) {
            return Err("noticer medium: thresholds must be positive".to_owned());
        }
        if (self.burst || self.propagation) && self.coincidence == CoincidenceForm::Off {
            return Err(
                "noticer medium: the burst and propagation cells need a coincidence form"
                    .to_owned(),
            );
        }
        if self.burst_confirm != Confirm::None && self.confirm_delay_ticks == 0 {
            return Err("noticer medium: confirm_delay_ticks must be at least 1".to_owned());
        }
        if self.burst && !(2..=4).contains(&self.burst_n) {
            return Err("noticer medium: burst_n must be 2 to 4".to_owned());
        }
        if self.coincidence == CoincidenceForm::Binned && !self.rhythms {
            return Err("noticer medium: a binned coincidence needs the rhythms".to_owned());
        }
        let probe = probe_graph();
        spec(self, &probe)
            .map(|_| ())
            .map_err(|e| format!("noticer medium: {e}"))
    }
}

/// A small public graph to check parameters on: a chain and a fan, five services.
fn probe_graph() -> Vec<Service> {
    use gordian_world::{ResourceKind, ServiceId};
    let deps: [&[u32]; 5] = [&[], &[0], &[0], &[1], &[1, 2]];
    deps.iter()
        .enumerate()
        .map(|(i, d)| Service {
            id: ServiceId(i as u32),
            depends_on: d.iter().map(|x| ServiceId(*x)).collect(),
            resource: ResourceKind::Cpu,
            config_hash: i as u64,
            unreliable_health: false,
        })
        .collect()
}

/// What each cell of a built graph is, for the effector and the tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// The service each retiring latch watches, by cell.
    pub latches: BTreeMap<CellId, u32>,
    /// The notice emitter of each service.
    pub notice: Vec<CellId>,
    /// The ramp emitter of each service, when the ramp cells exist.
    pub ramp_notice: Vec<CellId>,
}

impl Layout {
    /// The service whose emitter `cell` is, if it is an emitter.
    pub fn emitter_node(&self, cell: CellId) -> Option<u32> {
        self.notice
            .iter()
            .position(|c| *c == cell)
            .or_else(|| self.ramp_notice.iter().position(|c| *c == cell))
            .and_then(|i| u32::try_from(i).ok())
    }
}

/// The services whose alarms confirm a burst at service `i`, if a confirmation is asked for.
fn confirmers(confirm: Confirm, i: usize, dependents: &[Vec<usize>]) -> Option<Vec<usize>> {
    match confirm {
        Confirm::None => None,
        Confirm::Dependents => Some(dependents[i].clone()),
        Confirm::All => Some((0..dependents.len()).filter(|j| *j != i).collect()),
    }
}

/// Set parameter `index` of `cell` in time.
fn timed(b: &mut MediumBuilder, cell: CellId, index: u8, ns: u64) {
    b.timed(TimeTarget::Param { cell, index }, ns);
}

/// A coincidence of `n` sources in the form `params.coincidence`, consumed on firing, with
/// `window_ns` and `lookback_ns` given in time; `lead` (ordered form only): the first source first.
fn coincidence(
    b: &mut MediumBuilder,
    params: &MediumParams,
    n: u8,
    window_ns: u64,
    lookback_ns: u64,
    lead: bool,
) -> CellId {
    let c = match params.coincidence {
        CoincidenceForm::Sliding => {
            let c = b.coincidence(n, 0, true, 0);
            timed(b, c, 1, window_ns);
            c
        }
        CoincidenceForm::Binned => {
            let bins = (RHYTHMS_NS[0] / window_ns.max(params.tick_ns)).max(1) as u32;
            b.coincidence_binned(n, 0, bins, 0, true, 0)
        }
        CoincidenceForm::Ordered | CoincidenceForm::Off => {
            let c = b.coincidence_ordered(n, 0, lead, true, 0);
            timed(b, c, 1, window_ns);
            c
        }
    };
    timed(b, c, 3, lookback_ns);
    c
}

/// The medium's spec for the public graph `services`, and what its cells are.
pub fn spec(params: &MediumParams, services: &[Service]) -> Result<(MediumSpec, Layout), String> {
    let n = services.len();
    let mut b = MediumBuilder::new()
        .limits(Limits {
            max_passes: 3,
            ..Limits::default()
        })
        .prices(Prices::DECLARED);
    let mut layout = Layout::default();

    let node = |i: usize| u16::try_from(i).unwrap_or(u16::MAX);
    let at = |i: usize, channel: Option<u16>, tag: Tag| Pattern {
        domain: Some(DOMAIN),
        node: Some(node(i)),
        channel,
        tag: Some(tag),
    };
    let abn: Vec<CellId> = (0..n)
        .map(|i| b.sense(at(i, None, TAG_ABNORMAL), SenseMode::Count))
        .collect();
    let dependents: Vec<Vec<usize>> = (0..n)
        .map(|i| {
            if params.direct_dependents {
                (0..n)
                    .filter(|j| services[*j].depends_on.contains(&services[i].id))
                    .collect()
            } else {
                let mask = dependents_mask(services, services[i].id);
                (0..n).filter(|j| *j != i && mask[*j]).collect()
            }
        })
        .collect();

    for i in 0..n {
        // The emitter: fires whenever a cell feeding it fires (their activations are positive).
        let notice = b.emit(f32::MIN_POSITIVE, KIND_NOTICE, 0, 0);
        let confirm_ticks = if params.burst_confirm == Confirm::None {
            0
        } else {
            u64::from(params.confirm_delay_ticks) * params.tick_ns
        };
        timed(
            &mut b,
            notice,
            2,
            params
                .lookback_ns
                .max(params.burst_lookback_ns + confirm_ticks),
        );
        timed(&mut b, notice, 3, params.refractory_ns);
        layout.notice.push(notice);

        // Several alarms at the service (and, weighted, its dependents) within a short time.
        if params.onset {
            let onset = b.integrator(0.0, params.onset_threshold, true, 0);
            timed(&mut b, onset, 0, params.onset_tau_ns);
            timed(&mut b, onset, 3, params.lookback_ns);
            b.synapse(abn[i], onset, 1.0, 0);
            if params.dependent_weight > 0.0 {
                for &d in &dependents[i] {
                    b.synapse(abn[d], onset, params.dependent_weight, 0);
                }
            }
            b.synapse(onset, notice, 1.0, 0);
        }

        // A burst: abnormal observations of `burst_n` distinct kinds at the service within the
        // window. Kinds: error rate (0), latency (1), a message (5), and the others (2, 3, 4: the
        // other counters; 6: a snapshot) through a relay with no memory.
        if params.burst {
            let kinds: Vec<CellId> = [0u32, 1, 5]
                .iter()
                .map(|k| b.sense(at(i, None, Tag(ABNORMAL_KIND + k)), SenseMode::Count))
                .collect();
            let other = b.integrator(0.0, 0.5, true, 0);
            for k in [2u32, 3, 4, 6] {
                let s = b.sense(at(i, None, Tag(ABNORMAL_KIND + k)), SenseMode::Count);
                b.synapse(s, other, 1.0, 0);
            }
            let mut slots = kinds;
            slots.push(other);
            let burst = coincidence(
                &mut b,
                params,
                params.burst_n,
                params.burst_window_ns,
                params.burst_lookback_ns,
                false,
            );
            for s in &slots {
                b.synapse(*s, burst, 1.0, 0);
            }
            match confirmers(params.burst_confirm, i, &dependents) {
                None => {
                    b.synapse(burst, notice, 1.0, 0);
                }
                Some(from) => {
                    // The burst reaches the emitter `confirm_delay_ticks` later, through a relay,
                    // and only while a latch says a confirming service alarmed within
                    // `confirm_hold_ns` (alarms in the burst's tick and the ticks before the
                    // relay runs have reached the latch by then; the latch is a gate, so its
                    // events are not cited).
                    let confirm = b.latch(f32::MIN_POSITIVE, 0);
                    timed(&mut b, confirm, 1, params.confirm_hold_ns);
                    for d in from {
                        b.synapse(abn[d], confirm, 1.0, 0);
                    }
                    let relay = b.integrator(0.0, f32::MIN_POSITIVE, true, 0);
                    timed(
                        &mut b,
                        relay,
                        3,
                        params.burst_lookback_ns
                            + u64::from(params.confirm_delay_ticks) * params.tick_ns,
                    );
                    b.synapse(burst, relay, 1.0, params.confirm_delay_ticks);
                    b.synapse_with(SynapseSpec {
                        from: relay,
                        to: notice,
                        weight: 1.0,
                        delay_ticks: 0,
                        gate: Gate::Cell(confirm),
                        plastic: false,
                    });
                    if params.burst3_window_ns > 0 {
                        // Three kinds at the service need no confirmation.
                        let three = coincidence(
                            &mut b,
                            params,
                            3,
                            params.burst3_window_ns,
                            params.burst_lookback_ns,
                            false,
                        );
                        for s in &slots {
                            b.synapse(*s, three, 1.0, 0);
                        }
                        b.synapse(three, notice, 1.0, 0);
                    }
                }
            }
        }

        // Propagation: an alarm at the service, then one at a dependent (public rule 1).
        if params.propagation && !dependents[i].is_empty() {
            let dep = b.integrator(0.0, 0.5, true, 0);
            for &d in &dependents[i] {
                b.synapse(abn[d], dep, 1.0, 0);
            }
            let prop = coincidence(
                &mut b,
                params,
                2,
                params.coincidence_window_ns,
                params.lookback_ns,
                true,
            );
            // Slot 0 is the first incoming synapse by id: the site's own alarms.
            b.synapse(abn[i], prop, 1.0, 0);
            b.synapse(dep, prop, 1.0, 0);
            b.synapse(prop, notice, 1.0, 0);
        }

        // The ramp path: per counter, a reading arrived (+1) and it jumped from the last one
        // (minus `ramp_penalty` per `ramp_jump` of jump), leaking with `ramp_tau_ns`.
        let mut ramp_notice = None;
        if params.ramp {
            let rn = b.emit(f32::MIN_POSITIVE, KIND_NOTICE, 0, 0);
            timed(&mut b, rn, 2, params.ramp_lookback_ns);
            timed(&mut b, rn, 3, params.refractory_ns);
            for name in CounterName::ALL {
                let pattern = at(i, Some(CH_COUNTER), counter_tag(name));
                let arrived = b.sense(pattern, SenseMode::Presence);
                let value = b.sense(pattern, SenseMode::Sum);
                // Rate 1: the running estimate is the last reading, so the deviation is the
                // step from it; band = the floor, `ramp_jump`; one reading of warm-up.
                let jump = b.novelty(1.0, 0.0, params.ramp_jump, 1, false);
                let smooth = b.integrator(0.0, params.ramp_threshold, true, 0);
                timed(&mut b, smooth, 0, params.ramp_tau_ns);
                timed(&mut b, smooth, 3, params.ramp_lookback_ns);
                b.synapse(value, jump, 1.0, 0);
                b.synapse(arrived, smooth, 1.0, 0);
                let penalty = -params.ramp_penalty / params.ramp_jump.max(1.0);
                b.synapse(jump, smooth, penalty, 0);
                b.synapse(smooth, rn, 1.0, 0);
            }
            layout.ramp_notice.push(rn);
            ramp_notice = Some(rn);
        }

        // Retirement: a latch opened by a notice, kept open by alarms at the service while it
        // holds, proposing `retire` when the service has been quiet for the hold.
        let hold = b.latch_retiring(f32::MIN_POSITIVE, 0, KIND_RETIRE);
        timed(&mut b, hold, 1, params.hold_ns);
        b.synapse(notice, hold, 1.0, 0);
        if let Some(rn) = ramp_notice {
            b.synapse(rn, hold, 1.0, 0);
        }
        b.synapse_with(SynapseSpec {
            from: abn[i],
            to: hold,
            weight: 1.0,
            delay_ticks: 0,
            gate: Gate::Cell(hold),
            plastic: false,
        });
        layout
            .latches
            .insert(hold, u32::try_from(i).unwrap_or(u32::MAX));
    }

    let mut o = Oscillome {
        tick_len_ns: params.tick_ns,
        ..Oscillome::default()
    };
    if params.rhythms {
        o.periods_ns = RHYTHMS_NS.to_vec();
    }
    let spec = b.oscillome(o).into_spec();
    let (resolved, _) = spec.resolved().map_err(|e| format!("{e:?}"))?;
    Ok((resolved, layout))
}
