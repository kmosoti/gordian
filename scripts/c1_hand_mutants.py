"""Hand mutants of the dataflow noticer: does the test file notice a rule changed?

Usage: python3 -I scripts/c1_hand_mutants.py WAIT_SCRIPT OUT.csv

For each mutant (one textual change to one rule or one piece of the relations' upkeep) the script
applies it, runs `cargo test -p gordian-run --test stream_dataflow` (everything but the 200-stream
held-out reproduction, which the other tests make redundant here) under the build runner after the
core-sharing wait (`WAIT_SCRIPT LABEL -- COMMAND`), records whether any test failed, and restores the
file with `git checkout -- FILE`. A mutant that no test fails is reported as missed, with no
conclusion drawn about whether it is equivalent. The tree is checked clean at the end.
"""
import csv
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
D = "crates/gordian-run/src/stream/arms/dataflow"
RULES, PROG, NOTI = f"{D}/rules.rs", f"{D}/program.rs", f"{D}/noticing.rs"

MUTANTS = [
    ("split: silence must exceed the gap (<=) becomes (<)", RULES, "if silence <= gap_ns {", "if silence < gap_ns {"),
    ("split: a cluster is complete after the burst window (<=) becomes (<)", RULES, "if now <= ready {", "if now < ready {"),
    ("split: the due instant is never kept", PROG, "if let Some(w) = due {", "if let Some(w) = due.filter(|_| false) {"),
    ("split: a changed anomaly is not marked for examination", PROG, "self.dirty.put(d.key.0, ());", "let _ = d;"),
    ("re-anchor: densest burst keeps the latest on a tie", RULES, "if d > best {", "if d >= best {"),
    ("move: the burst reopens at the old instant", RULES, "burst_open: anchor_at,", "burst_open: self.burst_open,"),
    ("retime: a burst opens only strictly after the gap", RULES, "r.at >= l.saturating_add(burst_gap_ns)", "r.at > l.saturating_add(burst_gap_ns)"),
    ("later re-anchor: a burst may begin at the anchor's instant", RULES, "rows[k].at > first.at", "rows[k].at >= first.at"),
    ("later re-anchor: skipped", PROG, "if let Some(re) = self.reanchor {", "if let Some(re) = self.reanchor.filter(|_| false) {"),
    ("chain: the latest chain wins a tie on readings", RULES, "pick.is_none_or(|p| c.count > chains[p].count)", "pick.is_none_or(|p| c.count >= chains[p].count)"),
    ("chain: eviction prefers the newest last reading", RULES, "(0..chains.len()).min_by_key(|&j| (chains[j].crossed, chains[j].count, chains[j].last_at))", "(0..chains.len()).min_by_key(|&j| (chains[j].crossed, chains[j].count, u64::MAX - chains[j].last_at))"),
    ("chain: a ramp needs the rise strictly above", RULES, "c.count >= min_readings && rise >= i128::from(min_rise)", "c.count >= min_readings && rise > i128::from(min_rise)"),
    ("chain: expiry at the gap itself", RULES, "at > c.last_at.saturating_add(gap_ns)", "at >= c.last_at.saturating_add(gap_ns)"),
    ("attach: speaking within three gaps", RULES, "burst_gap_ns.saturating_mul(2)", "burst_gap_ns.saturating_mul(3)"),
    ("attach: the lowest anomaly at a site, not the highest", PROG, "self.by_site.range((svc, 0)..=(svc, u32::MAX)).rev()", "self.by_site.range((svc, 0)..=(svc, u32::MAX))"),
    ("attach: propagation prefers the later id on a tie", RULES, "(burst_open, Reverse(id))", "(burst_open, Reverse(u32::MAX - id))"),
    ("adopt: a candidate is adopted if any row is the ramp's", PROG, ".all(|(_, m)| self.by_obs.contains(&(id, m.obs)));", ".any(|(_, m)| self.by_obs.contains(&(id, m.obs)));"),
    ("extend: a reading the anomaly holds is attached again", PROG, "if self.by_obs.contains(&(id, held.id.0)) {", "if false {"),
    ("forget: a stale candidate is forgotten at the window itself", PROG, "now.0 > a.timing.last_abnormal.saturating_add(self.score.window_ns)", "now.0 >= a.timing.last_abnormal.saturating_add(self.score.window_ns)"),
    ("score: the window includes its lower edge", PROG, "let from = now.saturating_sub(self.score.window_ns).saturating_add(1);", "let from = now.saturating_sub(self.score.window_ns);"),
    ("score: the abnormal count is not kept", PROG, "self.agg.put(0, seen);", "self.agg.put(0, 0);"),
    ("view: an append is applied in place even when rows left", NOTI, "Ok(i) => !rebuild && self.views[i].anchor.0 == a.anchor,", "Ok(i) => self.views[i].anchor.0 == a.anchor,"),
]


def run(cmd, check=False):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)


def main(wait, out):
    rows = []
    for n, (label, file, old, new) in enumerate(MUTANTS, 1):
        path = ROOT / file
        text = path.read_text()
        assert text.count(old) == 1, f"mutant {n}: {old!r} occurs {text.count(old)} times in {file}"
        path.write_text(text.replace(old, new))
        cmd = [wait, f"mutant{n}", "--", "scripts/cgroup-run.sh", "--name", "lab2-build", "--cpus", "0-2",
               "--memory", "3G", "--", "cargo", "test", "--offline", "-p", "gordian-run", "--test",
               "stream_dataflow", "--", "--skip", "held_out"]
        r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True,
                           env={**__import__("os").environ, "CARGO_BUILD_JOBS": "3", "CARGO_PROFILE_DEV_DEBUG": "0"})
        failed = [l.split()[1] for l in (r.stdout + r.stderr).splitlines() if l.startswith("test ") and l.endswith("FAILED")]
        compiled = "could not compile" not in r.stderr
        subprocess.run(["git", "checkout", "--", file], cwd=ROOT, check=True)
        status = "unviable (did not compile)" if not compiled else ("caught" if r.returncode != 0 else "MISSED")
        rows.append({"n": n, "mutant": label, "file": file, "status": status, "exit": r.returncode,
                     "failing_tests": ";".join(failed)})
        print(n, status, label, failed, flush=True)
    dirty = run(["git", "status", "--porcelain", "--untracked-files=no"]).stdout.strip()
    assert not dirty, f"tree not clean: {dirty}"
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
