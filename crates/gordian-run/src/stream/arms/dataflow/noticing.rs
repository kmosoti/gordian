//! [`DataflowNoticer`]: the program behind B1's `Noticer` seam, the view the rung reads, and the
//! billing of the program's counted work.
//!
//! # The view
//!
//! The rung reads anomalies as [`Tracked`] values (anchor, site, the attached observations in
//! order, the evidence digest, the noticed instant, the peak score). The relations are the truth;
//! this adapter keeps a `Tracked` per anomaly equal to them, from the deltas of the two logged
//! relations, after every call that can change them: an append to an anomaly is applied in place,
//! anything else (rows leaving it, a new anomaly) rebuilds the anomaly's view from its rows. The
//! rules never read the view, and the view's own private timing is not the rules' (it is used by
//! nothing outside the hand-written noticers). The adapter's work is not counted in the modelled
//! cost; it is in the measured wall time.
//!
//! # Billing
//!
//! [`Noticer::take_cost`] reports the program's counted work since the last report, priced at
//! [`Prices::DECLARED`], under [`DATAFLOW_COMPONENT`]; the arm charges it to the bill like a
//! component call, as the medium's. Work done after the last report of a segment (the tail of its
//! last step) is never charged. With `billed` off in the spec nothing is reported.

use super::DataflowSpec;
use super::engine::{OpCounts, Prices};
use super::program::{Changes, Program};
use crate::stream::arms::noticer::{Notice, Noticer, NoticerCost, Tracked};
use crate::stream::arms::rung::{Held, RungConfig, Store};
use gordian_core::{ComponentId, Instant};
use gordian_world::{Service, ServiceId};
use std::collections::BTreeMap;

/// The noticer's id, as the run output writes it.
pub const DATAFLOW_ID: &str = "dataflow";

/// The id the noticer's charges are attributed to on the bill (`Phase::Component`): 16 is the
/// medium's, 17 the learned noticer's, 0 to 3 the components'.
pub const DATAFLOW_COMPONENT: ComponentId = ComponentId(18);

/// The dataflow noticer. See the module documentation of [`super`].
pub struct DataflowNoticer {
    program: Program,
    services: Vec<Service>,
    burst_gap_ns: u64,
    views: Vec<Tracked>,
    billed: bool,
    charged: OpCounts,
    stopped: bool,
}

impl std::fmt::Debug for DataflowNoticer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataflowNoticer")
            .field("anomalies", &self.views.len())
            .field("counts", &self.program.counts())
            .finish()
    }
}

impl DataflowNoticer {
    /// A noticer for the public graph `services` under the rung's parameters `cfg`.
    pub fn new(spec: DataflowSpec, cfg: RungConfig, services: &[Service]) -> Self {
        Self {
            program: Program::new(&spec, &cfg, services),
            services: services.to_vec(),
            burst_gap_ns: cfg.burst_gap_ns,
            views: Vec::new(),
            billed: spec.billed,
            charged: OpCounts::default(),
            stopped: false,
        }
    }

    /// Everything the program counted since construction.
    pub fn counts(&self) -> OpCounts {
        self.program.counts()
    }

    /// The modelled cost of everything counted, nanoseconds, at the declared prices.
    pub fn total_ns(&self) -> u64 {
        self.counts().modelled_ns(&Prices::DECLARED)
    }

    /// The program, for the tests.
    pub fn program(&self) -> &Program {
        &self.program
    }

    fn position(&self, id: u32) -> Result<usize, usize> {
        self.views.binary_search_by_key(&id, |t| t.id)
    }

    fn rebuild(&mut self, id: u32) {
        let members = self.program.peek_members(id);
        let Some(first) = members.first() else {
            if let Ok(i) = self.position(id) {
                self.views.remove(i);
            }
            return;
        };
        let held = first.held();
        let mut t = Tracked::new(
            id,
            &held,
            ServiceId(first.svc),
            &self.services,
            self.burst_gap_ns,
        );
        for m in &members[1..] {
            t.note_attached(&m.held(), ServiceId(m.svc), self.burst_gap_ns);
        }
        match self.position(id) {
            Ok(i) => self.views[i] = t,
            Err(i) => self.views.insert(i, t),
        }
    }

    /// Bring the view up to date with the relations.
    fn sync(&mut self) {
        let Changes { anomalies, members } = self.program.take_changes();
        if anomalies.is_empty() && members.is_empty() {
            return;
        }
        // For each anomaly touched: whether rows left it or were replaced (rebuild), and the rows
        // appended, in order.
        let mut touched: BTreeMap<u32, (bool, Vec<usize>)> = BTreeMap::new();
        for d in &anomalies {
            touched.entry(d.key).or_default();
        }
        for (n, d) in members.iter().enumerate() {
            let entry = touched.entry(d.key.0).or_default();
            if d.old.is_some() || d.new.is_none() {
                entry.0 = true;
            } else {
                entry.1.push(n);
            }
        }
        for (id, (rebuild, appended)) in touched {
            let Some(a) = self.program.peek_anomaly(id).copied() else {
                if let Ok(i) = self.position(id) {
                    self.views.remove(i);
                }
                continue;
            };
            let in_place = match self.position(id) {
                Ok(i) => !rebuild && self.views[i].anchor.0 == a.anchor,
                Err(_) => false,
            };
            if in_place {
                if let Ok(i) = self.position(id) {
                    for &n in &appended {
                        if let Some(m) = members[n].new.as_ref() {
                            self.views[i].note_attached(
                                &m.held(),
                                ServiceId(m.svc),
                                self.burst_gap_ns,
                            );
                        }
                    }
                }
            } else {
                self.rebuild(id);
            }
            if let Ok(i) = self.position(id) {
                self.views[i].noticed_at = a.noticed.map(Instant);
                self.views[i].peak_score = a.peak;
            }
        }
    }
}

impl Noticer for DataflowNoticer {
    fn id(&self) -> &'static str {
        DATAFLOW_ID
    }

    fn observe(&mut self, held: &Held) -> Option<u32> {
        if self.stopped {
            return None;
        }
        let id = self.program.observe(held);
        self.sync();
        id
    }

    fn notice(&mut self, now: Instant, store: &Store) -> Vec<Notice> {
        if self.stopped {
            return Vec::new();
        }
        let out = self.program.notice(now, store);
        self.sync();
        out
    }

    fn anomalies(&self) -> &[Tracked] {
        &self.views
    }

    fn tracked(&self, id: u32) -> Option<&Tracked> {
        self.position(id).ok().map(|i| &self.views[i])
    }

    fn score(&self, id: u32, now: Instant) -> f64 {
        self.program.score(id, now)
    }

    fn refresh(&mut self, now: Instant) {
        self.program.refresh(now);
        self.sync();
    }

    fn retirable(&self, now: Instant) -> Vec<u32> {
        self.program.retirable(now)
    }

    fn retire(&mut self, id: u32) {
        self.program.retire(id);
        self.sync();
    }

    fn take_cost(&mut self) -> Option<NoticerCost> {
        if !self.billed {
            return None;
        }
        let total = self.counts();
        let done = total.since(self.charged);
        if done.is_zero() {
            return None;
        }
        self.charged = total;
        Some(NoticerCost {
            component: DATAFLOW_COMPONENT,
            compute_ns: done.modelled_ns(&Prices::DECLARED),
        })
    }

    fn refused(&mut self) {
        self.stopped = true;
    }
}
