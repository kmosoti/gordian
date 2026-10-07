"""E1's reproduction of A1a's, A1c's and A1d's kept smoke arms after `main` was merged into the
branch: the kept manifests replayed with this branch's binary under new run ids (`e1m-a1creplay-...`,
`e1m-a1dreplay-...`; only `run_id` and `source_revision` changed), every arm compared with the kept
run's arm.

`results.csv` gained two columns at the end in work item E1, so what is compared is the file with
its `run_id` column and its last two columns (`recall_declarations`, `noticer_ns`) removed;
`incidents.csv` with its `run_id` column removed. A1d's trace files (`_trace/engram-trace-*.csv`)
are compared byte for byte. The two new columns are reported, not compared: they are new numbers (a
medium arm charges a noticer and declares recalls); the check that `recall_declarations` equals the
number of rows of the arm's `recalls.csv` is recorded.

Usage: e1_merge_gate.py     writes experiments/exploration/e1-merge-reproduction.csv; exit 1 unless
                            every arm and every trace file matches
"""

import csv
import hashlib
import pathlib
import sys

import e1_common as C

KEPT = pathlib.Path("/home/user/gordian/artifacts/runs")
A1C = KEPT / "a1c" / "a1c-smoke-b5-rho0.7"
A1A = KEPT / "a1a" / "a1a-smoke-b5-rho0.7"
A1D = KEPT / "a1d" / "a1d-smoke-b5-rho0.7"
NEW_C = C.E1_DIR / "e1m-a1creplay-b5-rho0.7"
NEW_D = C.E1_DIR / "e1m-a1dreplay-b5-rho0.7"


def sha(b):
    return hashlib.sha256(b).hexdigest()


def stripped(path, last=0):
    """`run_id` column and the last `last` columns removed."""
    h = hashlib.sha256()
    with open(path, newline="") as fh:
        for r in csv.reader(fh):
            r = r[1:] if not last else r[1:-last]
            h.update(",".join(r).encode() + b"\n")
    return h.hexdigest()


def arms(d):
    return sorted(p.name for p in d.iterdir() if p.is_dir() and not p.name.startswith("_"))


def new_columns(path):
    with open(path, newline="") as fh:
        rows = list(csv.DictReader(fh))
    assert list(rows[0])[-2:] == ["recall_declarations", "noticer_ns"]
    return sum(int(r["recall_declarations"]) for r in rows), sum(int(r["noticer_ns"]) for r in rows)


def main():
    rows = []
    for label, kept, new in (("a1c", A1C, NEW_C), ("a1a", A1A, NEW_C), ("a1d", A1D, NEW_D)):
        for a in arms(kept):
            rd, ns = new_columns(new / a / "results.csv")
            recalls_rows = len(open(new / a / "recalls.csv").read().splitlines()) - 1
            rows.append({
                "kept_run": label, "arm": a,
                "incidents_identical": stripped(kept / a / "incidents.csv") == stripped(new / a / "incidents.csv"),
                "results_existing_columns_identical":
                    stripped(kept / a / "results.csv") == stripped(new / a / "results.csv", 2),
                "recall_declarations": rd, "recalls_csv_rows": recalls_rows,
                "recall_declarations_equal_rows": rd == recalls_rows, "noticer_ns": ns,
            })
    trace = []
    kt, nt = A1D / "_trace", NEW_D / "_trace"
    for f in sorted(p.name for p in kt.iterdir()):
        trace.append((f, (nt / f).exists() and sha((kt / f).read_bytes()) == sha((nt / f).read_bytes())))
    extra = sorted(set(p.name for p in nt.iterdir()) - set(p.name for p in kt.iterdir()))
    with open(C.OUT / "e1-merge-reproduction.csv", "w", newline="") as fh:
        out = csv.DictWriter(fh, list(rows[0]))
        out.writeheader()
        out.writerows(rows)
        for f, ok in trace:
            fh.write(f"# trace {f} identical {ok}\n")
    ok_all = True
    for label in ("a1a", "a1c", "a1d"):
        sub = [r for r in rows if r["kept_run"] == label]
        ok = sum(r["incidents_identical"] and r["results_existing_columns_identical"] for r in sub)
        eq = sum(r["recall_declarations_equal_rows"] for r in sub)
        print(f"{label}: {ok} of {len(sub)} arms reproduced; recall_declarations = recalls.csv rows in {eq}")
        ok_all &= ok == len(sub)
    tok = sum(ok for _, ok in trace)
    print(f"a1d trace files: {tok} of {len(trace)} identical; extra files in the new run: {extra}")
    ok_all &= tok == len(trace) and not extra
    sys.exit(0 if ok_all else 1)


if __name__ == "__main__":
    main()
