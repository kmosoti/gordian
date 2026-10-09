"""Hand-applied mutants of B5's code: the budgeted selector (`public_budgeted.rs`), the new view field
(`rung.rs`, `noticer.rs`) and the selector's spelling and construction (`spec.rs`).

cargo-mutants does not drop a call, reorder two statements or swap two names. Each mutant here is one
textual change, applied to the source one at a time, with the tests that pin that code run after it and
the source restored. A mutant is caught when a test fails (or the crate no longer builds, reported as
`unviable`). Usage: b5_hand_mutants.py   (writes experiments/exploration/b5-hand-mutants.csv). Must run with
no other build in this worktree (it rewrites source while it runs); every cargo call goes through the
scratch wrapper (`B5_CARGO`), which waits for another worker's cargo, rustc or gordian-run and runs under
`scripts/cgroup-run.sh` on cores 0-2.

A mutant that no test can tell from the original is an equivalent mutant and is listed as such with the
argument (`EQUIVALENT` below), never dropped silently.
"""

import csv
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
CG = pathlib.Path(os.environ["B5_CARGO"])

RUN = ["-p", "gordian-run", "--test", "stream_b5", "--test", "stream_b5_features"]

SEL = "crates/gordian-run/src/stream/arms/public_budgeted.rs"
RUNG = "crates/gordian-run/src/stream/arms/rung.rs"
NOTICER = "crates/gordian-run/src/stream/arms/noticer.rs"
SPEC = "crates/gordian-run/src/stream/spec.rs"

READY = ("v.attempts == 0\n                && now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)\n"
         "                && self.seen.insert(v.id)")

MUTANTS = [
    # which anomalies, when
    (SEL, RUN, "the delay is exclusive", "now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)",
     "now.0 > v.noticed_at.0.saturating_add(self.delay_ns)"),
    (SEL, RUN, "the delay is ignored", "now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)", "true"),
    (SEL, RUN, "an asked anomaly is asked again", "v.attempts == 0\n                && now.0", "now.0"),
    (SEL, RUN, "an anomaly is marked seen before it is ready", READY,
     "self.seen.insert(v.id)\n                && v.attempts == 0\n                && now.0 >= "
     "v.noticed_at.0.saturating_add(self.delay_ns)"),
    (SEL, RUN, "an anomaly is looked at at every step", "&& self.seen.insert(v.id)", "&& { self.seen.insert(v.id); true }"),
    # order and budget
    (SEL, RUN, "worst first", "b.0.total_cmp(&a.0).then(a.1.cmp(&b.1))", "a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))"),
    (SEL, RUN, "ties by descending id", "b.0.total_cmp(&a.0).then(a.1.cmp(&b.1))", "b.0.total_cmp(&a.0).then(b.1.cmp(&a.1))"),
    (SEL, RUN, "no order", "ready.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));", ""),
    (SEL, RUN, "the budget is one too many", "if self.spent >= self.k {", "if self.spent > self.k {"),
    (SEL, RUN, "the budget is one too few", "if self.spent >= self.k {", "if self.spent + 1 >= self.k {"),
    (SEL, RUN, "the budget is never spent", "                self.spent += 1;\n", ""),
    (SEL, RUN, "a spent budget only stops the step", "            if self.spent >= self.k {\n                break;\n            }\n", ""),
    (SEL, RUN, "the threshold is exclusive", "if score >= self.score.threshold {", "if score > self.score.threshold {"),
    (SEL, RUN, "the threshold is ignored", "if score >= self.score.threshold {", "if true {"),
    (SEL, RUN, "an anomaly below the threshold costs budget", "            if score >= self.score.threshold {\n                self.spent += 1;\n                out.push(id);\n            }",
     "            self.spent += 1;\n            if score >= self.score.threshold {\n                out.push(id);\n            }"),
    (SEL, RUN, "the checker is not kept current", "fn monitors(&self) -> bool {\n        true", "fn monitors(&self) -> bool {\n        false"),
    # the features
    (SEL, RUN, "contradiction is the absence of a verdict", "u8::from(view.contradicted_since.is_some())", "u8::from(view.contradicted_since.is_none())"),
    (SEL, RUN, "silence is a declaration", "u8::from(!view.cheap_declared)", "u8::from(view.cheap_declared)"),
    (SEL, RUN, "evidence is not on a log scale", "clip((1.0 + f64::from(view.evidence)).ln() / (1.0 + EVIDENCE_CAP).ln())", "clip(f64::from(view.evidence) / EVIDENCE_CAP)"),
    (SEL, RUN, "evidence is not shifted", "(1.0 + f64::from(view.evidence)).ln() / (1.0 + EVIDENCE_CAP).ln()", "(1.0 + f64::from(view.evidence)).ln() / EVIDENCE_CAP.ln()"),
    (SEL, RUN, "evidence is not clipped", "clip((1.0 + f64::from(view.evidence)).ln() / (1.0 + EVIDENCE_CAP).ln())", "(1.0 + f64::from(view.evidence)).ln() / (1.0 + EVIDENCE_CAP).ln()"),
    (SEL, RUN, "services are over the evidence cap", "f64::from(view.services) / SERVICES_CAP", "f64::from(view.services) / EVIDENCE_CAP"),
    (SEL, RUN, "services are the evidence", "clip(f64::from(view.services) / SERVICES_CAP)", "clip(f64::from(view.evidence) / SERVICES_CAP)"),
    (SEL, RUN, "age is from the notice", "now.0.saturating_sub(view.anchor_at.0) as f64 / AGE_CAP_NS", "now.0.saturating_sub(view.noticed_at.0) as f64 / AGE_CAP_NS"),
    (SEL, RUN, "age wraps before the anchor", "now.0.saturating_sub(view.anchor_at.0) as f64", "now.0.wrapping_sub(view.anchor_at.0) as f64"),
    (SEL, RUN, "age is not clipped", "age: clip(now.0", "age: (now.0"),
    (SEL, RUN, "the score drops the age term", "\n            + self.age * f.age\n", "\n"),
    (SEL, RUN, "the score drops the contradiction term", "self.contradiction * f.contradiction\n            + ", ""),
    (SEL, RUN, "the services weight has the wrong sign", "+ self.services * f.services", "- self.services * f.services"),
    (SEL, RUN, "the silence weight is the contradiction's", "+ self.silence * f.silence", "+ self.contradiction * f.silence"),
    (SEL, RUN, "a non-finite weight passes", "        .all(|x| x.is_finite())", "        .all(|_| true)"),
    # the new view field
    (RUNG, RUN, "the view's services are its observations", "services: u32::try_from(a.service_count()).unwrap_or(u32::MAX),", "services: u32::try_from(a.attached.len()).unwrap_or(u32::MAX),"),
    (RUNG, RUN, "the view's services are always one", "services: u32::try_from(a.service_count()).unwrap_or(u32::MAX),", "services: 1,"),
    (NOTICER, RUN, "the count is of observations", "pub fn service_count(&self) -> usize {\n        self.services.len()", "pub fn service_count(&self) -> usize {\n        self.attached.len()"),
    (NOTICER, RUN, "the count is one too many", "pub fn service_count(&self) -> usize {\n        self.services.len()", "pub fn service_count(&self) -> usize {\n        self.services.len() + 1"),
    # the spelling and the construction
    (SPEC, RUN, "k of zero is accepted", "Self::PublicBudgeted { k: 0, .. } => {", "Self::PublicBudgeted { k: 4_000_000_000, .. } => {"),
    (SPEC, RUN, "a non-finite score is accepted", "Self::PublicBudgeted { score, .. } if !score.is_finite() => {", "Self::PublicBudgeted { score, .. } if false && !score.is_finite() => {"),
    (SPEC, RUN, "the score is not read", "let spec = Self::PublicBudgeted { delay_ns, k, score };", "let spec = Self::PublicBudgeted { delay_ns, k, score: Score::default() };"),
    (SPEC, RUN, "the score is not written", "score: Some(*score),", "score: None,"),
    (SPEC, RUN, "the budget is not written", "Self::PublicBudgeted { delay_ns, k, score } => Tagged {\n                k: Some(*k),", "Self::PublicBudgeted { delay_ns, k, score } => Tagged {\n                k: Some(*k + 1),"),
    (SPEC, RUN, "the delay is not built", "PublicBudgeted::new(*delay_ns, *k, *score)", "PublicBudgeted::new(0, *k, *score)"),
    (SPEC, RUN, "the budget is not built", "PublicBudgeted::new(*delay_ns, *k, *score)", "PublicBudgeted::new(*delay_ns, *k + 1, *score)"),
    (SPEC, RUN, "k is optional", "k: k.ok_or_else(|| \"policy \\\"public_budgeted\\\" needs k\".to_owned())?,", "k: k.unwrap_or(1),"),
    (SPEC, RUN, "the arm takes another arm's parameter", "public_budgeted::ID => {\n                only(&[\"delay_ns\", \"k\"])?;", "public_budgeted::ID => {\n                only(&[\"delay_ns\", \"k\", \"persist_ns\"])?;"),
    (SPEC, RUN, "any policy takes a score", "(_, Some(_)) => Err(format!(\"policy {:?} has no parameter \\\"score\\\"\", t.policy)),", "(spec, Some(_)) => Ok(spec),"),
    (SPEC, RUN, "the arm is not known", "    public_change::ID,\n    public_budgeted::ID,", "    public_change::ID,"),
]

# Mutants no test can tell from the original, with the argument, listed in the output as `equivalent`.
EQUIVALENT = {}


def run_tests(tests):
    p = subprocess.run([str(CG), "cargo", "test", "--locked", *tests], capture_output=True, text=True)
    out = p.stdout + p.stderr
    failed = re.findall(r"^test (\S+) \.\.\. FAILED", out, flags=re.M)
    if p.returncode == 0:
        return "survived", []
    if "could not compile" in out or "error[E" in out:
        return "unviable", []
    return "caught", failed


def main():
    rows = []
    originals = {}
    try:
        for path, tests, label, old, new in MUTANTS:
            src = ROOT / path
            originals.setdefault(path, src.read_text())
            original = originals[path]
            if original.count(old) != 1:
                rows.append({"file": path, "mutant": label, "result": "not-applied",
                             "first_failing_tests": f"pattern occurs {original.count(old)} times"})
                print(rows[-1], flush=True)
                continue
            src.write_text(original.replace(old, new))
            result, failed = run_tests(tests)
            src.write_text(original)
            if result == "survived" and label in EQUIVALENT:
                result = "equivalent"
            rows.append({"file": path, "mutant": label, "result": result,
                         "first_failing_tests": "; ".join(failed[:3])})
            print(rows[-1], flush=True)
    finally:
        for path, text in originals.items():
            (ROOT / path).write_text(text)
    out = ROOT / "experiments/exploration/b5-hand-mutants.csv"
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["file", "mutant", "result", "first_failing_tests"], lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    bad = [r for r in rows if r["result"] not in ("caught", "unviable", "equivalent")]
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
