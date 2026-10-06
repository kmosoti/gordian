"""Hand-applied mutants of the notice scorer (crates/gordian-stream-eval/src/notice.rs).

cargo-mutants does not change constants, drop a call or reorder checks. Each mutant here is one
textual change, applied to the source one at a time, with the crate's notice tests run after it and
the source restored. A mutant is caught when a test fails (or the crate no longer builds, reported
as `unviable`). Usage: b1_hand_mutants.py   (writes experiments/exploration/b1-hand-mutants.csv)
"""

import csv
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
SRC = ROOT / "crates/gordian-stream-eval/src/notice.rs"
CG = pathlib.Path("/tmp/claude-0/-home-user-gordian/ca6a76a5-f580-525e-b0d3-133df4b5ff51/scratchpad/cg.sh")

MUTANTS = [
    ("window is 2 s", "pub const ANCHOR_WINDOW_NS: u64 = 1_000_000_000;", "pub const ANCHOR_WINDOW_NS: u64 = 2_000_000_000;"),
    ("window is 0.5 s", "pub const ANCHOR_WINDOW_NS: u64 = 1_000_000_000;", "pub const ANCHOR_WINDOW_NS: u64 = 500_000_000;"),
    ("window exclusive", "offset.is_some_and(|o| o <= ANCHOR_WINDOW_NS)", "offset.is_some_and(|o| o < ANCHOR_WINDOW_NS)"),
    ("anchor-correct needs no incident offset (always true for an incident)", "offset.is_some_and(|o| o <= ANCHOR_WINDOW_NS)", "offset.is_some()"),
    ("anchor-correct not accumulated per incident (last notice decides)", "inc.anchor_correct |= correct;", "inc.anchor_correct = correct;"),
    ("offset from the incident's last observation", ".first()\n                .and_then(|o| obs_at.get(o.0 as usize).copied())", ".last()\n                .and_then(|o| obs_at.get(o.0 as usize).copied())"),
    ("latency from the anchor, not the first observation", "Some(notice.0.saturating_sub(first.0))", "Some(notice.0.saturating_sub(first.0).saturating_sub(0)).map(|l| l.min(1))"),
    ("first notice is the last notice", "inc.first_notice_at.get_or_insert(n.at);", "inc.first_notice_at = Some(n.at);"),
    ("decoy notices counted as plain", "Tier::Decoy => totals.on_decoy += 1,", "Tier::Decoy => totals.on_plain += 1,"),
    ("hard notices counted as plain", "Tier::Hard => totals.on_hard += 1,", "Tier::Hard => totals.on_plain += 1,"),
    ("background notices not counted", "totals.on_background += 1;", "totals.on_background += 0;"),
    ("notices per incident not counted", "inc.notices += 1;", "inc.notices = 1;"),
    ("noticed only when anchor-correct", "inc.noticed = true;", "inc.noticed = inc.anchor_correct;"),
    ("retirements not counted", "retirements: trace.retirements.len() as u32,", "retirements: 0,"),
    ("a notice before its anchor is accepted", "if n.at < anchor_at {", "if n.at < Instant(anchor_at.0.saturating_sub(u64::MAX / 2)) {"),
    ("notices may go back in time", "if previous.is_some_and(|p| n.at < p) {\n            return Err(NoticeEvalError::NoticeTimeWentBackwards", "if false && previous.is_some_and(|p| n.at < p) {\n            return Err(NoticeEvalError::NoticeTimeWentBackwards"),
    ("an anomaly may be noticed twice", "if noticed_at.insert(n.anomaly, n.at).is_some() {", "if noticed_at.insert(n.anomaly, n.at).is_some() && false {"),
    ("a retirement may precede its notice", "if r.at < at {", "if r.at < Instant(at.0.saturating_sub(u64::MAX / 2)) {"),
    ("retirements may go back in time", "if previous.is_some_and(|p| r.at < p) {\n            return Err(NoticeEvalError::RetirementTimeWentBackwards", "if false && previous.is_some_and(|p| r.at < p) {\n            return Err(NoticeEvalError::RetirementTimeWentBackwards"),
    ("a retired anomaly stays live", "let Some(at) = live.remove(&r.anomaly) else {", "let Some(at) = live.get(&r.anomaly).copied() else {"),
    ("instants not checked against labels", "if obs_at.len() != truth.labels.len() {", "if false && obs_at.len() != truth.labels.len() {"),
    ("incident ids not checked", "if inc.id as usize != index {", "if false && inc.id as usize != index {"),
    ("labels of unknown incidents not checked", "&& (*id as usize) >= truth.incidents.len()", "&& (*id as usize) > truth.incidents.len() + 1000"),
    ("per-notice tier dropped", "tier: Some(inc.tier),", "tier: None,"),
    ("an incident with no observation is noticed by a background anchor", "let owner = truth.incident_of(n.anchor);", "let owner = truth.incident_of(n.anchor).or(if truth.incidents.is_empty() { None } else { Some(0) });"),
]


def run_tests():
    p = subprocess.run(["bash", str(CG), "test", "--locked", "-p", "gordian-stream-eval", "--test", "notice_fixtures"],
                       capture_output=True, text=True)
    out = p.stdout + p.stderr
    failed = re.findall(r"^test (\S+) \.\.\. FAILED", out, flags=re.M)
    if p.returncode == 0:
        return "survived", []
    if "could not compile" in out or "error[E" in out:
        return "unviable", []
    return "caught", failed


def main():
    original = SRC.read_text()
    rows = []
    try:
        for label, old, new in MUTANTS:
            if original.count(old) != 1:
                rows.append({"mutant": label, "result": "not-applied", "first_failing_tests": f"pattern occurs {original.count(old)} times"})
                continue
            SRC.write_text(original.replace(old, new))
            result, failed = run_tests()
            rows.append({"mutant": label, "result": result, "first_failing_tests": "; ".join(failed[:3])})
            print(rows[-1], flush=True)
    finally:
        SRC.write_text(original)
    out = ROOT / "experiments/exploration/b1-hand-mutants.csv"
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["mutant", "result", "first_failing_tests"], lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    bad = [r for r in rows if r["result"] not in ("caught", "unviable")]
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
