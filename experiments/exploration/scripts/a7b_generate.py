"""A7b step 1: regenerate per-episode modelled costs of B1's proxy pairs from the repository.

Work item A7b (ratio-interval calibration). Development run; nothing here tests a hypothesis.

B1's CSV (`b1-variance.csv`) holds only per-class means and sds, so the empirical per-episode
cost distribution is regenerated here: for each B1 compute level this writes one interleaved
manifest with the four arms the proxy pairs need, runs it with the release `gordian-run`
binary under `scripts/cgroup-run.sh` (the isolation runner, cores 0-2, 2 GiB), and then writes the
paired per-episode `modelled_cost_ns` of each proxy pair to one compact table.

    P1  A = heuristic_only            B = fixed_heuristic_every2   (the B2 cost proxy)
    P2  A = all_components            B = random_p050

Why not `scripts/run-driver.sh`: the driver refuses unless the manifest's `source_revision` is
`HEAD` of a clean tree, which cannot hold while the milestones of this item are committed, and it
adds core-3 pinning and the internal/external time check, which exist for measured wall time.
`modelled_cost_ns` is a deterministic count, so a run's timing does not enter it. The check
`--verify-b1` compares what was regenerated with `b1-variance.csv` (every modelled mean to the
CSV's printed precision) to show the regeneration is B1's distribution and not a lookalike.
Seeds 1000-1499, 500 per class, 11 classes, default `episode_params`, as B1.

Usage (the binary must be built first, with no build and no measurement running):

    cargo build --locked --release -p gordian-run -j 3
    python3 experiments/exploration/scripts/a7b_generate.py [--verify-b1] [--out FILE]

Output file (default `experiments/exploration/data/a7b-paired-costs.csv.gz`): columns
`pair,compute_ns,seed,class,cost_a,cost_b`. Run directories go to `$GORDIAN_WORK`
(default /tmp/gordian-exploration-a7b), not into the repository.
"""

import argparse
import csv
import gzip
import hashlib
import json
import os
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
WORK = pathlib.Path(os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration-a7b"))
BIN = ROOT / "target" / "release" / "gordian-run"
LEVELS = [20_000_000, 250_000, 100_000, 60_000]
ARMS = [
    ("heuristic_only", "heuristic_only"),
    ("fixed_heuristic_every2", {"policy": "fixed_pipeline", "components": ["heuristic"], "every": 2}),
    ("all_components", "all_components"),
    ("random_p050", {"policy": "random_matched", "p": 0.5}),
]
PAIRS = {"P1": ("heuristic_only", "fixed_heuristic_every2"), "P2": ("all_components", "random_p050")}
DEFAULT_OUT = ROOT / "experiments" / "exploration" / "data" / "a7b-paired-costs.csv.gz"


def run_id(level):
    return f"a7b-c{level}-s1000-1499"


def make_manifest(level):
    tmp = WORK / f"manifest-{level}.base"
    subprocess.run(
        [str(BIN), "init", "--run-id", run_id(level), "--policy", "heuristic_only",
         "--seed-start", "1000", "--seed-count", "500", "--out", str(tmp),
         "--experiment", "exploration-a7b", "--trace-sample-rate", "0"],
        check=True, cwd=ROOT,
    )
    m = json.load(open(tmp))
    m.pop("arm"), m.pop("policy")
    m["arms"] = [{"arm": n, "policy": p} for n, p in ARMS]
    m["run_seed"] = 1  # B1's; results.csv does not depend on it (B1 checked)
    m["limits"]["compute"] = level
    m["isolation"]["timeout_secs"] = 3600
    out = WORK / f"manifest-{level}.json"
    json.dump(m, open(out, "w"), indent=2)
    tmp.unlink()
    return out


def run_level(level):
    out_dir = WORK / "runs" / run_id(level)
    if (out_dir / ARMS[0][0] / "results.csv").exists():
        print(f"reusing {out_dir}", file=sys.stderr)
        return out_dir
    manifest = make_manifest(level)
    out_dir.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [str(ROOT / "scripts" / "cgroup-run.sh"), "--name", run_id(level), "--cpus", "0-2",
         "--cpu-quota", "300", "--memory", "2G", "--report", str(out_dir.parent / f"{run_id(level)}.usage.json"),
         "--", str(BIN), "--manifest", str(manifest), "--out", str(out_dir)],
        check=True, cwd=ROOT,
    )
    return out_dir


def modelled(path):
    """{(seed, class): modelled_cost_ns} from one arm's results.csv; fails on duplicates."""
    out = {}
    with open(path, newline="") as fh:
        for row in csv.DictReader(fh):
            key = (int(row["seed"]), row["class"])
            if key in out:
                raise SystemExit(f"duplicate episode {key} in {path}")
            out[key] = int(row["modelled_component_ns"]) + int(row["modelled_sched_ns"])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument("--verify-b1", action="store_true",
                    help="compare the regenerated modelled means with b1-variance.csv")
    args = ap.parse_args()
    WORK.mkdir(parents=True, exist_ok=True)
    if not BIN.exists():
        raise SystemExit(f"{BIN} is missing: build it first (cargo build --locked --release -p gordian-run)")

    rows = []
    per_arm = {}
    for level in LEVELS:
        d = run_level(level)
        for arm, _ in ARMS:
            per_arm[(level, arm)] = modelled(d / arm / "results.csv")
        for pair, (a, b) in PAIRS.items():
            ca, cb = per_arm[(level, a)], per_arm[(level, b)]
            if ca.keys() != cb.keys():
                raise SystemExit(f"arms {a} and {b} played different episodes at {level}")
            for key in sorted(ca):
                rows.append((pair, level, key[0], key[1], ca[key], cb[key]))

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    # mtime=0 and a fixed order so the file is byte-reproducible
    with open(out, "wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, filename="") as gz:
        text = "pair,compute_ns,seed,class,cost_a,cost_b\n" + "".join(
            f"{p},{c},{s},{k},{a},{b}\n" for p, c, s, k, a, b in rows)
        gz.write(text.encode())
    print(f"wrote {out}: {len(rows)} rows, sha256 {hashlib.sha256(out.read_bytes()).hexdigest()}")

    if args.verify_b1:
        verify_b1(per_arm)


def verify_b1(per_arm):
    """Every (arm, level, class) modelled mean and sd must equal b1-variance.csv's."""
    import statistics

    ref = ROOT / "experiments" / "exploration" / "b1-variance.csv"
    n_checked = n_bad = 0
    with open(ref, newline="") as fh:
        for row in csv.DictReader(fh):
            key = (int(row["compute_ns"]), row["arm"])
            if key not in per_arm:
                continue
            vals = [v for (s, k), v in per_arm[key].items() if k == row["class"]]
            mean, sd = statistics.fmean(vals), statistics.stdev(vals)
            n_checked += 1
            ok = (len(vals) == int(row["n"]) and abs(mean - float(row["modelled_cost_ns_mean"])) < 5e-4
                  and abs(sd - float(row["modelled_cost_ns_sd"])) < 5e-6 * max(1.0, sd))
            if not ok:
                n_bad += 1
                print(f"MISMATCH {key} {row['class']}: n={len(vals)} mean={mean} sd={sd} vs "
                      f"{row['n']} {row['modelled_cost_ns_mean']} {row['modelled_cost_ns_sd']}")
    print(f"verify-b1: {n_checked} (arm, level, class) cells compared, {n_bad} mismatches")
    if n_checked == 0 or n_bad:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
