//! The hand-designed noticing graph (work item M2, build item 4) and its parameters.
//!
//! Public information only: the public graph (which services depend on which), the public rules'
//! verdict on each observation, its value and its time. Nothing about tiers, families, incidents
//! or the hidden rules is encoded here; every number below is a parameter in the manifest, chosen
//! on the tuning streams (10000-10099), and its meaning is stated where it is used.
//!
//! # The graph, per service `n` (structure fixed; the parameters are [`MediumParams`])
//!
//! | Cell | Archetype | Inputs | What it is for |
//! |---|---|---|---|
//! | `abn[n]` | `Sense`, count | abnormal observations about `n` | the public alarm signal at `n` |
//! | `onset[n]` | `Integrator`, leak from `onset_tau_ns`, threshold `onset_threshold`, reset, lookback `lookback_ns` | `abn[n]` (1), `abn[d]` for each dependent `d` of `n` (`dependent_weight`) | a burst at `n` and the services that depend on it: what an incident's start looks like and a lone stray does not |
//! | `dep[n]` | `Integrator`, no memory, threshold 1/2, lookback 0 | `abn[d]` for each dependent `d` | relays "a dependent of `n` alarmed in this tick" (only with a coincidence) |
//! | `prop[n]` | `Coincidence` (sliding, binned or ordered by event time, `coincidence`), n = 2, consumed, lookback `lookback_ns` | `abn[n]` (slot 0), `dep[n]` (slot 1) | propagation: an alarm at `n` and one at a dependent within `coincidence_window_ns` (ordered: `n` first, public rule 1) |
//! | `notice[n]` | `Emit`, kind notice, lookback `lookback_ns`, refractory `refractory_ns` | `onset[n]`, `prop[n]` | the notice; its anchor is the earliest event in the support that reaches it |
//! | `arrived[n, c]`, `value[n, c]` | `Sense`, presence and sum | counter `c` at `n`, benign or not | that a reading came, and the reading itself (five counters) |
//! | `jump[n, c]` | `Novelty`, rate 1 (its estimate is the last reading), band `ramp_jump`, one reading of warm-up | `value[n, c]` | the reading jumped from the last one by more than `ramp_jump` |
//! | `smooth[n, c]` | `Integrator`, leak from `ramp_tau_ns`, threshold `ramp_threshold`, reset, lookback `ramp_lookback_ns` | `arrived[n, c]` (+1), `jump[n, c]` (minus `ramp_penalty` per `ramp_jump` of jump) | readings of one counter that come often and move little: a ramp, which noise (readings far apart, each drawn afresh) is not |
//! | `rampnotice[n]` | `Emit`, kind notice, lookback `ramp_lookback_ns`, refractory `refractory_ns` | `smooth[n, c]` | the notice of a ramp |
//! | `hold[n]` | `Latch`, retiring, hold `hold_ns` | `notice[n]`, `rampnotice[n]`, and `abn[n]` while it holds | an open anomaly at `n`; it proposes `retire` when `n` has been quiet for the hold |
//!
//! The ramp cells exist only when `ramp` is on; `dep` and `prop` only with a coincidence and only
//! for a service that has a dependent. Times are given in nanoseconds and converted by the
//! medium at build time for the tick length (the oscillome's `seconds`), so a change of tick does
//! not change the program except by rounding (the conversion table is in the report).
//!
//! # The anchoring rule, as used here
//!
//! The emitter's anchor is the earliest event in its support; its support is the support of the
//! cell that fired it, which an `Integrator` or a `Coincidence` prunes to its lookback in ticks.
//! So `lookback_ns` is set on the integrator, the coincidence and the emitter alike: it is "the
//! emitter's lookback" of the brief, the parameter anchor correctness is reported against. A
//! lookback of 0 cites only the tick that crossed the threshold.

use super::adapters::{CH_COUNTER, DOMAIN, TAG_ABNORMAL, counter_tag};
use gordian_medium::{
    CellId, Gate, Limits, MediumBuilder, MediumSpec, Oscillome, Pattern, Prices, SenseMode,
    SynapseSpec, TimeTarget,
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

/// Which form of `Coincidence` detects propagation, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoincidenceForm {
    /// No propagation cells.
    Off,
    /// M1's sliding window, in ticks.
    Sliding,
    /// Binned by the 10 s rhythm (needs rhythms): bins of `coincidence_window_ns`.
    Binned,
    /// Ordered by event time (`offset_ns`), the site first.
    Ordered,
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
    /// The anchor lookback (integrator, coincidence and emitter), nanoseconds.
    pub lookback_ns: u64,
    /// Fewest nanoseconds between two notices of one emitter.
    pub refractory_ns: u64,
    /// The propagation coincidence's form.
    pub coincidence: CoincidenceForm,
    /// Its window, nanoseconds (ticks when sliding, bins when binned, microseconds when ordered).
    pub coincidence_window_ns: u64,
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

impl Default for MediumParams {
    /// A starting point, not a tuned value: 100 ms ticks; an onset of three alarms within about
    /// 300 ms; the ramp path on.
    fn default() -> Self {
        Self {
            tick_ns: 100_000_000,
            abnormal_only: false,
            onset_tau_ns: 300_000_000,
            onset_threshold: 3.0,
            dependent_weight: 1.0,
            direct_dependents: false,
            lookback_ns: 200_000_000,
            refractory_ns: 6_000_000_000,
            coincidence: CoincidenceForm::Off,
            coincidence_window_ns: 300_000_000,
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
                "noticer medium: tick_ns must be a positive whole number of microseconds \
                        of at most u32::MAX"
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
    /// The onset emitter of each service.
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
    let abn: Vec<CellId> = (0..n)
        .map(|i| {
            b.sense(
                Pattern {
                    domain: Some(DOMAIN),
                    node: Some(node(i)),
                    channel: None,
                    tag: Some(TAG_ABNORMAL),
                },
                SenseMode::Count,
            )
        })
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
        // The onset integrator.
        let onset = b.integrator(0.0, params.onset_threshold, true, 0);
        b.timed(
            TimeTarget::Param {
                cell: onset,
                index: 0,
            },
            params.onset_tau_ns,
        );
        b.timed(
            TimeTarget::Param {
                cell: onset,
                index: 3,
            },
            params.lookback_ns,
        );
        b.synapse(abn[i], onset, 1.0, 0);
        if params.dependent_weight > 0.0 {
            for &d in &dependents[i] {
                b.synapse(abn[d], onset, params.dependent_weight, 0);
            }
        }
        // The emitter: fires whenever a cell feeding it fires (their activations are positive).
        let notice = b.emit(f32::MIN_POSITIVE, KIND_NOTICE, 0, 0);
        b.timed(
            TimeTarget::Param {
                cell: notice,
                index: 2,
            },
            params.lookback_ns,
        );
        b.timed(
            TimeTarget::Param {
                cell: notice,
                index: 3,
            },
            params.refractory_ns,
        );
        b.synapse(onset, notice, 1.0, 0);
        layout.notice.push(notice);

        // Propagation, when a coincidence is asked for and the service has a dependent.
        if params.coincidence != CoincidenceForm::Off && !dependents[i].is_empty() {
            let dep = b.integrator(0.0, 0.5, true, 0);
            for &d in &dependents[i] {
                b.synapse(abn[d], dep, 1.0, 0);
            }
            let prop = match params.coincidence {
                CoincidenceForm::Sliding => {
                    let c = b.coincidence(2, 0, true, 0);
                    b.timed(
                        TimeTarget::Param { cell: c, index: 1 },
                        params.coincidence_window_ns,
                    );
                    c
                }
                CoincidenceForm::Binned => {
                    let bins = (RHYTHMS_NS[0] / params.coincidence_window_ns.max(params.tick_ns))
                        .max(1) as u32;
                    b.coincidence_binned(2, 0, bins, 0, true, 0)
                }
                CoincidenceForm::Ordered | CoincidenceForm::Off => {
                    let c = b.coincidence_ordered(2, 0, true, true, 0);
                    b.timed(
                        TimeTarget::Param { cell: c, index: 1 },
                        params.coincidence_window_ns,
                    );
                    c
                }
            };
            b.timed(
                TimeTarget::Param {
                    cell: prop,
                    index: 3,
                },
                params.lookback_ns,
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
            b.timed(
                TimeTarget::Param { cell: rn, index: 2 },
                params.ramp_lookback_ns,
            );
            b.timed(
                TimeTarget::Param { cell: rn, index: 3 },
                params.refractory_ns,
            );
            for name in CounterName::ALL {
                let pattern = Pattern {
                    domain: Some(DOMAIN),
                    node: Some(node(i)),
                    channel: Some(CH_COUNTER),
                    tag: Some(counter_tag(name)),
                };
                let arrived = b.sense(pattern, SenseMode::Presence);
                let value = b.sense(pattern, SenseMode::Sum);
                // Rate 1: the running estimate is the last reading, so the deviation is the
                // step from it; band = the floor, `ramp_jump`; one reading of warm-up.
                let jump = b.novelty(1.0, 0.0, params.ramp_jump, 1, false);
                let smooth = b.integrator(0.0, params.ramp_threshold, true, 0);
                b.timed(
                    TimeTarget::Param {
                        cell: smooth,
                        index: 0,
                    },
                    params.ramp_tau_ns,
                );
                b.timed(
                    TimeTarget::Param {
                        cell: smooth,
                        index: 3,
                    },
                    params.ramp_lookback_ns,
                );
                b.synapse(value, jump, 1.0, 0);
                b.synapse(arrived, smooth, 1.0, 0);
                b.synapse(
                    jump,
                    smooth,
                    -params.ramp_penalty / params.ramp_jump.max(1.0),
                    0,
                );
                b.synapse(smooth, rn, 1.0, 0);
            }
            layout.ramp_notice.push(rn);
            ramp_notice = Some(rn);
        }

        // Retirement: a latch opened by a notice, kept open by alarms at the service while it
        // holds, proposing `retire` when the service has been quiet for the hold.
        let hold = b.latch_retiring(f32::MIN_POSITIVE, 0, KIND_RETIRE);
        b.timed(
            TimeTarget::Param {
                cell: hold,
                index: 1,
            },
            params.hold_ns,
        );
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
    let b = b.oscillome(o);
    let spec = b.into_spec();
    let (resolved, _) = spec.resolved().map_err(|e| format!("{e:?}"))?;
    Ok((resolved, layout))
}
