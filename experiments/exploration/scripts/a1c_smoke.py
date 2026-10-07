"""A1c's smoke counts, from the run's `incidents.csv` and `results.csv` files only (the existing
columns), with nothing tuned against them; and the reproduction of A1a's eight arms.

Usage: a1c_smoke.py   (reads artifacts/runs/a1c/a1c-smoke-b5-rho0.7 and A1a's kept smoke run;
                       writes experiments/exploration/a1c-smoke.csv, a1c-smoke-streams.csv and
                       a1c-smoke-a1a-reproduction.csv)

Per arm and tier, A1a's measure (`a1a_smoke.counts`, imported): incidents; incidents declared
correctly with no escalation about them (`correct_declarations` >= 1 and `escalations` = 0);
incidents with any escalation; incidents with a wrong declaration and no escalation. Per arm, from
`results.csv`: reasoner calls, `bill_compute` (the arm's modelled compute, which carries the
noticer's and the gate's checks; `total_cost_ns` omits the noticer's charge for medium arms since
M2) and `components_run` (the consistency checks are component runs). Per stream: plain incidents
with a wrong declaration and no escalation, for the plain growth with streams. The tier column is
the evaluator's, read here to report, never by an arm.

A1a's eight arms in this run must reproduce A1a's kept run: `incidents.csv` and `results.csv`
identical row for row once the `run_id` column (which names the run) is removed.
"""

import csv
import hashlib

import a1a_smoke
import a1c_common as C

RUN = C.SMOKE_DIR / C.run_id("smoke")


def rows(path):
    with open(path) as fh:
        return list(csv.DictReader(fh))


def stripped_sha(path):
    """SHA-256 of a CSV with its first column (`run_id`) removed."""
    h = hashlib.sha256()
    with open(path, newline="") as fh:
        for r in csv.reader(fh):
            h.update(",".join(r[1:]).encode() + b"\n")
    return h.hexdigest()


def main():
    names = [n for n, _ in C.arms()]
    out = []
    streams = []
    for name in names:
        d = RUN / C.arm_name(name)
        inc = rows(d / "incidents.csv")
        res = rows(d / "results.csv")
        extra = {
            "calls": sum(int(r["reasoner_calls"]) for r in res),
            "bill_compute": sum(int(r["bill_compute"]) for r in res),
            "components_run": sum(int(r["components_run"]) for r in res),
        }
        out.append(dict(a1a_smoke.counts(name, "all", inc), **extra))
        for tier in sorted({r["tier"] for r in inc}):
            out.append(dict(a1a_smoke.counts(name, tier, [r for r in inc if r["tier"] == tier]),
                            calls="", bill_compute="", components_run=""))
        for seed in sorted({r["seed"] for r in inc}, key=int):
            plain = [r for r in inc if r["seed"] == seed and r["tier"] == "plain"]
            unasked = [r for r in plain if int(r["escalations"]) == 0]
            streams.append({
                "arm": name, "seed": seed,
                "plain_wrong_unasked": sum(int(r["wrong_declarations"]) > 0 for r in unasked),
                "plain_correct_unasked": sum(int(r["correct_declarations"]) > 0 for r in unasked),
            })
    fields = a1a_smoke.FIELDS + ["calls", "bill_compute", "components_run"]
    with open(C.OUT / "a1c-smoke.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fields)
        w.writeheader()
        w.writerows(out)
    with open(C.OUT / "a1c-smoke-streams.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, ["arm", "seed", "plain_wrong_unasked", "plain_correct_unasked"])
        w.writeheader()
        w.writerows(streams)
    rep = []
    for name in C.A1A_ARMS:
        a = C.A1A_SMOKE / C.arm_name(name)
        b = RUN / C.arm_name(name)
        rep.append({
            "arm": name,
            "incidents_identical": stripped_sha(a / "incidents.csv") == stripped_sha(b / "incidents.csv"),
            "results_identical": stripped_sha(a / "results.csv") == stripped_sha(b / "results.csv"),
        })
    with open(C.OUT / "a1c-smoke-a1a-reproduction.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, ["arm", "incidents_identical", "results_identical"])
        w.writeheader()
        w.writerows(rep)
    print(",".join(fields))
    for r in out:
        print(",".join(str(r[f]) for f in fields))
    print("A1a reproduction:", sum(r["incidents_identical"] and r["results_identical"] for r in rep),
          "of", len(rep))
    first = next(r for r in out if r["arm"] == "eng_family_gated" and r["tier"] == "plain")
    print("first number: eng_family_gated plain wrong_unasked =", first["wrong_unasked"])


if __name__ == "__main__":
    main()
