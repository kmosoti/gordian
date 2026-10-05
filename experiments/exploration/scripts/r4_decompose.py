"""R4 exploratory decomposition of the oracle's gap, and a paired standard error for sizing.

Usage: r4_decompose.py        (reads the held-out runs; writes r4-decomposition.csv, r4-sizing.csv)

Exploration (nothing here tests a hypothesis). Arithmetic on recorded runs, not new runs.

The oracle has three privileges at once: it knows which anomalies are hard incidents (selection),
it asks when the decisive evidence has arrived (timing), and its context is exactly that evidence
(context). The margin compares it with baselines at equal cost and cannot say which privilege the
gap is made of; EXP-101 changes selection and timing, not context. So this script asks, of the
`always_escalate` arms that were run: what would a selector that escalated *only the hard
incidents*, with the contexts and the delay of that arm, have scored and cost?

  quality    the arm's own (the hard incidents' calls, their contexts and their timing are the arm's;
             an answer about one incident does not depend on what else was asked, DESIGN.md s11, and
             the rung's notice and context do not depend on escalation)
  cost       mean substrate cost + (the arm's calls on hard incidents per stream) x (the arm's mean
             tokens per call over all its calls) x the exchange rate. An ESTIMATE: contexts about
             hard incidents may be longer or shorter than average; r4-decomposition.csv carries the
             cost at 0.5x and 2x the mean tokens per call as a range.

It is an upper bound for what any selector can reach *with the rung's contexts*: perfect selection,
no cost of finding the incidents. It is not an arm and does not enter the margin.
"""

import numpy as np
import pandas as pd

import r4_common as C
from gordian_analysis.frontier import ClusterData, stream_table
from gordian_analysis.load import load_stream_run

PRICE = 250_000


def main():
    rows = []
    sizing = []
    for i, (b, rho) in enumerate(C.SETTINGS):
        sid = C.setting_id(b, rho)
        run = load_stream_run(C.RUNS / C.run_id("heldout", b, rho))
        orc = stream_table(run.arms[C.ORACLE])
        oq = orc["quality_num"].sum() / orc["quality_den"].sum()
        oc = orc["total_cost_ns"].mean()
        for name, arm in run.arms.items():
            if not name.startswith("always_d"):
                continue
            t = stream_table(arm)
            inc = arm.incidents
            hard = inc[inc["tier"] == "hard"]
            hard_calls = hard.groupby("seed")["escalations"].sum().reindex(t.index).fillna(0)
            tok_per_call = t["reasoner_tokens"].sum() / max(t["reasoner_calls"].sum(), 1)
            sub = t["substrate_ns"].mean()
            calls = hard_calls.mean()
            est = [sub + calls * tok_per_call * k * PRICE for k in (1.0, 0.5, 2.0)]
            kept = hard[hard["family"] != "slow_leak"]
            rows.append(
                {
                    "setting": sid, "arm": name,
                    "quality": t["quality_num"].sum() / t["quality_den"].sum(),
                    "arm_cost_s": t["total_cost_ns"].mean() / C.NS,
                    "arm_calls_per_stream": t["reasoner_calls"].mean(),
                    "hard_calls_per_stream": calls,
                    "tokens_per_call": tok_per_call,
                    "hard_only_cost_s_est": est[0] / C.NS,
                    "hard_only_cost_s_lo": est[1] / C.NS,
                    "hard_only_cost_s_hi": est[2] / C.NS,
                    "oracle_quality": oq, "oracle_cost_s": oc / C.NS,
                    "hard_escalated_share": float((kept["escalations"] > 0).mean()),
                    "hard_informed_share": float((kept["informed_escalations"] > 0).mean()),
                    "hard_correct_share": float(kept["correct_by_deadline"].mean()),
                }
            )
        # paired standard error of a quality difference at the held-out size, between the oracle
        # and the highest-quality always arm, and between that arm and the ablation
        data = ClusterData.from_arms(run.arms)
        always = [n for n in run.arms if n.startswith("always_d")]
        qs, _ = data.estimates()
        top = always[int(np.argmax([qs[data.row(n)] for n in always]))]
        rng = np.random.default_rng(777 + i)
        idx = rng.integers(0, data.n_streams, size=(4000, data.n_streams))
        q, _ = data.estimates(idx)
        for a, bb in ((C.ORACLE, top), (top, C.ABLATION), (C.ORACLE, C.ABLATION)):
            d = q[data.row(a)] - q[data.row(bb)]
            se = float(np.std(d, ddof=1))
            sizing.append(
                {
                    "setting": sid, "a": a, "b": bb, "diff": float(qs[data.row(a)] - qs[data.row(bb)]),
                    "se_at_200_streams": se,
                    # one-sided 5% test, 90% power: streams for a margin m need SE <= m / 2.926
                    "streams_for_margin_0.10": int(np.ceil(200 * (se / (0.10 / 2.926)) ** 2)),
                    "streams_for_margin_0.05": int(np.ceil(200 * (se / (0.05 / 2.926)) ** 2)),
                }
            )
    d = pd.DataFrame(rows)
    d.to_csv(C.OUT / "r4-decomposition.csv", index=False, float_format="%.6g")
    s = pd.DataFrame(sizing)
    s.to_csv(C.OUT / "r4-sizing.csv", index=False, float_format="%.6g")
    pd.set_option("display.width", 250, "display.max_columns", 40, "display.max_rows", 200)
    print(d.round(3).to_string(index=False))
    print(s.round(3).to_string(index=False))


if __name__ == "__main__":
    main()
