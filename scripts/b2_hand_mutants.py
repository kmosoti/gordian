"""Hand-applied mutants of the site check and notice precision (rules N13 to N16 of the stream
evaluator, `crates/gordian-stream-eval/src/notice.rs`).

cargo-mutants does not change method names, drop a call or reorder checks. Each mutant here is one
textual change, applied to the source one at a time, with the notice tests of the crate run after it
and the source restored. A mutant is caught when a test fails (or the crate no longer builds, reported
as `unviable`). Usage: b2_hand_mutants.py   (writes experiments/exploration/b2-hand-mutants.csv).
Must run with no other build in this worktree (it rewrites the scorer's source while it runs); the
cargo wrapper it calls waits for another worker's `gordian-run` and pins cores 0-2.
"""

import csv
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SRC = ROOT / "crates/gordian-stream-eval/src/notice.rs"
CG = pathlib.Path(os.environ["B2_CARGO"])  # the scratch wrapper: waits for gordian-run, pins cores 0-2

MUTANTS = [
    ("the incident's site is the last service it occupies", "inc.occupies.first().map(|s| s.0)", "inc.occupies.last().map(|s| s.0)"),
    ("an incident that occupies nothing has site 0", "inc.occupies.first().map(|s| s.0)", "Some(inc.occupies.first().map_or(0, |s| s.0))"),
    ("a notice is read against the first incident's site", "incident_site(&truth.incidents[id as usize])", "incident_site(&truth.incidents[0])"),
    ("site-correct is not accumulated per incident (the last notice decides)", "inc.site_correct |= site_correct;", "inc.site_correct = site_correct;"),
    ("anchor-and-site-correct is not accumulated per incident (the last notice decides)", "inc.anchor_site_correct |= both;", "inc.anchor_site_correct = both;"),
    ("a missing site is right", "            _ => false,\n        };\n        inc.site_correct", "            _ => true,\n        };\n        inc.site_correct"),
    ("site compared with <=", "(Some(notice), Some(incident)) => notice == incident,", "(Some(notice), Some(incident)) => notice <= incident,"),
    ("both is anchor-correct or site-correct", "let both = correct && site_correct;", "let both = correct || site_correct;"),
    ("both is site-correct only", "let both = correct && site_correct;", "let both = site_correct;"),
    ("both is anchor-correct only", "let both = correct && site_correct;", "let both = correct;"),
    ("a notice on background is site-correct when it has a site", "                anchor_correct: false,\n                site_correct: false,", "                anchor_correct: false,\n                site_correct: n.site.is_some(),"),
    ("site-correct incidents are counted in the anchor-correct totals", "bump(&mut totals.site_correct, inc.tier);", "bump(&mut totals.anchor_correct, inc.tier);"),
    ("both incidents are counted in the site-correct totals", "bump(&mut totals.anchor_site_correct, inc.tier);", "bump(&mut totals.site_correct, inc.tier);"),
    ("site-correct notices counted by anchor-correctness", "totals.notices_site_correct += u32::from(site_correct);", "totals.notices_site_correct += u32::from(correct);"),
    ("both notices counted by site-correctness", "totals.notices_anchor_site_correct += u32::from(both);", "totals.notices_anchor_site_correct += u32::from(site_correct);"),
    ("notices on incidents omit decoys", "self.on_plain + self.on_hard + self.on_decoy", "self.on_plain + self.on_hard"),
    ("precision in the hard tier reads the plain count", "Tier::Hard => self.on_hard,", "Tier::Hard => self.on_plain,"),
    ("precision in the decoy tier reads the hard count", "Tier::Decoy => self.on_decoy,", "Tier::Decoy => self.on_hard,"),
    ("strict precision reads the site-correct count", "ratio(self.notices_anchor_site_correct, self.notices)", "ratio(self.notices_site_correct, self.notices)"),
    ("precision of a stream with no notice is zero, not none", "(den > 0).then(|| f64::from(num) / f64::from(den))", "Some(f64::from(num) / f64::from(den.max(1)))"),
]


def run_tests():
    p = subprocess.run(
        [str(CG), "cargo", "test", "--locked", "-p", "gordian-stream-eval", "--test", "notice_fixtures",
         "--test", "notice_generated"],
        capture_output=True, text=True,
    )
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
                rows.append({"mutant": label, "result": "not-applied",
                             "first_failing_tests": f"pattern occurs {original.count(old)} times"})
                print(rows[-1], flush=True)
                continue
            SRC.write_text(original.replace(old, new))
            result, failed = run_tests()
            rows.append({"mutant": label, "result": result, "first_failing_tests": "; ".join(failed[:3])})
            print(rows[-1], flush=True)
    finally:
        SRC.write_text(original)
    out = ROOT / "experiments/exploration/b2-hand-mutants.csv"
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=["mutant", "result", "first_failing_tests"], lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    bad = [r for r in rows if r["result"] not in ("caught", "unviable")]
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
