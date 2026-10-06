"""Write the B4 manifests.

Usage:
  b4_manifests.py xcheck        R6's held-out manifest (b = 5, rho = 0.7) with only `source_revision`
                                replaced, run id kept, under the prefix `xcheck5-` (the byte-identity gate)
  b4_manifests.py xcheck-final  the same under the prefix `xcheck6-` (the gate again, with the final tree's
                                binary)
  b4_manifests.py tunefollow    the 100 tuning streams: the follow-up rule's grid (192 configurations) over
                                the comparator row's noticer, each under `always_escalate` at 16 s, beside
                                the same arm without the rule (stage F of `b4_common.py`)
  b4_manifests.py tuneselect    the same streams: every row of the table under the selection oracle, never
                                escalating, always escalating at 16 s, and each selector's grid (stage S;
                                needs b4-selected.json with stage F)
  b4_manifests.py heldout       the 200 held-out streams (20000-20199): every row under the selection
                                oracle (the ceiling), never, always, and each selector at the parameter stage
                                S chose, and the sensitivity arms (needs b4-selected.json complete)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. The manifests are written by B2's writer (`b2_manifests.write`): the
environment is captured by `gordian-run init-stream`, then the arms, the reasoner's (b, rho) and the
per-arm noticers are set.
"""

import sys

import b2_manifests as M2
import b3_manifests as M3
import b4_common as C


def row_arms(stem, noticer, params=None, grids=False):
    """The arms of one row: (name, policy, noticer). `params`: the chosen (t_s, k) when the row's
    selectors are the chosen ones; `grids`: every grid configuration instead."""
    out = [(C.oracle_arm(stem), C3_FIXED(), noticer), (C.never_arm(stem), C.never_policy(), noticer),
           (C.always_arm(stem), C.always_policy(), noticer)]
    if grids:
        out += [(C.thr_arm(stem, t), C.thr_policy(t), noticer) for t in C.THR_PERSIST_S]
        out += [(C.chg_arm(stem, k), C.chg_policy(k), noticer) for k in C.CHG_K]
    if params is not None:
        out.append((C.thr_arm(stem, params["thr_t_s"]), C.thr_policy(params["thr_t_s"]), noticer))
        out.append((C.chg_arm(stem, params["chg_k"]), C.chg_policy(params["chg_k"]), noticer))
    return out


def C3_FIXED():
    return C.C3.fixed_policy()


def write(rid, seeds, triples, run_seed, experiment):
    seen, arms, noticers = set(), [], {}
    for name, policy, noticer in triples:
        if name in seen:
            continue
        seen.add(name)
        arms.append((name, policy))
        noticers[name] = noticer
    M2.write(rid, seeds, arms, noticers, run_seed, experiment)


def tunefollow(sel4):
    sel3 = C.C3.load_selected()
    base = dict(M3.table_rows(sel3))[C.COMPARATOR]
    triples = [("fol_none", C.always_policy(), base)]
    for p in C.follow_grid():
        triples.append((f"fol_{C.follow_stem_name(p)}", C.always_policy(),
                        C.with_follow(base, C.follow_json(p))))
    write(C.run_id("tunefollow"), C.TUNING_SEEDS, triples, 13_100, "exploration-b4-tunefollow")


def tuneselect(sel4):
    sel3 = C.C3.load_selected()
    triples = []
    for stem, noticer in C.rows(sel3, sel4["follow"]["chosen"]["json"]):
        triples += row_arms(stem, noticer, grids=True)
    write(C.run_id("tuneselect"), C.TUNING_SEEDS, triples, 13_200, "exploration-b4-tuneselect")


def heldout(sel4):
    sel3 = C.C3.load_selected()
    rows = C.rows(sel3, sel4["follow"]["chosen"]["json"])
    triples = []
    for stem, noticer in rows:
        triples += row_arms(stem, noticer, params=sel4["select"][stem])
    by = dict(rows)
    # Sensitivity, never used to choose anything: the comparator row under every grid value of each
    # selector, and the follow-up rule at the other tolerances (the comparator row's noticer) under
    # always, the threshold and the change rule at the parameters the follow-up row chose.
    cmp_noticer = by[C.COMPARATOR]
    triples += row_arms(C.COMPARATOR, cmp_noticer, grids=True)
    sel_follow = sel4["select"][C.follow_stem(C.COMPARATOR)]
    sel3_base = dict(M3.table_rows(sel3))[C.COMPARATOR]
    for tol, cfg in sorted(sel4["follow"]["tolerance"].items(), key=lambda kv: int(kv[0])):
        stem = f"{C.follow_stem(C.COMPARATOR)}_tol{tol}"
        noticer = C.with_follow(sel3_base, cfg["json"])
        triples += row_arms(stem, noticer, params=sel_follow)[1:]  # no oracle for the sensitivity rows
    del sel3_base
    write(C.run_id("heldout"), C.HELDOUT_SEEDS, triples, 13_300, "exploration-b4-heldout")


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "xcheck":
        M2.xcheck("5")
    elif stage == "xcheck-final":
        M2.xcheck("6")
    elif stage in ("tunefollow", "tuneselect", "heldout"):
        sel4 = {} if stage == "tunefollow" else C.load_selected()
        {"tunefollow": tunefollow, "tuneselect": tuneselect, "heldout": heldout}[stage](sel4)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
