"""Write the R10 manifests.

Usage:
  r10_manifests.py xcheck       R6's held-out manifest at b = 5, rho = 0.7, unchanged but for
                                `source_revision`, to replay under its own run id
  r10_manifests.py heldout      b = 5, 2.5, 8 at rho = 0.7, the 200 held-out streams: both
                                pairings (selection oracle and notice oracle, with the rung's own
                                context and with `window` 40 s / N 256) at R5's tuned delay, R4's
                                oracle, never_escalate
  r10_manifests.py diag         b = 5, rho = 0.7, 100 diagnostic streams (seeds 32000-32099),
                                every ledger kept: the rung-context pair and R4's oracle
  r10_manifests.py sweep        b = 5, rho = 0.7, the 200 held-out streams, one run per notice
                                threshold z in (2.0, 1.0, 0.5): `oracle_selection` with the rung's
                                context at the primary delay; 10% of the ledgers kept

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the
driver refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. It starts from `gordian-run init-stream` (environment capture), then
sets the arms (each with its context builder), the reasoner's (b, rho), the rung's notice
threshold, the sampling rate and the backstop.
"""

import json
import subprocess
import sys

import r10_common as C

TIMEOUT_S = 3 * 3600


def arm_entry(name, policy, context):
    e = {"arm": name, "policy": policy}
    if context is not None:
        e["context"] = context
    return e


def write(rid, b, rho, seeds, arms, trace_rate, run_seed, experiment, z=None):
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    tmp = C.MANIFESTS / (rid + ".base")
    out = C.MANIFESTS / (rid + ".json")
    if out.exists():
        print(f"{out} exists; kept", file=sys.stderr)
        return
    tmp.unlink(missing_ok=True)
    subprocess.run(
        [
            str(C.BIN), "init-stream", "--run-id", rid, "--arms", "never_escalate=never_escalate",
            "--seed-start", str(seeds[0]), "--seed-count", str(seeds[1]),
            "--trace-sample-rate", str(trace_rate), "--experiment", experiment, "--out", str(tmp),
        ],
        check=True,
        cwd=C.ROOT,
    )
    m = json.load(open(tmp))
    tmp.unlink()
    names = [a[0] for a in arms]
    if len(set(names)) != len(names):
        raise SystemExit("duplicate arm names")
    m["arms"] = [arm_entry(*a) for a in arms]
    r = m["stream_params"]["reasoner"]
    r["b"] = b
    r["rho"] = rho
    if z is not None:
        m["rung"]["notice_z"] = z
    m["run_seed"] = run_seed
    m["isolation"]["timeout_secs"] = TIMEOUT_S
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def xcheck():
    """R6's held-out manifest at the primary setting with only `source_revision` replaced by HEAD,
    the run id kept."""
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=C.ROOT, capture_output=True, text=True,
                          check=True).stdout.strip()
    b, rho = C.PRIMARY
    rid = f"r6-heldout-{C.setting_id(b, rho)}"
    m = json.load(open(C.R6_RUNS / "_manifests" / f"{rid}.json"))
    assert m["run_id"] == rid
    m["source_revision"] = head
    out = C.MANIFESTS / f"xcheck-{rid}.json"
    if out.exists():
        raise SystemExit(f"{out} exists")
    C.MANIFESTS.mkdir(parents=True, exist_ok=True)
    with open(out, "w") as fh:
        json.dump(m, fh, indent=2)
        fh.write("\n")
    print(out)


def heldout():
    for i, (b, rho) in enumerate(C.SETTINGS):
        d = C.sel_delay_s(b, rho)
        write(C.run_id("heldout", b, rho), b, rho, C.HELDOUT_SEEDS, C.heldout_arms(d), 0.0, 10_100 + i,
              "exploration-r10-heldout")


def diag():
    b, rho = C.PRIMARY
    d = C.sel_delay_s(b, rho)
    write(C.run_id("diag", b, rho), b, rho, C.DIAG_SEEDS, C.diag_arms(d), 1.0, 10_200, "exploration-r10-trace")


def sweep():
    b, rho = C.PRIMARY
    d = C.sel_delay_s(b, rho)
    for k, z in enumerate(C.SWEEP_Z):
        arms = [(C.SWEEP_ARM, C.policy("oracle_selection", d), None)]
        write(C.run_id("sweep", b, rho, z), b, rho, C.HELDOUT_SEEDS, arms, 0.1, 10_300 + k,
              "exploration-r10-sweep", z=z)


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    {"xcheck": xcheck, "heldout": heldout, "diag": diag, "sweep": sweep}.get(stage, lambda: sys.exit(__doc__))()


if __name__ == "__main__":
    main()
