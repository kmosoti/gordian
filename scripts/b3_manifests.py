"""Write the B3 manifests.

Usage:
  b3_manifests.py xcheck      R6's held-out manifest (b = 5, rho = 0.7) with only `source_revision`
                              replaced, run id kept, under the prefix `xcheck3-` (the byte-identity gate)
  b3_manifests.py xcheck-final  the same under the prefix `xcheck4-` (the gate again, with the final
                              tree's binary)
  b3_manifests.py tuneramp    the 100 tuning streams: the rung at its default threshold and the 216 ramp
                              configurations over it (stage R of `b3_common.py`)
  b3_manifests.py tunesplit   the same streams: the rung and the later re-anchor row, and the 15 split
                              configurations over each (stage S)
  b3_manifests.py tunedelay   the same streams: for every row of the table, the selection oracle with
                              `hold_until_asked` at every delay of R5's grid (stage D; needs b3-selected.json
                              with stages R and S)
  b3_manifests.py heldout     the 200 held-out streams (20000-20199): the table's rows at R5's fixed delay
                              with the rung's retirement, the same rows with the hold at the delay stage D
                              chose, and the sensitivity arms (needs b3-selected.json complete)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. The manifests are written by B2's writer (`b2_manifests.write`): the
environment is captured by `gordian-run init-stream`, then the arms, the reasoner's (b, rho) and the
per-arm noticers are set.
"""

import sys

import b2_manifests as M2
import b3_common as C

BASE_ALONE = {
    "rung_z3": {"kind": "base", "base": "r3"},
    "reanchor": {"kind": "base", "base": "re2"},
    "reanchor_z3": {"kind": "base", "base": "re3"},
}
RUNG_Z2 = {"noticer": "rung", "notice_z": 2.0}


def alone_json(stem):
    if stem == "rung_z2":
        return dict(RUNG_Z2)
    return C.noticer_json(BASE_ALONE[stem])


def fixed_arms(configs):
    """The selection oracle at R5's fixed delay under each (stem, noticer json)."""
    arms, noticers = [], {}
    for stem, nj in configs:
        arm = C.arm_name(stem)
        arms.append((arm, C.fixed_policy()))
        noticers[arm] = nj
    return arms, noticers


def ramp_configs():
    return [(C.name_of("r3", ramp=p), C.composed("r3", ramp=p)) for p in C.ramp_grid()]


def split_configs():
    out = []
    for base in ("r3", "re2"):
        out += [(C.name_of(base, split=p), C.composed(base, split=p)) for p in C.split_grid()]
    return out


def table_rows(sel):
    """The table's rows: (stem, noticer json), in table order."""
    ramp = sel["ramp"]["chosen"]["params"]
    s1 = sel["split_r3"]["chosen"]["params"]
    s2 = sel["split_re2"]["chosen"]["params"]
    return [
        ("rung_z3", alone_json("rung_z3")),
        ("rung_z2", alone_json("rung_z2")),
        ("reanchor", alone_json("reanchor")),
        ("ramp_over_r3", C.composed("r3", ramp=ramp)),
        ("split_over_r3", C.composed("r3", split=s1)),
        ("ramp_split_over_r3", C.composed("r3", ramp=ramp, split=s1)),
        ("ramp_over_re2", C.composed("re2", ramp=ramp)),
        ("split_over_re2", C.composed("re2", split=s2)),
        ("ramp_split_over_re2", C.composed("re2", ramp=ramp, split=s2)),
        ("reanchor_z3", alone_json("reanchor_z3")),
        ("ramp_split_over_re3", C.composed("re3", ramp=ramp, split=s2)),
    ]


def sensitivity_rows(sel):
    """Arms played on the held-out streams beside the table, never used to choose anything: each ramp
    parameter at its other grid values with the rest at the chosen, two looser ramps, and the split's
    other grid values around the chosen over the re-anchor."""
    ramp = sel["ramp"]["chosen"]["params"]
    rows = []
    for key, values in (("gap_ms", C.RAMP_GAP_MS), ("max_step", C.RAMP_MAX_STEP),
                        ("max_drop", C.RAMP_MAX_DROP), ("min_readings", C.RAMP_MIN_READINGS),
                        ("min_rise", C.RAMP_MIN_RISE)):
        for v in values:
            if v != ramp[key]:
                p = dict(ramp, **{key: v})
                rows.append((C.name_of("r3", ramp=p), C.composed("r3", ramp=p)))
    # Two ramps outside the grid, looser: what a ramp that notices more costs.
    for p in (C.ramp_params(3000, 14, 4, 3, 10), C.ramp_params(3000, 20, 6, 2, 5)):
        rows.append((C.name_of("r3", ramp=p), C.composed("r3", ramp=p)))
    s2 = sel["split_re2"]["chosen"]["params"]
    for key, values in (("gap_ms", C.SPLIT_GAP_MS), ("min_burst", C.SPLIT_MIN_BURST)):
        for v in values:
            if v != s2[key]:
                p = dict(s2, **{key: v})
                rows.append((C.name_of("re2", split=p), C.composed("re2", split=p)))
    seen, out = set(), []
    for stem, nj in rows:
        if stem not in seen:
            seen.add(stem)
            out.append((stem, nj))
    return out


def tunedelay(sel):
    arms, noticers = [], {}
    for stem, nj in table_rows(sel):
        for d in C.DELAYS_S:
            arm = C.delay_arm_name(stem, d)
            arms.append((arm, C.hold_policy(d)))
            noticers[arm] = nj
    M2.write(C.run_id("tunedelay"), C.TUNING_SEEDS, arms, noticers, 12_500, "exploration-b3-tunedelay")


def heldout(sel):
    rows = table_rows(sel)
    arms, noticers = fixed_arms(rows + sensitivity_rows(sel))
    for stem, nj in rows:
        d = sel["delay"][stem]["delay_s"]
        arm = C.hold_arm_name(stem)
        arms.append((arm, C.hold_policy(d)))
        noticers[arm] = nj
    M2.write(C.run_id("heldout"), C.HELDOUT_SEEDS, arms, noticers, 12_700, "exploration-b3-heldout")


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "xcheck":
        M2.xcheck("3")
    elif stage == "xcheck-final":
        M2.xcheck("4")
    elif stage == "tuneramp":
        arms, noticers = fixed_arms([("rung_z3", alone_json("rung_z3"))] + ramp_configs())
        M2.write(C.run_id("tuneramp"), C.TUNING_SEEDS, arms, noticers, 12_300, "exploration-b3-tuneramp")
    elif stage == "tunesplit":
        controls = [("rung_z3", alone_json("rung_z3")), ("reanchor", alone_json("reanchor"))]
        arms, noticers = fixed_arms(controls + split_configs())
        M2.write(C.run_id("tunesplit"), C.TUNING_SEEDS, arms, noticers, 12_400, "exploration-b3-tunesplit")
    elif stage == "tunedelay":
        tunedelay(C.load_selected())
    elif stage == "heldout":
        heldout(C.load_selected())
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
