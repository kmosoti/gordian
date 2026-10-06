"""Hand-applied mutants of B4's code: the selection accounting (`crates/gordian-stream-eval/src/select.rs`,
rules E1 to E8), the follow-up rule (`noticer_follow.rs`) and the two public selectors
(`public_threshold.rs`, `public_change.rs`).

cargo-mutants does not drop a call, reorder two statements or swap two names. Each mutant here is one
textual change, applied to the source one at a time, with the tests that pin that code run after it and
the source restored. A mutant is caught when a test fails (or the crate no longer builds, reported as
`unviable`). Usage: b4_hand_mutants.py   (writes experiments/exploration/b4-hand-mutants.csv). Must run with
no other build in this worktree (it rewrites source while it runs); every cargo call goes through the
scratch wrapper (`B4_CARGO`), which waits for another worker's cargo, rustc or gordian-run and runs under
`scripts/cgroup-run.sh` on cores 0-2.
"""

import csv
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CG = pathlib.Path(os.environ["B4_CARGO"])

EVAL = ["-p", "gordian-stream-eval", "--test", "selection_fixtures", "--test", "selection_properties"]
RUN = ["-p", "gordian-run", "--test", "stream_follow", "--test", "stream_b4"]

SELECT = "crates/gordian-stream-eval/src/select.rs"
FOLLOW = "crates/gordian-run/src/stream/arms/noticer_follow.rs"
THRESHOLD = "crates/gordian-run/src/stream/arms/public_threshold.rs"
CHANGE = "crates/gordian-run/src/stream/arms/public_change.rs"
HARNESS = "crates/gordian-run/src/stream/harness.rs"
RESULTS = "crates/gordian-run/src/stream/results.rs"

MUTANTS = [
    # E1: which notice a call is about
    (SELECT, EVAL, "a call before the notice is about it", "n.at <= e.at", "n.at <= e.at || true"),
    (SELECT, EVAL, "a notice made at the call's own instant is not about it", "n.at <= e.at", "n.at < e.at"),
    (SELECT, EVAL, "a retirement at the call's own instant comes before it", "is_none_or(|r| r >= e.at)", "is_none_or(|r| r > e.at)"),
    (SELECT, EVAL, "a retired notice is still asked about", "is_none_or(|r| r >= e.at)", "is_none_or(|_| true)"),
    (SELECT, EVAL, "the first notice at the focus, not the latest", ".enumerate()\n            .rev()\n            .find(", ".enumerate()\n            .find("),
    (SELECT, EVAL, "the anchor need not be the focus", "n.anchor == e.focus\n                    &&", "true\n                    &&"),
    (SELECT, EVAL, "the first call's instant is the latest", "f.min(e.at)", "f.max(e.at)"),
    (SELECT, EVAL, "an unattributed call is not counted", "None => escalations.unattributed += 1,", "None => {}"),
    # E2: classes and cost
    (SELECT, EVAL, "tokens are the modelled nanoseconds", "*escalations.tokens.slot(class) += e.tokens;", "*escalations.tokens.slot(class) += e.modelled_ns;"),
    (SELECT, EVAL, "modelled nanoseconds are the tokens", "*escalations.modelled_ns.slot(class) += e.modelled_ns;", "*escalations.modelled_ns.slot(class) += e.tokens;"),
    (SELECT, EVAL, "a call is counted twice", "*escalations.calls.slot(class) += 1;", "*escalations.calls.slot(class) += 2;"),
    (SELECT, EVAL, "a slow leak is a hard incident", "Tier::Hard if inc.shape.hard_kind == Some(HardKind::SlowLeak) => IncidentClass::Leak,", "Tier::Hard if inc.shape.hard_kind == Some(HardKind::Compound) => IncidentClass::Leak,"),
    (SELECT, EVAL, "a decoy is plain", "Tier::Decoy => IncidentClass::Decoy,", "Tier::Decoy => IncidentClass::Plain,"),
    (SELECT, EVAL, "an incident of no label is plain", "return Ok((None, IncidentClass::Background));", "return Ok((None, IncidentClass::Plain));"),
    (SELECT, EVAL, "an observation past the end is allowed", "if (obs.0 as usize) >= truth.labels.len() {", "if (obs.0 as usize) > truth.labels.len() {"),
    # E3 to E5, E7
    (SELECT, EVAL, "retired before escalation: retired or never asked", "o.retired_at.is_some() && o.escalations == 0", "o.retired_at.is_some() || o.escalations == 0"),
    (SELECT, EVAL, "retired before escalation: never asked", "o.retired_at.is_some() && o.escalations == 0", "o.escalations == 0"),
    (SELECT, EVAL, "a retirement by the rule is read from the first retirement only", "per_notice[pos].followup = r.followup;", "per_notice[pos].followup = false;"),
    (SELECT, EVAL, "a second retirement is allowed", "if per_notice[pos].retired_at.is_some() {\n            return Err(SelectionError::RetirementWithoutNotice { anomaly: r.anomaly });\n        }", ""),
    (SELECT, EVAL, "a rule retirement before escalation is any rule retirement", "            if o.retired_before_escalation {\n                *notices.followup_before_escalation", "            if true {\n                *notices.followup_before_escalation"),
    (SELECT, EVAL, "escalated counts every call", "if o.escalations > 0 {\n            *notices.escalated", "if o.escalations > 1 {\n            *notices.escalated"),
    # the follow-up rule
    (FOLLOW, RUN, "a fall of exactly max_fall is a reversal", "seen.peak.saturating_sub(seen.last) > u64::from(self.max_fall)", "seen.peak.saturating_sub(seen.last) >= u64::from(self.max_fall)"),
    (FOLLOW, RUN, "a reversal needs no follow-up reading", "if seen.readings >= 1 && seen.peak", "if seen.peak"),
    (FOLLOW, RUN, "the window is one reading longer", "if seen.readings >= self.readings {", "if seen.readings > self.readings {"),
    (FOLLOW, RUN, "min_gain is exclusive", "seen.last >= seen.level0.saturating_add(u64::from(self.min_gain))", "seen.last > seen.level0.saturating_add(u64::from(self.min_gain))"),
    (FOLLOW, RUN, "no reading keeps a flat counter", "if seen.readings >= 1 && seen.last >=", "if seen.last >="),
    (FOLLOW, RUN, "the horizon is exclusive", "now.0 >= w.t0.0.saturating_add(spec.horizon_ns)", "now.0 > w.t0.0.saturating_add(spec.horizon_ns)"),
    (FOLLOW, RUN, "a withdrawal is a keep", "if *v == Verdict::Withdraw {", "if *v == Verdict::Keep {"),
    (FOLLOW, RUN, "the peak is the lowest value", "w.seen.peak = w.seen.peak.max(value);", "w.seen.peak = w.seen.peak.min(value);"),
    (FOLLOW, RUN, "the peak starts at zero", "                peak: level0,", "                peak: 0,"),
    (FOLLOW, RUN, "every key's readings are follow-up readings", "for w in self.watches.iter_mut().filter(|w| w.key == key) {", "for w in self.watches.iter_mut() {"),
    (FOLLOW, RUN, "a withdrawal is never forgotten", "self.withdrawn.retain(|a| live(*a));", ""),
    (FOLLOW, RUN, "a watch of a gone anomaly is kept", "self.watches.retain(|w| live(w.anomaly));", ""),
    (FOLLOW, RUN, "the last reading is the first", "w.seen.last = value;", ""),
    # the selectors
    (THRESHOLD, RUN, "contradictory and silent, not or", "contradictory || silent", "contradictory && silent"),
    (THRESHOLD, RUN, "a declared rung is silent", "let silent = !view.cheap_declared", "let silent = view.cheap_declared"),
    (THRESHOLD, RUN, "the contradiction need not last", "now.0 >= since.0.saturating_add(self.persist_ns)", "now.0 >= since.0"),
    (THRESHOLD, RUN, "silent counts from the start", "now.0 >= view.noticed_at.0.saturating_add(self.persist_ns)", "true"),
    (THRESHOLD, RUN, "an asked anomaly is asked again", "v.attempts == 0\n                    &&", "true\n                    &&"),
    (THRESHOLD, RUN, "the delay is exclusive", "now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)", "now.0 > v.noticed_at.0.saturating_add(self.delay_ns)"),
    (CHANGE, RUN, "growth by k is exclusive", "v.evidence >= base.saturating_add(self.k)", "v.evidence > base.saturating_add(self.k)"),
    (CHANGE, RUN, "the baseline is zero", "or_insert(v.evidence)", "or_insert(0)"),
    (CHANGE, RUN, "k of zero asks at notice", "k: k.max(1),", "k,"),
    (CHANGE, RUN, "an asked anomaly is asked again", "if v.attempts == 0\n                && ", "if "),
    (CHANGE, RUN, "the delay is exclusive", "now.0 >= v.noticed_at.0.saturating_add(self.delay_ns)", "now.0 > v.noticed_at.0.saturating_add(self.delay_ns)"),
    # the harness and the files
    (HARNESS, RUN, "calls are stamped with the instant they were applied at", "st.escalation_steps.push(st.step_at);", "st.escalation_steps.push(st.clock.now());"),
    (HARNESS, RUN, "a retirement by the rule is not read from the log", "followup: e.cause == Some(RetireCause::Followup),", "followup: false,"),
    (RESULTS, RUN, "the cause is quiet for a rule retirement", "(Some(_), true) => \"followup\",", "(Some(_), true) => \"quiet\","),
]


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
            rows.append({"file": path, "mutant": label, "result": result,
                         "first_failing_tests": "; ".join(failed[:3])})
            print(rows[-1], flush=True)
    finally:
        for path, text in originals.items():
            (ROOT / path).write_text(text)
    out = ROOT / "experiments/exploration/b4-hand-mutants.csv"
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["file", "mutant", "result", "first_failing_tests"], lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    bad = [r for r in rows if r["result"] not in ("caught", "unviable")]
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
