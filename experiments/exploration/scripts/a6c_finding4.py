"""A6c: does a failed component still raise an arm's success? The `finding4.py` paired comparison, before and after.

Stage B exploration script (development run; nothing here tests a hypothesis). Standard library only.

    a6c_finding4.py

Reads the ComponentTimeout grids that `finding4.py grid SIDE-COND COND` wrote for SIDE in `before`,
`after` and COND in `all` (directives as in B1), `none` (no directive applied), `fail` (only `Fail`
directives) and `slow` (only `Slow`): run ids `diag4-SIDE-COND-c<budget>` under artifacts/runs. Setup
and the binary that reads the directive switch are in `finding4.py`'s docstring (the patch's harness
hunk only; the rule is the repository's, so "before" is the tree at da73030 and "after" the tree
after work item A6c).

Prints, per side, for the seven arms at the four budgets (500 ComponentTimeout episodes each):

- success % with all directives, with none, with only `Fail`, with only `Slow`;
- the number of episodes `Fail` alone raised and lowered against none (the paired effect);

and for the three cells B3 reported (`b3-stress.md`), the same with the real components that failed.
"""
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import finding4 as f4  # noqa: E402

CONDS = ("all", "none", "fail", "slow")


def table(side, dirs):
    pct = lambda r: 100 * sum(f4.ok(x) for x in r.values()) / len(r)
    print(f"\n{side}: ComponentTimeout success %, by which directives are applied (n=500); "
          "raised/lowered = episodes Fail alone changed against no directive")
    print(f"{'arm':24s}{'budget':>10s}{'all':>7s}{'none':>7s}{'fail':>7s}{'slow':>7s} | Fail-only up/down  Slow-only up/down")
    total_up = total_down = 0
    out = {}
    for c in f4.BUDGETS:
        for arm in f4.KEEP:
            r = {t: f4.rows(f"{side}-{t}", c, arm) for t in CONDS}
            n = r["none"]

            def flips(t):
                return (sum(1 for k in r[t] if f4.ok(r[t][k]) and not f4.ok(n[k])),
                        sum(1 for k in r[t] if not f4.ok(r[t][k]) and f4.ok(n[k])))

            fu, fd = flips("fail")
            su, sd = flips("slow")
            if c != 20_000_000:
                total_up += fu
                total_down += fd
            star = "  <== B3 cell" if (arm, c) in f4.CELLS else ""
            print(f"{arm:24s}{c:10d}{pct(r['all']):7.1f}{pct(n):7.1f}{pct(r['fail']):7.1f}{pct(r['slow']):7.1f}"
                  f" | {fu:4d}/{fd:<4d}            {su:4d}/{sd:<4d}{star}")
            out[(arm, c)] = (pct(r["all"]), pct(n), pct(r["fail"]), fu, fd)
    print(f"binding budgets (250,000 and below), all seven arms: Fail alone raised {total_up} episodes, lowered {total_down}")
    print(f"\n{side}: the three B3 cells, episodes raised by Fail alone, by which real components failed")
    for arm, c in f4.CELLS:
        rf, rn = f4.rows(f"{side}-fail", c, arm), f4.rows(f"{side}-none", c, arm)
        ups = [k for k in rf if f4.ok(rf[k]) and not f4.ok(rn[k])]
        downs = [k for k in rf if not f4.ok(rf[k]) and f4.ok(rn[k])]
        print(f"  {arm} @ {c}: raised {len(ups)}, lowered {len(downs)};",
              dict(Counter(tuple(sorted(f4.NAMES[i] for i in dirs[k])) for k in ups)),
              "stop reasons (none, Fail-only) of the raised:",
              dict(Counter((rn[k]["stop_reason"], rf[k]["stop_reason"]) for k in ups)))
    return out


def main():
    dirs = f4.directives()
    for side in (sys.argv[1:] or ("before", "after")):
        table(side, dirs)


if __name__ == "__main__":
    main()
