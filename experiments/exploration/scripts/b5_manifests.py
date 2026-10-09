"""Write the B5 manifests.

Usage:
  b5_manifests.py features      the 100 tuning streams: the arms the feature log and the AUCs are read
                                from (the noticers of `b5_common.AUC_ROWS`; the arm's policy is a
                                placeholder, the log tool plays its own rule over the arm's noticer)
  b5_manifests.py tune K        the same streams: for budget K, the tuning grid over the tuning rows
                                (stage T of `b5_common.py`, written after the AUCs)
  b5_manifests.py table         the 200 held-out streams (20000-20199): every row of the table under
                                the oracle, never, always, and for each k the budgeted selector at its
                                tuned score and the first-k baseline (needs b5-selected.json)
  b5_manifests.py sweep         the same streams: the selection oracle at 8, 12, 16 and 20 s over the
                                sweep rows

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. The manifests are written by B2's writer (`b2_manifests.write`): the
environment is captured by `gordian-run init-stream`, then the arms, the reasoner's (b, rho) and the
per-arm noticers are set. The byte-identity gate's manifest is `b5_gate.py manifest`.
"""

import pathlib
import sys

# B5's scripts live here (the unit's territory, E1's convention); the earlier units' modules they build
# on (B2 to B4, C1, M2) live in scripts/, and the analysis package is this checkout's, not an installed one.
_ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(_ROOT / "scripts"))
sys.path.insert(0, str(_ROOT / "analysis"))

import json
import sys

import b2_manifests as M2
import b5_common as C


def write(rid, seeds, triples, run_seed, experiment):
    """`triples` are (arm name, policy, noticer); duplicates by name are written once."""
    seen, arms, noticers = set(), [], {}
    for name, policy, noticer in triples:
        if name in seen:
            continue
        seen.add(name)
        arms.append((name, policy))
        noticers[name] = noticer
    M2.write(rid, seeds, arms, noticers, run_seed, experiment)


def features():
    triples = [(f"feat_{stem}", C.never_policy(), C.row_json(stem)) for stem in C.AUC_ROWS]
    write(C.run_id("features"), C.TUNING_SEEDS, triples, 16_100, "exploration-b5-features")


def load_selected():
    with open(C.OUT / "b5-selected.json") as fh:
        return json.load(fh)


def row_arms(stem, noticer, selected):
    """The arms of one table row: the oracle, never, always, and per k the first-k baseline and the
    budgeted selector at the tuned score for that k."""
    out = [(C.oracle_arm(stem), C.oracle_policy(), noticer), (C.never_arm(stem), C.never_policy(), noticer),
           (C.always_arm(stem), C.always_policy(), noticer)]
    for k in C.KS:
        out.append((C.fk_arm(stem, k), C.budgeted_policy(k, C.FLAT), noticer))
        out.append((C.bud_arm(stem, k), C.budgeted_policy(k, selected["score"][str(k)]), noticer))
    return out


def table():
    sel = load_selected()
    triples = []
    for stem, noticer, _billed in C.table_rows():
        triples += row_arms(stem, noticer, sel)
    write(C.run_id("table"), C.HELDOUT_SEEDS, triples, 16_300, "exploration-b5-table")


def sweep():
    triples = []
    for stem, noticer, _billed in C.sweep_rows():
        for d in C.SWEEP_DELAYS_S:
            triples.append((C.oracle_arm(stem, d), C.oracle_policy(d), noticer))
    write(C.run_id("sweep"), C.HELDOUT_SEEDS, triples, 16_400, "exploration-b5-sweep")


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "features":
        features()
    elif stage == "table":
        table()
    elif stage == "sweep":
        sweep()
    elif stage == "tune" and len(sys.argv) > 2:
        import b5_tuning as T  # written with the tuning rule, after the AUCs

        T.write_manifest(int(sys.argv[2]))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
