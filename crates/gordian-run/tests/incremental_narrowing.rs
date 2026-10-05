//! Work item A6d: the shared rule narrows each stored hypothesis set once, not once per step.
//!
//! The acceptance of the item is that nothing the rule decides changes, and only what it costs
//! does. `verdicts_at_the_default_budget_are_those_the_rule_gave_before_incremental_narrowing`
//! checks that end to end through the recorder against a record written before the change; the
//! tests below it check the mechanism and its accounting.

mod common;

use common::*;
use gordian_run::recorder::execute;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/verdicts-before-incremental-narrowing.csv"
);

/// Every arm at the default 20 ms compute budget, 20 seeds by 11 classes, must give for each
/// episode the verdict columns the rule gave before work item A6d. The fixture was written by this
/// test at the last commit before the rule changed, by running it with
/// `GORDIAN_WRITE_A6D_VERDICT_FIXTURE=1`; it is a record of the old behaviour, so it is never
/// regenerated from the new code. At 20 ms no arm is near its limit, so a verdict that moved would
/// mean the rule's decisions depended on how often it narrowed.
#[test]
fn verdicts_at_the_default_budget_are_those_the_rule_gave_before_incremental_narrowing() {
    let arms = b1_arms();
    let m = grid_manifest("a6d-verdicts", arms.clone(), 20, 20_000_000);
    let dir = scratch("a6d-verdicts");
    execute(&m, &dir).unwrap();
    let now = verdicts(&dir, &arms);
    assert_eq!(now.len(), arms.len() * 11 * 20);

    if std::env::var_os("GORDIAN_WRITE_A6D_VERDICT_FIXTURE").is_some() {
        let mut text = format!("arm,seed,class,{}\n", VERDICT_COLUMNS.join(","));
        for ((arm, seed, class), v) in &now {
            text.push_str(&format!("{arm},{seed},{class},{v}\n"));
        }
        fs::create_dir_all(Path::new(FIXTURE).parent().unwrap()).unwrap();
        fs::write(FIXTURE, text).unwrap();
        return;
    }

    let recorded = fs::read_to_string(FIXTURE).expect("the pre-A6d verdict fixture");
    let mut before = BTreeMap::new();
    for line in recorded.lines().skip(1) {
        let f: Vec<&str> = line.splitn(4, ',').collect();
        before.insert(
            (
                f[0].to_owned(),
                f[1].parse::<u64>().unwrap(),
                f[2].to_owned(),
            ),
            f[3].to_owned(),
        );
    }
    assert_eq!(before.len(), now.len(), "the fixture covers every episode");
    let moved: Vec<String> = now
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, v)| format!("{k:?}: before {:?}, now {v:?}", before.get(k)))
        .collect();
    assert!(
        moved.is_empty(),
        "{} of {} verdicts changed, first: {:#?}",
        moved.len(),
        now.len(),
        &moved[..moved.len().min(10)]
    );
}
