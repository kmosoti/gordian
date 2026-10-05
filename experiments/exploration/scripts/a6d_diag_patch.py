"""A6d: write the scratch diagnostic variants of the shared rule into a COPY of the tree.

Stage B exploration script (development run; nothing here tests a hypothesis). Never run on the
repository tree: the variants read environment variables inside the rule and the harness, which
the reference core must not do. Standard library only.

    a6d_diag_patch.py COPY_DIR [--tree=old|new]

COPY_DIR holds a copy of `crates/` (and Cargo.toml, Cargo.lock, rust-toolchain.toml, .cargo). The
edits are exact string replacements and fail loudly if the code they target has changed. Each
variant is a pure function of the process environment, so one binary serves every condition.

Both trees: the harness reads GORDIAN_DIAG_DIRECTIVES, which of the episode's harness directives
apply: all (default), none, fail (only `Fail`), slow (only `Slow`).

The patch also writes the example that lists which components each ComponentTimeout episode fails
(`cargo run --release -p gordian-world --example directives`, the input of `a6d_explain.py`).

`--tree=old` is the rule as of A6c (a `Decider` that recognises a repeated output and narrows at
every call). It adds

    GORDIAN_DIAG_WORLD_FREE        the narrowing term of the declared cost is zero
    GORDIAN_DIAG_VERIFIER_UNUSED   the verifier's stored set is never a candidate source, while
                                   every charge (which reads the stored sets) is unchanged:
                                   "priority only"
    GORDIAN_DIAG_VERIFIER_FREE     the decoding and comparing of the verifier's outputs is not
                                   declared: "cost only"

`--tree=new` is the rule as of A6d (narrowed views kept, work declared one step late). The harness
also records, per episode, which components the episode's directives fail (whether or not they are
applied), and the rule reads that mask under

    GORDIAN_DIAG_MIRROR=cost       the work the rule does on the outputs of the components the
                                   episode fails (decoding, comparing, narrowing, scoring) is not
                                   declared, as if they had failed, while their outputs are used
                                   as usual: the cost channel of a `Fail` alone
    GORDIAN_DIAG_MIRROR=priority   the decision is taken as if those components' stored sets were
                                   absent, while every charge is unchanged: the priority channel
                                   of a `Fail` alone
    GORDIAN_DIAG_MIRROR=both       both, which is a `Fail` except for anything else a `Fail` does

Run with GORDIAN_DIAG_DIRECTIVES=none, the mirrors say what each channel of a `Fail` is worth.
"""
import pathlib
import sys


def sub(text, old, new, what):
    if text.count(old) != 1:
        sys.exit(f"patch target not found exactly once: {what}")
    return text.replace(old, new)


def patch_harness(root, mask):
    p = root / "crates/gordian-run/src/harness.rs"
    t = p.read_text()
    hook = ""
    if mask:
        hook = """    // DIAGNOSTIC PATCH: which components the episode's directives fail, for the mirror variants.
    gordian_run_diag_mask(&directives);
"""
    t = sub(
        t,
        "    let directives = episode.harness_directives().to_vec();\n",
        """    let mut directives = episode.harness_directives().to_vec();
"""
        + hook
        + """    // DIAGNOSTIC PATCH (scratch copy, not in the repository): GORDIAN_DIAG_DIRECTIVES selects
    // which harness directives are applied: all (default), none, fail, slow.
    match std::env::var("GORDIAN_DIAG_DIRECTIVES").as_deref() {
        Ok("none") => directives.clear(),
        Ok("fail") => directives.retain(|d| matches!(d.mode, ComponentMode::Fail)),
        Ok("slow") => directives.retain(|d| matches!(d.mode, ComponentMode::Slow { .. })),
        _ => {}
    }
""",
        "harness directives",
    )
    if mask:
        t += """
/// DIAGNOSTIC PATCH: record which components the episode's directives fail.
fn gordian_run_diag_mask(directives: &[gordian_world::ComponentDirective]) {
    let mut mask = 0u32;
    for d in directives {
        if matches!(d.mode, ComponentMode::Fail) && d.component < 32 {
            mask |= 1u32 << d.component;
        }
    }
    crate::policy::decide::DIAG_FAILED.store(mask, std::sync::atomic::Ordering::Relaxed);
}
"""
    p.write_text(t)


def patch_old_rule(root):
    p = root / "crates/gordian-run/src/policy/decide.rs"
    t = p.read_text()
    # cost only: the verifier's decode is not carried into the next declared cost
    t = sub(
        t,
        "        if !output.entries.is_empty() {\n            self.decoded.0 += 1;",
        """        let free = id == VERIFIER_ID && std::env::var("GORDIAN_DIAG_VERIFIER_FREE").is_ok();
        if !output.entries.is_empty() && !free {
            self.decoded.0 += 1;""",
        "absorb decode",
    )
    t = sub(
        t,
        "            self.decoded.2 += bytes;\n",
        """            if !(id == VERIFIER_ID && std::env::var("GORDIAN_DIAG_VERIFIER_FREE").is_ok()) {
                self.decoded.2 += bytes;
            }
""",
        "absorb compare",
    )
    # priority only: the verifier's set is never read by the decision, every charge unchanged
    t = sub(
        t,
        "    fn view(&self, state: &WorkingState, bought: &Bought, ops: &mut RuleOps) -> Option<View> {\n        self.sources().find_map(|set| {",
        """    fn decision_sources(&self) -> Vec<&Vec<Hypothesis>> {
        let skip = std::env::var("GORDIAN_DIAG_VERIFIER_UNUSED").is_ok();
        [&self.verifier, &self.estimator, &self.heuristic]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !(skip && *i == 0))
            .filter_map(|(_, held)| held.as_ref()?.set.as_ref())
            .collect()
    }

    fn view(&self, state: &WorkingState, bought: &Bought, ops: &mut RuleOps) -> Option<View> {
        self.decision_sources().into_iter().find_map(|set| {""",
        "view sources",
    )
    # narrowing term of the declared cost is zero
    t = sub(
        t,
        "            .saturating_add(WORLD_PS.saturating_mul(f.worlds))\n",
        """            .saturating_add(if std::env::var("GORDIAN_DIAG_WORLD_FREE").is_ok() {
                0
            } else {
                WORLD_PS.saturating_mul(f.worlds)
            })
""",
        "world term",
    )
    p.write_text(t)


def patch_new_rule(root):
    p = root / "crates/gordian-run/src/policy/decide.rs"
    t = p.read_text()
    t = sub(
        t,
        "/// The name of this rule, as [`crate::policy::Policy::decision_rule`] reports it.\n",
        """/// DIAGNOSTIC PATCH: the components the current episode's directives fail (bit per component).
pub static DIAG_FAILED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// DIAGNOSTIC PATCH: whether the mirror variant `channel` applies to component `id` now.
fn diag_mirror(channel: &str, id: ComponentId) -> bool {
    let mode = std::env::var("GORDIAN_DIAG_MIRROR").unwrap_or_default();
    (mode == "both" || mode == channel)
        && DIAG_FAILED.load(std::sync::atomic::Ordering::Relaxed) & (1u32 << id.0) != 0
}

fn diag_slot_id(index: usize) -> ComponentId {
    match index {
        0 => VERIFIER_ID,
        1 => ESTIMATOR_ID,
        _ => HEURISTIC_ID,
    }
}

/// The name of this rule, as [`crate::policy::Policy::decision_rule`] reports it.
""",
        "statics",
    )
    t = sub(
        t,
        "            ops.add(R_COMPARED_BYTES, bytes);\n            self.last.compared_bytes += bytes;\n",
        """            ops.add(R_COMPARED_BYTES, bytes);
            if !diag_mirror("cost", id) {
                self.last.compared_bytes += bytes;
            }
""",
        "compare",
    )
    t = sub(
        t,
        "        if !output.entries.is_empty() {\n            self.last.decoded_outputs += 1;",
        "        if !output.entries.is_empty() && !diag_mirror(\"cost\", id) {\n            self.last.decoded_outputs += 1;",
        "decode",
    )
    t = sub(
        t,
        "                self.last.narrowed_worlds += ops.count(R_WORLDS) - before;\n",
        """                if !diag_mirror("cost", diag_slot_id(index)) {
                    self.last.narrowed_worlds += ops.count(R_WORLDS) - before;
                }
""",
        "narrow",
    )
    t = sub(
        t,
        "            self.last.scored_worlds += ops.count(R_WORLDS) - worlds;\n            self.last.probe_evals += ops.count(R_PROBE_EVALS) - evals;\n",
        """            if !diag_mirror("cost", diag_slot_id(source)) {
                self.last.scored_worlds += ops.count(R_WORLDS) - worlds;
                self.last.probe_evals += ops.count(R_PROBE_EVALS) - evals;
            }
""",
        "score",
    )
    t = sub(
        t,
        "        let action = self.decide_counted(state, outputs, last, &mut ops);\n",
        """        let mut action = self.decide_counted(state, outputs, last, &mut ops);
        // DIAGNOSTIC PATCH: the priority channel: decide as if the failed components' sets were absent.
        let mut shadow = self.clone();
        let mut any = false;
        if diag_mirror("priority", VERIFIER_ID) {
            shadow.verifier = None;
            any = true;
        }
        if diag_mirror("priority", ESTIMATOR_ID) {
            shadow.estimator = None;
            any = true;
        }
        if diag_mirror("priority", HEURISTIC_ID) {
            shadow.heuristic = None;
            any = true;
        }
        if any {
            action = shadow.decide_counted(state, &[], last, &mut RuleOps::ZERO);
        }
""",
        "priority",
    )
    p.write_text(t)


DIRECTIVES_EXAMPLE = """//! SCRATCH (not in the repository): list the harness directives of ComponentTimeout episodes.
//! Public API only; no hidden-state feature.
use gordian_world::{EpisodeClass, EpisodeSpec, generate};

fn main() {
    println!("seed\\tdirectives");
    for seed in 1000..1500u64 {
        let ep = generate(&EpisodeSpec::new(seed, EpisodeClass::ComponentTimeout));
        let d: Vec<String> = ep
            .harness_directives()
            .iter()
            .map(|d| format!("{}:{:?}", d.component, d.mode))
            .collect();
        println!("{}\\t{}", seed, d.join(";"));
    }
}
"""


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = [a for a in sys.argv[1:] if a.startswith("--")]
    root = pathlib.Path(args[0])
    new = "--tree=new" in flags
    # the example `a6d_explain.py` reads the failed components from (the old patch carried it too)
    (root / "crates/gordian-world/examples/directives.rs").write_text(DIRECTIVES_EXAMPLE)
    patch_harness(root, mask=new)
    if new:
        patch_new_rule(root)
    else:
        patch_old_rule(root)


if __name__ == "__main__":
    main()
