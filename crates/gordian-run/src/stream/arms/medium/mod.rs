//! The medium as a noticer on the stream world (work item M2, Lab 1).
//!
//! A hand-designed graph of cells on `gordian-medium` ([`graph`]), fed every delivered
//! observation with its value through the sense and clock adapters ([`adapters`]), emitting
//! notices and retirements into B1's `Noticer` seam ([`noticing`]); everything downstream (the
//! shared rung, its context, the selection rule) is the rung's and does not depend on this
//! noticer. The medium's counted operations, priced, plus a price per tick, are charged to the
//! arm's bill like a component call (`Meter::charge_noticer`).
//!
//! # What the medium may know
//!
//! What every arm may: the public rules (through the verdict on each observation, which the rung
//! computes from the public graph), the public graph (which services depend on which), and the
//! observations as delivered, with their values and instants. Nothing else crosses the sense
//! port. The graph's parameters are chosen on the tuning streams and recorded in the manifest.
//! This directory is held to the textual ban of `scripts/check-no-oracle.sh` like every file
//! under `arms/`.
//!
//! # Memory (work item A1a)
//!
//! [`engram`]: an engram layer beside the noticing graph, switched on by the parameters'
//! `engram` field. It binds the arm's own answered incidents into engrams and recalls them by
//! coincidence; a recall reaches the arm through the seam's memory hooks
//! ([`super::noticer::Noticer::answered`], [`super::noticer::Noticer::recalls`]) and is declared
//! without an escalation. The key definition and the confirmation policy are in [`engram`]'s
//! documentation.
//!
//! # The recall gate and the two-site key (work item A1c)
//!
//! [`gate`]: a recall is acted on only for an anomaly whose latest public consistency check found
//! no consistent hypothesis (the rung keeps the verdict for a gated noticer). The two-site key, a
//! key over the anomaly's service and one service the public graph does not connect to it, is in
//! [`engram`]'s documentation, "A1c".

//! # Where a recall may speak, and its counters (work item A1d)
//!
//! [`gate`]'s `stale` value: a recall speaks only on a contradicted anomaly that carries no
//! declaration made strictly after the checker's last consistent verdict (the rung says which
//! declarations stand). [`trace`]: the engram layer's counters as marks on its medium's trace port,
//! written to the arm's own trace file.

pub mod adapters;
pub mod engram;
pub mod gate;
pub mod graph;
pub mod noticing;
pub mod trace;

pub use engram::{ConfirmPolicy, EngramConfig, EngramLayer, SiteMode};
pub use gate::{Reading, RecallGate};
pub use graph::{
    CoincidenceForm, Confirm, InhibitForm, KIND_NOTICE, KIND_RETIRE, Layout, MediumParams,
};
pub use noticing::{MEDIUM_COMPONENT, MEDIUM_ID, MediumNoticer, MediumStats};

use super::noticer::Noticer;
use super::rung::RungConfig;
use gordian_world::Service;

/// The medium noticer `params` names, for the public graph `services`, with the rung's
/// parameters `cfg`.
///
/// # Panics
///
/// If the graph does not build for this public graph. [`MediumParams::validate`] builds it on a
/// small graph when the manifest is checked, and the graph's structure does not depend on the
/// graph's size beyond the number of cells, so this is a defect, not a result.
pub fn build(params: &MediumParams, cfg: &RungConfig, services: &[Service]) -> Box<dyn Noticer> {
    match MediumNoticer::new(*params, cfg.clone(), services) {
        Ok(n) => Box::new(n),
        Err(e) => panic!("the medium noticer's graph does not build: {e}"),
    }
}
