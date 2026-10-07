"""Hand mutants of the memory measures (B1's standard: cargo-mutants does not change constants,
swap tiers or reorder checks). Each mutant is one edit of `crates/gordian-stream-eval/src/memory.rs`;
the crate's whole test suite is run after it (through the isolation runner), the source is
restored, and the first failing tests are recorded. A mutant that no test fails is a survivor.

Usage: e1_hand_mutants.py   (needs a clean `memory.rs`; writes experiments/exploration/e1-hand-mutants.csv)
"""

import csv
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[3]
SRC = ROOT / "crates/gordian-stream-eval/src/memory.rs"

# (name, old, new, which occurrence: 1-based, or 0 for all)
#
# Two mutants of a first run were not killed and are recorded in the CSV of that run
# (`e1-hand-mutants-first-run.csv`): "an incident is its own earlier incident" (`..=i`), which is
# equivalent (an incident is never at another site than itself, so it cannot make itself true), kept
# below as a documented equivalent; and "background is not an incident, wrongly (declarations)", a
# dead computation of `correct` for a declaration about background, which the code no longer makes.
MUTANTS = [
    ("unasked correct needs only a correct declaration", "let unasked_correct = a.correct > 0 && a.escalations == 0;", "let unasked_correct = a.correct > 0;", 1),
    ("unasked correct tolerates one escalation", "let unasked_correct = a.correct > 0 && a.escalations == 0;", "let unasked_correct = a.correct > 0 && a.escalations <= 1;", 1),
    ("unasked wrong reads the correct count", "let unasked_wrong = a.wrong > 0 && a.escalations == 0;", "let unasked_wrong = a.correct > 0 && a.escalations == 0;", 1),
    ("stale wrong counts correct recalls", "let stale_wrong = a.cells.wrong() > 0 && a.escalations == 0;", "let stale_wrong = a.cells.total() > 0 && a.escalations == 0;", 1),
    ("stale wrong ignores escalations", "let stale_wrong = a.cells.wrong() > 0 && a.escalations == 0;", "let stale_wrong = a.cells.wrong() > 0;", 1),
    ("same family at the same site counts", "site_of(earlier).is_some_and(|s| s != site)", "site_of(earlier).is_some_and(|s| s == site)", 1),
    ("same family ignores the mode", "class_of(earlier) == Some(class)", "class_of(earlier).map(|c| c.0) == Some(class.0)", 1),
    ("an incident is its own earlier incident", "truth.incidents[..i].iter()", "truth.incidents[..=i].iter()", 1),
    ("the site is the last occupied service", "inc.occupies.first().map(|s| s.0)", "inc.occupies.last().map(|s| s.0)", 1),
    ("a right source is a different one", "if source.diagnosis == stored_truth {", "if source.diagnosis != stored_truth {", 1),
    ("an earlier source is judged in this stream", "SourceTruth::Earlier { truth } => truth,", "SourceTruth::Earlier { .. } => truth_of_obs(truth, source.obs).flatten(),", 1),
    ("background is not an incident, wrongly (recalls)", "None => diagnosis.is_none(),", "None => diagnosis.is_some(),", 1),
    ("a decoy's recall is counted on plain", "Some(Tier::Decoy) => totals.recalls_on_decoy.add(correct, class),", "Some(Tier::Decoy) => totals.recalls_on_plain.add(correct, class),", 1),
    ("the elsewhere population includes recurrences", "let elsewhere = !recurrence && *same_family;", "let elsewhere = *same_family;", 1),
    ("the reachable population needs both", "let reachable = recurrence || *same_family;", "let reachable = recurrence && *same_family;", 1),
    ("a decoy's unasked correct is counted as plain", "Tier::Decoy => counts.decoy += 1,", "Tier::Decoy => counts.plain += 1,", 1),
    ("a repeated recall step is allowed", "return Err(MemoryError::StepRepeated { step: r.step });", "{}", 1),
    ("a recall of a refused step is allowed", "return Err(MemoryError::NotADeclaration { step: r.step });", "{}", 1),
    ("the collision cell is the inherited one", "(false, SourceClass::Right) => &mut self.wrong_source_right,", "(false, SourceClass::Right) => &mut self.wrong_source_wrong,", 1),
    ("an unknown source is a right one", "None => SourceClass::Unknown,", "None => SourceClass::Right,", 1),
    ("an escalation about a background focus counts for the last incident", "if let Some(id) = truth.incident_of(*focus) {\n                    acc[id as usize].escalations += 1;\n                }", "acc[truth.incidents.len() - 1].escalations += 1;", 1),
]


def nth(text, old, k):
    idx = -1
    for _ in range(k):
        idx = text.find(old, idx + 1)
        if idx < 0:
            return -1
    return idx


def main():
    original = SRC.read_text()
    rows = []
    try:
        for name, old, new, k in MUTANTS:
            idx = nth(original, old, k)
            if idx < 0:
                rows.append({"mutant": name, "result": "NOT APPLIED", "failing_tests": ""})
                continue
            SRC.write_text(original[:idx] + new + original[idx + len(old):])
            p = subprocess.run(
                [str(ROOT / "scripts/cgroup-run.sh"), "--name", "memory-measures-build", "--cpus", "0-2", "--memory", "3G", "--",
                 "cargo", "test", "-p", "gordian-stream-eval", "--no-fail-fast"],
                cwd=ROOT, capture_output=True, text=True,
                env={"PATH": "/root/.cargo/bin:/usr/bin:/bin", "CARGO_BUILD_JOBS": "3", "CARGO_PROFILE_DEV_DEBUG": "0", "HOME": "/root"})
            out = p.stdout + p.stderr
            if "error[E" in out or "could not compile" in out:
                rows.append({"mutant": name, "result": "DOES NOT COMPILE", "failing_tests": ""})
                continue
            failing = re.findall(r"^test (\S+) \.\.\. FAILED", out, flags=re.M)
            rows.append({"mutant": name, "result": "caught" if failing else "SURVIVED", "failing_tests": "; ".join(failing[:3])})
            print(rows[-1], flush=True)
    finally:
        SRC.write_text(original)
    with open(ROOT / "experiments/exploration/e1-hand-mutants.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, ["mutant", "result", "failing_tests"])
        w.writeheader()
        w.writerows(rows)
    caught = sum(r["result"] == "caught" for r in rows)
    print(f"{caught} of {len(rows)} caught")


if __name__ == "__main__":
    main()
