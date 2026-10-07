"""A1a's smoke counts, from the run's `incidents.csv` files only (the existing per-incident
columns), with nothing tuned against them.

Usage: a1a_smoke.py     (reads artifacts/runs/a1a/a1a-smoke-b5-rho0.7; writes
                         experiments/exploration/a1a-smoke.csv)

Per arm and tier: incidents; incidents declared correctly with no escalation about them
(`correct_declarations` >= 1 and `escalations` = 0), the brief's measure; and, beside it as context
from the same file, incidents with any escalation and incidents with a wrong declaration and no
escalation. The tier column is the evaluator's, read here to report, never by an arm.
"""

import csv

import a1a_common as C

RUN = C.SMOKE_DIR / C.run_id("smoke")


FIELDS = ["arm", "tier", "incidents", "correct_unasked", "escalated", "wrong_unasked",
          "escalations"]


def counts(arm, tier, rows):
    unasked = [r for r in rows if int(r["escalations"]) == 0]
    return {
        "arm": arm,
        "tier": tier,
        "incidents": len(rows),
        "correct_unasked": sum(int(r["correct_declarations"]) > 0 for r in unasked),
        "escalated": len(rows) - len(unasked),
        "wrong_unasked": sum(int(r["wrong_declarations"]) > 0 for r in unasked),
        "escalations": sum(int(r["escalations"]) for r in rows),
    }


def main():
    out = []
    for name, _ in C.arms():
        with open(RUN / C.arm_name(name) / "incidents.csv") as fh:
            rows = list(csv.DictReader(fh))
        out.append(counts(name, "all", rows))
        for tier in sorted({r["tier"] for r in rows}):
            out.append(counts(name, tier, [r for r in rows if r["tier"] == tier]))
    with open(C.OUT / "a1a-smoke.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, FIELDS)
        w.writeheader()
        w.writerows(out)
    print(",".join(FIELDS))
    for r in out:
        print(",".join(str(r[f]) for f in FIELDS))


if __name__ == "__main__":
    main()
