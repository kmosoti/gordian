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

Added after the run (reporting only, nothing adjusted): a paired breakdown against the memoryless
control `m3` on the same incidents (`a1c-smoke-paired.csv`): plain incidents unasked in both with a
wrong declaration in the arm and none in the control (newly wrong), of those the ones that also
carry a correct declaration (the recall added a declaration, it did not replace one), and plain
incidents correct in the control and not in the arm (displaced); and `eng_off_gated` against
`eng_off` column by column (monitoring alone).

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
    ctl = {(r["seed"], r["incident"]): r for r in rows(RUN / C.arm_name("m3") / "incidents.csv")}
    paired = []
    for name in names:
        s = {"arm": name, "newly_wrong": 0, "newly_wrong_also_correct": 0, "displaced": 0}
        for r in rows(RUN / C.arm_name(name) / "incidents.csv"):
            c = ctl[(r["seed"], r["incident"])]
            if r["tier"] != "plain" or int(r["escalations"]) > 0 or int(c["escalations"]) > 0:
                continue
            if int(r["wrong_declarations"]) > 0 and int(c["wrong_declarations"]) == 0:
                s["newly_wrong"] += 1
                s["newly_wrong_also_correct"] += int(r["correct_declarations"]) > 0
            s["displaced"] += int(c["correct_declarations"]) > 0 and int(r["correct_declarations"]) == 0
        paired.append(s)
    with open(C.OUT / "a1c-smoke-paired.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, ["arm", "newly_wrong", "newly_wrong_also_correct", "displaced"])
        w.writeheader()
        w.writerows(paired)
    off, gated = (rows(RUN / C.arm_name(n) / "incidents.csv") for n in ("eng_off", "eng_off_gated"))
    differing = sorted({k for x, y in zip(off, gated) for k in x if k != "run_id" and x[k] != y[k]})
    print("eng_off_gated vs eng_off, incidents.csv columns that differ:", differing)
    for s in paired:
        print("paired", s)
    print(",".join(fields))
    for r in out:
        print(",".join(str(r[f]) for f in fields))
    print("A1a reproduction:", sum(r["incidents_identical"] and r["results_identical"] for r in rep),
          "of", len(rep))
    first = next(r for r in out if r["arm"] == "eng_family_gated" and r["tier"] == "plain")
    print("first number: eng_family_gated plain wrong_unasked =", first["wrong_unasked"])


if __name__ == "__main__":
    main()
