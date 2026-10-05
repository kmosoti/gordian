"""Finding 4 (a failed component raised an arm's score at binding budgets): diagnosis scripts.

Stage B exploration script (development run; nothing here tests a hypothesis). The write-up is
`experiments/exploration/b3-finding4.md`; read it first.

The diagnosis needs a binary that can switch harness directives off and change what the shared
rule charges. That binary is built from a COPY of `crates/` with `finding4-diagnostic.patch`
applied. The patch is never applied to the repository tree: it makes the harness and the rule read
environment variables, which the reference core must not do. Setup, from the repository root with
a clean tree (the driver refuses otherwise) and no other build running:

    export GORDIAN_WORK=/tmp/gordian-exploration   # the default
    mkdir -p $GORDIAN_WORK/diag-src && cp -r crates Cargo.toml Cargo.lock rust-toolchain.toml $GORDIAN_WORK/diag-src/
    (cd $GORDIAN_WORK/diag-src && patch -p1 < <repo>/experiments/exploration/scripts/finding4-diagnostic.patch \
        && cargo build --release -p gordian-run && cargo run --release -q -p gordian-world --example directives \
        > $GORDIAN_WORK/directives.tsv)
    cargo build --locked --release -p gordian-run          # the unpatched binary, for the manifests
    python3 experiments/exploration/scripts/make_manifests.py   # writes $GORDIAN_WORK/manifests/c*.json

Subcommands (every run goes through scripts/run-driver.sh, so cores 0-2 and the cgroup limits apply):

    finding4.py grid TAG ENV          ComponentTimeout only, seven arms, four budgets, 500 seeds.
                                      ENV is the value of GORDIAN_DIAG_DIRECTIVES (all, none, fail,
                                      slow). Run ids are diag4-TAG-c<budget>.
    finding4.py grid TAG ENV --vfd    the same with GORDIAN_DIAG_VERIFIER_FREE_DECODE set
    finding4.py grid TAG ENV --vun    the same with GORDIAN_DIAG_VERIFIER_UNUSED set
    finding4.py full TAG ENV --vfd    all eleven classes and ten arms (the B1 grid), run ids diag4-TAG-c<budget>
    finding4.py trace RUN_ID BUDGET ARM SEEDS ENV [--vfd]
                                      one arm, the listed seeds (comma-separated), trace_sample_rate 1
    finding4.py show RUN_ID ARM SEED [MAXSTEPS]
                                      per-step view of one sampled episode from the ledger
    finding4.py analyse               the paired tables of the write-up, from the grid runs
                                      all, none, fail, slow, vfd, vun (tags of the same names)
"""
import csv
import json
import os
import pathlib
import re
import subprocess
import sys
from collections import Counter, defaultdict

ROOT = str(pathlib.Path(__file__).resolve().parents[3])
W = os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration")
RUNS = f"{ROOT}/artifacts/runs"
BUDGETS = (20_000_000, 250_000, 100_000, 60_000)
KEEP = ["heuristic_only", "all_components", "random_p025", "random_p050", "fixed_verifier_only",
        "fixed_estimator_only", "fixed_heuristic_every4"]
CELLS = [("all_components", 250_000), ("random_p050", 100_000), ("random_p025", 60_000)]
NAMES = {0: "heur", 1: "est", 2: "mem", 3: "ver"}
ok = lambda r: r["success"] == "true"


def base_manifest(c):
    return json.load(open(f"{W}/manifests/c{c}.json"))


def write_manifest(m):
    path = f"{W}/manifests/{m['run_id']}.json"
    json.dump(m, open(path, "w"), indent=2)
    open(path, "a").write("\n")
    return path


def drive(manifest, env_value, vfd=False, vun=False):
    env = dict(os.environ, GORDIAN_DIAG_DIRECTIVES=env_value)
    if vfd:
        env["GORDIAN_DIAG_VERIFIER_FREE_DECODE"] = "1"
    if vun:
        env["GORDIAN_DIAG_VERIFIER_UNUSED"] = "1"
    r = subprocess.run([f"{ROOT}/scripts/run-driver.sh", "--manifest", manifest,
                        "--bin", f"{W}/diag-src/target/release/gordian-run"], env=env, cwd=ROOT,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(os.path.basename(manifest), "exit", r.returncode, flush=True)


def cmd_grid(tag, env_value, flags, full=False):
    for c in BUDGETS:
        m = base_manifest(c)
        if not full:
            m["arms"] = [a for a in m["arms"] if a["arm"] in KEEP]
            m["episode_classes"] = [["ComponentTimeout", 500]]
        m["run_id"] = f"diag4-{tag}-c{c}"
        m["experiment"] = "exploration-b3-finding4"
        m["trace_sample_rate"] = 0.0
        drive(write_manifest(m), env_value, "--vfd" in flags, "--vun" in flags)


def cmd_trace(run_id, budget, arm, seeds, env_value, flags):
    m = base_manifest(250_000)
    m["arms"] = [a for a in m["arms"] if a["arm"] == arm]
    seeds = [int(s) for s in seeds.split(",")]
    m["seeds"], m["episode_classes"] = seeds, [["ComponentTimeout", len(seeds)]]
    m["run_id"], m["experiment"] = run_id, "exploration-b3-finding4-trace"
    m["trace_sample_rate"] = 1.0
    m["limits"]["compute"] = int(budget)
    drive(write_manifest(m), env_value, "--vfd" in flags)


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


def rows(tag, c, arm):
    return {int(x["seed"]): x for x in csv.DictReader(open(f"{RUNS}/diag4-{tag}-c{c}/{arm}/results.csv"))}


def ambiguous(c, arm):
    rs = [x for x in csv.DictReader(open(f"{RUNS}/b1-c{c}-s1000-1499/{arm}/results.csv")) if x["class"] == "Ambiguous"]
    return 100 * sum(ok(x) for x in rs) / len(rs)


def directives():
    out = {}
    for line in list(open(f"{W}/directives.tsv"))[1:]:
        seed, d = line.rstrip("\n").split("\t")
        fails = set()
        for item in filter(None, d.split(";")):
            comp, mode = item.split(":", 1)
            if mode == "Fail" and int(comp) in NAMES:
                fails.add(int(comp))
        out[int(seed)] = fails
    return out


def cmd_analyse():
    dirs = directives()
    pct = lambda r: 100 * sum(ok(x) for x in r.values()) / len(r)
    print("T1. ComponentTimeout success %, by which directives are applied (n=500); Ambiguous from B1")
    print(f"{'arm':24s}{'budget':>10s}{'all':>7s}{'none':>7s}{'fail':>7s}{'slow':>7s}{'Ambig':>7s} | up/down vs none: all, fail-only, slow-only")
    for c in BUDGETS:
        for arm in KEEP:
            r = {t: rows(t, c, arm) for t in ("all", "none", "fail", "slow")}

            def fl(t):
                return (sum(1 for k in r[t] if ok(r[t][k]) and not ok(r["none"][k])),
                        sum(1 for k in r[t] if not ok(r[t][k]) and ok(r["none"][k])))
            star = "  <== B3 cell" if (arm, c) in CELLS else ""
            print(f"{arm:24s}{c:10d}{pct(r['all']):7.1f}{pct(r['none']):7.1f}{pct(r['fail']):7.1f}{pct(r['slow']):7.1f}"
                  f"{ambiguous(c, arm):7.1f} | {fl('all')[0]}/{fl('all')[1]}  {fl('fail')[0]}/{fl('fail')[1]}  {fl('slow')[0]}/{fl('slow')[1]}{star}")
    print("\nT2. The three B3 cells: episodes raised by Fail-only, by which real components failed")
    for arm, c in CELLS:
        rf, rn = rows("fail", c, arm), rows("none", c, arm)
        ups = [k for k in rf if ok(rf[k]) and not ok(rn[k])]
        print(f"{arm} @ {c}: raised {len(ups)}, lowered {sum(1 for k in rf if not ok(rf[k]) and ok(rn[k]))};",
              dict(Counter(tuple(sorted(NAMES[i] for i in dirs[k])) for k in ups)),
              "stop reasons (without, Fail-only):", dict(Counter((rn[k]["stop_reason"], rf[k]["stop_reason"]) for k in ups)))
    print("\nT3. Which mechanism: no directives; none = rule as committed; VFD = verifier decoding not charged; "
          "VUNUSED = verifier set never a candidate source (all charges unchanged)")
    print(f"{'arm':24s}{'budget':>10s}{'none':>7s}{'VFD':>7s}{'VUNUSED':>9s}")
    for c, arms in ((250_000, ("all_components", "random_p050", "random_p025", "fixed_verifier_only", "fixed_estimator_only")),
                    (100_000, ("all_components", "random_p050", "random_p025")), (60_000, ("random_p025", "random_p050"))):
        for arm in arms:
            r = {t: rows(t, c, arm) for t in ("none", "vfd", "vun")}
            print(f"{arm:24s}{c:10d}{pct(r['none']):7.1f}{pct(r['vfd']):7.1f}{pct(r['vun']):9.1f}")


if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    flags = [x for x in sys.argv[1:] if x.startswith("--")]
    if a[0] == "grid":
        cmd_grid(a[1], a[2], flags)
    elif a[0] == "full":
        cmd_grid(a[1], a[2], flags, full=True)
    elif a[0] == "trace":
        cmd_trace(a[1], a[2], a[3], a[4], a[5], flags)
    elif a[0] == "show":
        cmd_show(a[1], a[2], int(a[3]), int(a[4]) if len(a) > 4 else 10**9)
    elif a[0] == "analyse":
        cmd_analyse()
    else:
        sys.exit(__doc__)
