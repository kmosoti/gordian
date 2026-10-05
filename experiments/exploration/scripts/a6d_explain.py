"""A6d: explain the episodes a `Fail` directive alone raised, from the paired runs and the ledger.

Stage B exploration script (development run; nothing here tests a hypothesis). Standard library
only. Reads the runs `a6d_fail_only.py` wrote.

    a6d_explain.py PREFIX [--channels A,B,...] [--traces TRACEPREFIX] [--rows]

PREFIX names the grids `diag6-PREFIX-none` and `diag6-PREFIX-fail` (ComponentTimeout, seeds 1000
to 1499, seven arms, four budgets). The raised episodes are those the Fail-only run decided
correctly and the run with no directive did not, at the three binding budgets, for the arms that
run the verifier. With `--channels`, each channel is a grid `diag6-PREFIX-<channel>` of the
no-directive condition with one channel of the `Fail` imitated (the scratch variants of
`a6d_diag_patch.py`), and every raised episode is classified by which channels alone get it right.
With `--traces`, the ledger traces `diag6tr-TRACEPREFIX-none-c<budget>-<arm>` and `...-fail-...`
give, per episode, what the two runs spent the compute budget on until the no-directive run
stopped.
"""
import csv
import json
import os
import sys
from collections import Counter

ROOT = os.environ.get("GORDIAN_ROOT") or os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
W = os.environ.get("GORDIAN_WORK", "/tmp/gordian-exploration")
RUNS = f"{ROOT}/artifacts/runs"
BUDGETS = (250_000, 100_000, 60_000)
ARMS = ("all_components", "random_p025", "random_p050")
NAMES = {0: "heur", 1: "est", 2: "mem", 3: "ver"}
UNITS = ["calls", "dec_out", "dec_rank", "worlds", "evals", "cmp_b"]
ok = lambda r: r["success"] == "true"


def rows(tag, c, arm):
    path = f"{RUNS}/diag6-{tag}-c{c}/{arm}/results.csv"
    return {int(x["seed"]): x for x in csv.DictReader(open(path))}


def failed_components():
    out = {}
    for line in list(open(f"{W}/directives.tsv"))[1:]:
        seed, d = line.rstrip("\n").split("\t")
        out[int(seed)] = sorted(
            int(c.split(":")[0]) for c in filter(None, d.split(";")) if c.split(":", 1)[1] == "Fail" and int(c.split(":")[0]) in NAMES
        )
    return out


def trace(tp, cond, budget, arm, seed):
    path = f"{RUNS}/diag6tr-{tp}-{cond}-c{budget}-{arm}/{arm}/events-sample.jsonl"
    if not os.path.exists(path):
        return None
    steps, cur = [], None
    for line in open(path):
        d = json.loads(line)
        if d["seed"] != seed or d["record"] != "entry":
            continue
        k, p, pl = d["kind"], d["producer"], d["payload"]
        if k == "Accounting" and p.startswith("policy/") and pl["phase"].get("phase") == "Scheduling":
            cur = dict(rule=sum(c["amount"] for c in pl["charges"] if c["resource"] == "Compute"),
                       refused=not pl["accepted"], comp=0, ops=[0] * 6)
            steps.append(cur)
        elif cur is None:
            continue
        elif k == "Accounting" and p.startswith("component/") and pl["accepted"]:
            cur["comp"] += sum(c["amount"] for c in pl["charges"] if c["resource"] == "Compute")
        elif k == "Measurement" and pl.get("what") == "decide":
            cur["ops"] = pl["ops"][:6]
    return steps


def ledger(tp, budget, arm, seed):
    """What the two runs spent until the no-directive run stopped, and by which units."""
    n, f = trace(tp, "none", budget, arm, seed), trace(tp, "fail", budget, arm, seed)
    if not n or not f:
        return None
    k = len(n)  # the no-directive run's steps, the last of which may be the refused one
    accepted = [s for s in n if not s["refused"]]
    spent = sum(s["rule"] + s["comp"] for s in accepted)
    refused = [s for s in n if s["refused"]]
    short = None
    if refused:
        short = refused[-1]["rule"] - (budget - spent)
    rule_n = sum(s["rule"] for s in accepted)
    rule_f = sum(s["rule"] for s in f[: len(accepted)] if not s["refused"])
    du = [sum(s["ops"][i] for s in n[:k]) - sum(s["ops"][i] for s in f[:k]) for i in range(6)]
    return dict(steps_n=len(n), steps_f=len(f), rule_delta=rule_n - rule_f, short=short, units=du)


def main():
    a = sys.argv[1:]
    prefix = a[0]
    channels = a[a.index("--channels") + 1].split(",") if "--channels" in a else []
    tp = a[a.index("--traces") + 1] if "--traces" in a else None
    dirs = failed_components()
    groups = Counter()
    lines = []
    for c in BUDGETS:
        for arm in ARMS:
            none, fail = rows(f"{prefix}-none", c, arm), rows(f"{prefix}-fail", c, arm)
            var = {ch: rows(f"{prefix}-{ch}", c, arm) for ch in channels}
            for k in sorted(fail):
                if ok(fail[k]) and not ok(none[k]):
                    got = tuple(ch for ch in channels if ok(var[ch][k]))
                    groups[got] += 1
                    led = ledger(tp, c, arm, k) if tp else None
                    lines.append((arm, c, k, [NAMES[i] for i in dirs[k]], none[k]["stop_reason"], fail[k]["stop_reason"], got, led))
    print(f"{prefix}: episodes raised by Fail alone, binding budgets, verifier-running arms: {len(lines)}")
    if channels:
        print("sufficient channels (variants of the no-directive run that get the episode right):")
        for key, n in sorted(groups.items(), key=lambda kv: (-kv[1], kv[0])):
            print(f"  {n:3d}  {', '.join(key) or 'none of them'}")
    print("stop reasons (no directive -> Fail only):",
          dict(Counter((x[4], x[5]) for x in lines)))
    leds = [x[7] for x in lines if x[7]]
    if leds:
        # What the no-directive run's rule did that the Fail-only run's did not, over these episodes,
        # priced at the declared constants of decide.rs (530 ns per decoded output, 115 per listed
        # hypothesis, 0.033 per compared byte, 10.5 per narrowed world, 36 per probe evaluation).
        # Listed hypotheses stand for the leading tie the declared cost counts, which can only
        # overstate decoding a little.
        sums = [sum(l["units"][i] for l in leds) for i in range(6)]
        price = {1: 530.0, 2: 115.0, 3: 10.5, 4: 36.0, 5: 0.033}
        parts = {UNITS[i]: sums[i] * price[i] for i in price}
        total = sum(parts.values())
        print(f"{len(leds)} traced episodes: unit differences {dict(zip(UNITS, sums))}")
        print("  at the declared constants: " + ", ".join(f"{k} {v:,.0f} ns ({100 * v / total:.0f}%)" for k, v in parts.items()),
              f"= {total:,.0f} ns; the rule's declared charge differed by {sum(l['rule_delta'] for l in leds):,} ns")
        shorts = sorted(l["short"] for l in leds if l["short"] is not None)
        print(f"  the no-directive run's refused step was short of budget by median {shorts[len(shorts) // 2]} ns, "
              f"max {shorts[-1]} ns, under 100 ns in {sum(1 for s in shorts if s < 100)} of {len(shorts)}; "
              f"the rule charged the no-directive run more by median {sorted(l['rule_delta'] for l in leds)[len(leds) // 2]:,} ns")
    if "--rows" in a:
        for arm, c, k, failed, sn, sf, got, led in lines:
            extra = ""
            if led:
                extra = (f" | steps {led['steps_n']}/{led['steps_f']}, short by {led['short']} ns, rule charge +{led['rule_delta']} ns, "
                         + ", ".join(f"{u} {v:+d}" for u, v in zip(UNITS, led['units']) if v and u != "calls"))
            print(f"  {arm} {c} {k} fails {failed} {sn} -> {sf} channels {list(got)}{extra}")
    if "--md" in a:
        print("| arm | limit | seed | failed | stop without -> with | channels alone sufficient | short by (ns) | rule charged more (ns) | decoded listed / compared bytes / worlds narrowed, more |")
        print("|---|---:|---:|---|---|---|---:|---:|---|")
        for arm, c, k, failed, sn, sf, got, led in lines:
            u = dict(zip(UNITS, led["units"])) if led else {}
            alone = ", ".join(g for g in got if g not in ("mboth", "vboth")) or "only together"
            print(f"| `{arm}` | {c:,} | {k} | {', '.join(failed) or '-'} | {sn} -> {sf} | {alone} | "
                  f"{led['short'] if led and led['short'] is not None else '-'} | "
                  f"{led['rule_delta']:,} | {u.get('dec_rank', 0):+d} / {u.get('cmp_b', 0):+,d} / {u.get('worlds', 0):+d} |"
                  if led else f"| `{arm}` | {c:,} | {k} | {', '.join(failed) or '-'} | {sn} -> {sf} | {alone} | | | |")


if __name__ == "__main__":
    main()
