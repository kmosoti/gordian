"""A1a's smoke counts, from the run's `incidents.csv` files only (the existing per-incident
columns), with nothing tuned against them.

Usage: a1a_smoke.py     (reads artifacts/runs/a1a/a1a-smoke-b5-rho0.7; writes
                         experiments/exploration/a1a-smoke.csv)

Per arm and tier: incidents; incidents declared correctly with no escalation about them
(`correct_declarations` >= 1 and `escalations` = 0), the brief's measure; and, beside it as context
from the same file, incidents with any escalation and incidents with a wrong declaration and no
escalation. The tier column is the evaluator's, read here to report, never by an arm.
"""

import pandas as pd

import a1a_common as C

RUN = C.SMOKE_DIR / C.run_id("smoke")


def main():
    rows = []
    for name, _ in C.arms():
        df = pd.read_csv(RUN / C.arm_name(name) / "incidents.csv")
        for tier, g in [("all", df)] + list(df.groupby("tier")):
            unasked = g.escalations == 0
            rows.append({
                "arm": name,
                "tier": tier,
                "incidents": len(g),
                "correct_unasked": int(((g.correct_declarations > 0) & unasked).sum()),
                "escalated": int((~unasked).sum()),
                "wrong_unasked": int(((g.wrong_declarations > 0) & unasked).sum()),
                "escalations": int(g.escalations.sum()),
            })
    out = pd.DataFrame(rows)
    out.to_csv(C.OUT / "a1a-smoke.csv", index=False)
    print(out.to_string(index=False))


if __name__ == "__main__":
    main()
