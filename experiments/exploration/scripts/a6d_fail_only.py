"""A6d: the Fail-only paired comparison and its ledger traces, for any build of the rule.

Stage B exploration script (development run; nothing here tests a hypothesis). Standard library
only. It generalises `finding4.py` / `a6c_finding4.py`: the binary and the diagnostic switches are
arguments, so it serves the old rule, the new rule and every scratch variant of either.

    a6d_fail_only.py grid TAG BIN [ENV=VALUE ...]
    a6d_fail_only.py trace TAG BIN BUDGET ARM SEEDS [ENV=VALUE ...]
    a6d_fail_only.py table PREFIX [--seeds]
    a6d_fail_only.py compare BEFORE_PREFIX AFTER_PREFIX
    a6d_fail_only.py control SIDE TAG
    a6d_fail_only.py show RUN_ID ARM SEED [MAXSTEPS]

`grid` plays ComponentTimeout, seeds 1000 to 1499, seven public arms, the four budgets of B1, one run
per budget, run ids `diag6-TAG-c<budget>`, through `scripts/run-driver.sh` (so cores 0-2 and the
cgroup limits apply). BIN is a `gordian-run` built from a COPY of the tree with
`a6d_diag_patch.py` applied (it reads GORDIAN_DIAG_DIRECTIVES and the variant switches), and the
ENV=VALUE pairs set those switches for the run. `trace` plays the listed seeds of one arm at one
budget with every episode traced (`trace_sample_rate` 1), run id `diag6tr-TAG-c<budget>-<arm>`.
`table PREFIX` compares the runs PREFIX-none and PREFIX-fail: per arm and budget, success % with no
directive and with only `Fail` directives, and the episodes `Fail` alone raised and lowered.
`compare BEFORE AFTER` prints, for the seven cells of `a6c-before-after.md` section 5, that
comparison for two builds side by side. $GORDIAN_ROOT overrides the repository root the script
finds from its own location (for running it from elsewhere).

Needs $GORDIAN_WORK/manifests/c<budget>.json (from `make_manifests.py`, at the commit being run; the
driver refuses a manifest whose source revision is not a clean HEAD).
"""
import csv
import json
import os
import pathlib
import subprocess
import sys

ROOT = os.environ.get("GORDIAN_ROOT") or str(pathlib.Path(__file__).resolve().parents[3])
W =os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration")
RUNS = f"{ROOT}/artifacts/runs"
BUDGETS = (20_000_000, 250_000, 100_000, 60_000)
ARMS = ["heuristic_only", "all_components", "random_p025", "random_p050", "fixed_verifier_only",
        "fixed_estimator_only", "fixed_heuristic_every4"]
VERIFIER_ARMS = ("all_components", "random_p025", "random_p050")
NAMES = {0: "heur", 1: "est", 2: "mem", 3: "ver"}


def ok(row):
    return row["success"] == "true"


def write_manifest(m):
    path = f"{W}/manifests/{m['run_id']}.json"
    with open(path, "w") as f:
        json.dump(m, f, indent=2)
        f.write("\n")
    return path


def drive(manifest, binary, env_pairs):
    env = dict(os.environ)
    for pair in env_pairs:
        key, value = pair.split("=", 1)
        env[key] = value
    run_id = json.load(open(manifest))["run_id"]
    if os.path.exists(f"{RUNS}/{run_id}/{ARMS[0]}/results.csv") or os.path.exists(f"{RUNS}/{run_id}/results.csv"):
        print(run_id, "exists, skipped", flush=True)
        return
    r = subprocess.run([f"{ROOT}/scripts/run-driver.sh", "--manifest", manifest, "--bin", binary],
                       env=env, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(run_id, "exit", r.returncode, flush=True)
    if r.returncode != 0:
        sys.exit(r.returncode)


def base(c):
    return json.load(open(f"{W}/manifests/c{c}.json"))


def cmd_grid(tag, binary, env_pairs):
    for c in BUDGETS:
        m = base(c)
        m["arms"] = [a for a in m["arms"] if a["arm"] in ARMS]
        m["episode_classes"] = [["ComponentTimeout", 500]]
        m["run_id"] = f"diag6-{tag}-c{c}"
        m["experiment"] = "exploration-a6d"
        m["trace_sample_rate"] = 0.0
        drive(write_manifest(m), binary, env_pairs)


def cmd_trace(tag, binary, budget, arm, seeds, env_pairs):
    m = base(250_000)
    m["arms"] = [a for a in m["arms"] if a["arm"] == arm]
    seeds = [int(s) for s in seeds.split(",")]
    m["seeds"], m["episode_classes"] = seeds, [["ComponentTimeout", len(seeds)]]
    m["run_id"], m["experiment"] = f"diag6tr-{tag}-c{budget}-{arm}", "exploration-a6d-trace"
    m["trace_sample_rate"] = 1.0
    m["limits"]["compute"] = int(budget)
    drive(write_manifest(m), binary, env_pairs)


def rows(tag, c, arm):
    path = f"{RUNS}/diag6-{tag}-c{c}/{arm}/results.csv"
    return {int(x["seed"]): x for x in csv.DictReader(open(path))}


def raised(prefix, c, arm):
    none, fail = rows(f"{prefix}-none", c, arm), rows(f"{prefix}-fail", c, arm)
    up = sorted(k for k in fail if ok(fail[k]) and not ok(none[k]))
    down = sorted(k for k in fail if not ok(fail[k]) and ok(none[k]))
    return none, fail, up, down


def cmd_table(prefix, list_seeds):
    pct = lambda r: 100 * sum(ok(x) for x in r.values()) / len(r)
    print(f"{prefix}: ComponentTimeout success %, no directive, Fail-only; episodes Fail alone raised / lowered")
    print(f"{'arm':24s}{'budget':>10s}{'none':>7s}{'fail':>7s}  up/down")
    total = {"all": [0, 0], "verifier": [0, 0]}
    for c in BUDGETS:
        for arm in ARMS:
            none, fail, up, down = raised(prefix, c, arm)
            print(f"{arm:24s}{c:10d}{pct(none):7.1f}{pct(fail):7.1f}  {len(up)}/{len(down)}"
                  + (f"  {up}" if list_seeds and up else ""))
            if c != 20_000_000:
                total["all"][0] += len(up)
                total["all"][1] += len(down)
                if arm in VERIFIER_ARMS:
                    total["verifier"][0] += len(up)
                    total["verifier"][1] += len(down)
    print(f"binding budgets (250,000 and below), seven arms: raised {total['all'][0]}, lowered {total['all'][1]}")
    print(f"binding budgets, the three verifier-running arms: raised {total['verifier'][0]}, "
          f"lowered {total['verifier'][1]}")


CELLS = [("all_components", 250_000), ("random_p050", 250_000), ("random_p025", 100_000),
         ("random_p050", 100_000), ("all_components", 100_000), ("random_p025", 60_000),
         ("random_p050", 60_000)]
B3_CELLS = {("all_components", 250_000), ("random_p050", 100_000), ("random_p025", 60_000)}


def cmd_compare(before, after):
    """The table of `a6c-before-after.md` section 5, for any two builds: success % with no directive
    and with only `Fail` directives, and the episodes `Fail` alone raised and lowered."""
    pct = lambda r: 100 * sum(ok(x) for x in r.values()) / len(r)
    print(f"| arm | limit | {before}: none, fail, raised/lowered | {after}: none, fail, raised/lowered |")
    print("|---|---|---|---|")
    totals = {before: [0, 0], after: [0, 0]}
    for arm, c in CELLS:
        cells = []
        for prefix in (before, after):
            none, fail, up, down = raised(prefix, c, arm)
            cells.append(f"{pct(none):.1f}, {pct(fail):.1f}, {len(up)}/{len(down)}")
            totals[prefix][0] += len(up)
            totals[prefix][1] += len(down)
        star = " (B3 cell)" if (arm, c) in B3_CELLS else ""
        print(f"| `{arm}` | {c:,}{star} | {cells[0]} | {cells[1]} |")
    for prefix in (before, after):
        print(f"\n{prefix}: over these seven cells `Fail` alone raised {totals[prefix][0]} episodes "
              f"and lowered {totals[prefix][1]}")


DETERMINISTIC = ["success", "critical_miss", "false_alarm", "abstained", "undecided", "probes_used", "corrections",
                 "decision_at_ns", "bill_compute", "components_run", "stop_reason", "ops_component", "ops_sched",
                 "modelled_component_ns", "modelled_sched_ns"]


def cmd_control(side, tag):
    """The scratch binary with its switches unset must give the shipped binary's results: compare the
    ComponentTimeout rows of the B1 grid of `a6d-SIDE` with the grid `diag6-TAG` (run with no switch set),
    on every deterministic column."""
    rows_n = differ = 0
    for c in BUDGETS:
        for arm in ARMS:
            path = f"{RUNS}/a6d-{side}-b1-c{c}-s1000-1499/{arm}/results.csv"
            a = {int(r["seed"]): r for r in csv.DictReader(open(path)) if r["class"] == "ComponentTimeout"}
            b = rows(tag, c, arm)
            assert a.keys() == b.keys()
            for k in a:
                rows_n += 1
                differ += any(a[k][col] != b[k][col] for col in DETERMINISTIC)
    print(f"{rows_n} rows compared, {differ} differ")


def cmd_show(run, arm, seed, maxsteps=10**9):
    entries, stream = [], []
    for line in open(f"{RUNS}/{run}/{arm}/events-sample.jsonl"):
        d = json.loads(line)
        if d["seed"] != seed:
            continue
        if d["record"] == "public_stream":
            stream = d["observations"]
        if d["record"] == "entry":
            entries.append(d)
    steps, cur, total_comp, total_rule = [], None, 0, 0
    for e in entries:
        k, p, pl = e["kind"], e["producer"], e["payload"]
        if k == "Accounting" and p.startswith("policy/") and pl["phase"].get("phase") == "Scheduling":
            cur = dict(at=e["at_ns"], rule=sum(c["amount"] for c in pl["charges"]), comp={}, hyp=[], dec=[],
                       refused=not pl["accepted"])
            steps.append(cur)
            total_rule += cur["rule"]
        elif cur is None:
            continue
        elif k == "Accounting" and p.startswith("component/"):
            amt = sum(c["amount"] for c in pl["charges"] if c["resource"] == "Compute")
            cur["comp"][NAMES[int(p.split("/")[1])]] = (amt, pl["accepted"])
            total_comp += amt if pl["accepted"] else 0
        elif k == "Hypothesis" and "Candidates" in pl:
            c = pl["Candidates"]
            cur["hyp"].append(f"{c['source'][:4]}:{len(c['ranked'])}top={c['ranked'][0]['hypothesis'] if c['ranked'] else None}")
        elif k == "ComputationResult":
            cur["hyp"].append(f"{NAMES[pl['component']]}:NONE")
        elif k == "Decision":
            cur["dec"].append(json.dumps(pl))
    print(f"{run}/{arm} seed {seed}: {len(steps)} steps, rule charged {total_rule}, components charged {total_comp}")
    print("first observation instants (ns):", [o["at_ns"] for o in stream][:6])
    for i, s in enumerate(steps[:maxsteps]):
        comp = " ".join(f"{n}={a}{'' if good else '(refused)'}" for n, (a, good) in s["comp"].items())
        print(f"{i:3d} t={s['at'] / 1e6:7.1f}ms rule={s['rule']:4d}{'(refused)' if s['refused'] else ''} {comp} | "
              f"{'; '.join(s['hyp'])} {'DEC ' + ' '.join(s['dec']) if s['dec'] else ''}")


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    if a[0] == "grid":
        cmd_grid(a[1], a[2], a[3:])
    elif a[0] == "trace":
        cmd_trace(a[1], a[2], a[3], a[4], a[5], a[6:])
    elif a[0] == "table":
        cmd_table(a[1], "--seeds" in a)
    elif a[0] == "compare":
        cmd_compare(a[1], a[2])
    elif a[0] == "control":
        cmd_control(a[1], a[2])
    elif a[0] == "show":
        cmd_show(a[1], a[2], int(a[3]), int(a[4]) if len(a) > 4 else 10**9)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
