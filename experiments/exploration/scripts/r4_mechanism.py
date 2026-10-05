"""R4 mechanism check: when does `always_escalate` escalate a hard incident, and what is in the context?

Exploration (nothing here tests a hypothesis). Analysis only: it reads evaluator output
(`incidents.csv`) and the events samples of a stream run, and uses the privileged arm's recorded
escalation contexts as the identity of each hard incident's decisive observations. None of this
reaches a policy.

Usage: r4_mechanism.py RUN_DIR [ARM ...]      (default arm: always_escalate)
       The run must hold the privileged arm `oracle_escalation_privileged` and events samples.

Method. The privileged arm makes one escalation per hard incident whose context is exactly that
incident's decisive set D. Each hard incident is matched to one privileged call by onset
(`first_correct_at - time_to_first_correct` in the privileged arm's incidents.csv; every decisive
observation arrives in [onset + 6 s, onset + 16 s)). Another arm's escalation is attributed to a
hard incident when its focus observation was emitted in [onset, onset + 40 s) and at a service
that carries one of the incident's decisive observations. The attribution is a heuristic, so it
is checked against the evaluator's own per-incident escalation counts, and every figure below is
reported with how many incidents the two agree on. q for a call is |context & D| / |D|.
"""

import json
import sys
from pathlib import Path

import pandas as pd

WINDOW_NS = 40_000_000_000
ORACLE = "oracle_escalation_privileged"


def events_by_seed(path):
    """{seed: (obs_at, obs_service, escalations)}; the file holds one block per sampled seed."""
    out = {}
    cur = None
    for line in open(path):
        d = json.loads(line)
        if d.get("record") == "public_stream":
            cur = d["seed"]
            obs = d["observations"]
            out[cur] = (
                [o["at_ns"] for o in obs],
                [next(iter(o["observation"].values())).get("service") for o in obs],
                [],
            )
            continue
        if d.get("record") != "entry" or d["kind"] != "Decision":
            continue
        a = d["payload"]["action"]
        if "Escalate" in a:
            e = a["Escalate"]
            out[cur][2].append(
                (
                    d["at_ns"],
                    e["question"]["Diagnose"]["focus"],
                    [r["Passive"] for r in e["context"] if "Passive" in r],
                )
            )
    return out


def main():
    run = Path(sys.argv[1])
    arms = sys.argv[2:] or ["always_escalate"]
    orc_inc = pd.read_csv(run / ORACLE / "incidents.csv")
    orc_ev = events_by_seed(run / ORACLE / "events-sample.jsonl")
    hard = orc_inc[orc_inc["tier"] == "hard"].copy()
    hard["onset_ns"] = hard["first_correct_at_ns"] - hard["time_to_first_correct_ns"]

    # incident -> (onset, D, oracle call instant), matched per seed
    info = {}
    for seed, h in hard.groupby("seed"):
        if seed not in orc_ev:
            continue
        at, svc, calls = orc_ev[seed]
        left = list(range(len(calls)))
        for r in h.itertuples():
            if pd.isna(r.onset_ns):
                continue
            cand = [
                k
                for k in left
                if (dmin := min(at[o] for o in calls[k][2])) - 16.1e9 < r.onset_ns <= dmin - 5.9e9
            ]
            if len(cand) == 1:
                k = cand[0]
                left.remove(k)
                info[(int(seed), int(r.incident))] = (int(r.onset_ns), set(calls[k][2]), calls[k][0], r.family)
        # incidents the privileged arm missed have no onset: matched only if one call is left over
    rows = []
    for arm in arms:
        inc = pd.read_csv(run / arm / "incidents.csv")
        ev = events_by_seed(run / arm / "events-sample.jsonl")
        for (seed, i), (on, D, o_at, fam) in sorted(info.items()):
            if seed not in ev:
                continue
            at, svc, esc = ev[seed]
            sites = {svc[o] for o in D}
            mine = [
                (t, f, c)
                for (t, f, c) in esc
                if on <= at[f] < on + WINDOW_NS and svc[f] in sites
            ]
            ev_row = inc[(inc.seed == seed) & (inc.incident == i)].iloc[0]
            first = min(mine, key=lambda x: x[0]) if mine else None
            dlast = max(at[o] for o in D)
            row = {
                "arm": arm,
                "seed": seed,
                "incident": i,
                "family": fam,
                "last_decisive_after_onset_s": (dlast - on) / 1e9,
                "oracle_call_after_onset_s": (o_at - on) / 1e9,
                "n_decisive": len(D),
                "calls_attributed": len(mine),
                "calls_evaluator": int(ev_row.escalations),
                "informed_evaluator": int(ev_row.informed_escalations),
            }
            if first is not None:
                row["first_call_after_onset_s"] = (first[0] - on) / 1e9
                row["first_q"] = len(set(first[2]) & D) / len(D)
                row["best_q"] = max(len(set(c) & D) / len(D) for (_, _, c) in mine)
                row["first_before_last_decisive"] = first[0] < dlast
            rows.append(row)
    df = pd.DataFrame(rows)
    pd.set_option("display.width", 250, "display.max_columns", 30, "display.max_rows", 400)
    print(df.round(2).to_string(index=False))
    for arm, g in df.groupby("arm"):
        agree = g[g.calls_attributed == g.calls_evaluator]
        print(f"\n== {arm}: {len(g)} hard incidents matched to a privileged call, in sampled streams")
        print(f"attribution agrees with the evaluator's call count on {len(agree)} of {len(g)}")
        for name, sub in (("all matched", g), ("attribution agrees", agree)):
            e = sub[sub.calls_attributed > 0]
            if not len(e):
                continue
            print(f"-- {name}: {len(e)} incidents with at least one attributed call")
            print(
                "   first call after onset (s): median %.2f (min %.2f, max %.2f); last decisive evidence arrives at median %.2f"
                % (e.first_call_after_onset_s.median(), e.first_call_after_onset_s.min(),
                   e.first_call_after_onset_s.max(), e.last_decisive_after_onset_s.median())
            )
            print("   first call before the last decisive observation was delivered:",
                  int(e.first_before_last_decisive.sum()), "of", len(e))
            print("   first-call q: mean %.3f; q == 0 for %d; q == 1 for %d" % (
                e.first_q.mean(), int((e.first_q == 0).sum()), int((e.first_q >= 0.999).sum())))
            print("   best q over the incident's calls: mean %.3f; q == 1 for %d" % (
                e.best_q.mean(), int((e.best_q >= 0.999).sum())))
        # The headline split: the slow leak has no burst, so it is noticed only once its reading
        # crosses the alarm threshold, late; the other three families are noticed within seconds.
        for label, sub in (
            ("burst families (compound, cascade, split_brain)", agree[agree.family != "slow_leak"]),
            ("slow_leak", agree[agree.family == "slow_leak"]),
        ):
            e = sub[sub.calls_attributed > 0]
            if not len(e):
                continue
            print(f"-- attribution agrees, {label}: {len(e)} incidents")
            print("   median first call after onset %.2f s; median last decisive observation at %.2f s; "
                  "first call before it: %d of %d" % (
                      e.first_call_after_onset_s.median(), e.last_decisive_after_onset_s.median(),
                      int(e.first_before_last_decisive.sum()), len(e)))
            print("   first-call q: mean %.3f, q == 0 for %d, q == 1 for %d" % (
                e.first_q.mean(), int((e.first_q == 0).sum()), int((e.first_q >= 0.999).sum())))
        print("evaluator, all matched incidents: %d calls, %d informed" % (
            int(g.calls_evaluator.sum()), int(g.informed_evaluator.sum())))


if __name__ == "__main__":
    main()
