//! How the components relate to the checker over generated episodes, per class.
//!
//! Descriptive, not asserting, and `#[ignore]`d so that it prints only when asked:
//!
//! ```text
//! cargo test -p gordian-components --release --test agreement -- --ignored --nocapture
//! ```
//!
//! Columns, each a count over 100 seeds of one class (default spec parameters): the heuristic
//! made a proposal; the estimator made a proposal; the lookup made a proposal, and that proposal
//! is in the checker's set; the checker's set has exactly one member; and the verifier reported
//! damaged evidence when the window held only the most recent 16, 32, or 64 observations.

mod common;

use common::state_of;
use gordian_components::payload::{HypothesisEntry, decode};
use gordian_components::{
    Component, ConsistencyVerifier, CountEstimator, PriorRecordLookup, RuleHeuristic,
};
use gordian_world::physics::consistent_hypotheses;
use gordian_world::{EpisodeClass, EpisodeSpec, generate};

#[test]
#[ignore = "descriptive table, run with --ignored --nocapture"]
fn agreement_table() {
    println!(
        "| class | heuristic proposes | estimator proposes | lookup proposes | lookup in checker set | checker singleton | damaged at 16 | at 32 | at 64 |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for class in EpisodeClass::ALL {
        let (mut h, mut e, mut m, mut mv, mut single) = (0, 0, 0, 0, 0);
        let mut damaged = [0u32; 3];
        for seed in 0..100 {
            let ep = generate(&EpisodeSpec::new(seed, class));
            let len = ep.stream().len();
            let full = state_of(&ep, len, len);
            let set = consistent_hypotheses(&full.public, ep.stream());
            h += u32::from(RuleHeuristic::new().run(&full).proposal.is_some());
            e += u32::from(CountEstimator::new().run(&full).proposal.is_some());
            if let Some(p) = PriorRecordLookup::new().run(&full).proposal {
                m += 1;
                mv += u32::from(set.contains(&p));
            }
            single += u32::from(set.len() == 1);
            for (slot, cap) in [16, 32, 64].into_iter().enumerate() {
                let out = ConsistencyVerifier::new().run(&state_of(&ep, cap, len));
                let entry = decode(&out.entries[0].1).unwrap();
                damaged[slot] +=
                    u32::from(matches!(entry, HypothesisEntry::EvidenceDamaged { .. }));
            }
        }
        println!(
            "| {class:?} | {h} | {e} | {m} | {mv} | {single} | {} | {} | {} |",
            damaged[0], damaged[1], damaged[2]
        );
    }
}
