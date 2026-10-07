"""E1's tuning rule, applied to the tuning runs (the module documentation of
`crates/gordian-run/src/stream/arms/noticer_record.rs`, "Parameters and the tuning rule"; the two
cases it leaves open are decided there before any run, in words this file repeats).

Usage: e1_select.py stage1 | stage2

stage1  reads artifacts/runs/e1/e1-tune1-b5-rho0.7: for each form and each reset setting, the three
        levels under the policy `never`. A level meets the bound when, over the tuning streams, the
        recalls whose stored answer was right for its own incident (`recalls_correct_source_right +
        recalls_wrong_source_right`) are wrong at most 0.20 of the time (`recalls_wrong_source_right`
        over them; 0 when there are none) and at most 0.25 such wrong recalls per stream
        (`recalls_wrong_source_right` summed over the streams, over their number). The level kept is
        the one that meets the bound with the most hard incidents unasked correct
        (`unasked_correct_hard`, summed), ties to the richer level (timing over bands over kinds);
        when none meets the bound the level kept is `timing` and the cell is marked as failing it;
        when the best meeting level has no hard incident unasked correct the cell is marked as one
        the bound leaves empty and the level kept is `timing`. Writes experiments/exploration/
        e1-selected.json ("stage1") and e1-tuning-stage1.csv.
stage2  reads artifacts/runs/e1/e1-tune2-b5-rho0.7 (and stage 1's choices): for each form and reset
        setting, at the stage 1 level, `every` at k of 2, 4 and 8, and `on_contradiction`. The `k`
        kept is the one meeting the bound with the most hard incidents unasked correct (ties to the
        smaller k); when none meets it, the one with the smallest collision share, ties to the larger
        k. Writes "stage2" and "heldout" (the values every held-out cell runs with) into
        e1-selected.json and e1-tuning-stage2.csv.
"""

import csv
import json
import sys

import pandas as pd

import e1_common as C

SEL = C.OUT / "e1-selected.json"
LEVEL_RANK = {l: i for i, l in enumerate(C.LEVELS)}  # richer = larger


def stats(arm_dir):
    """The statistics the rule reads, from one arm's memory.csv and results.csv."""
    m = pd.read_csv(arm_dir / "memory.csv")
    streams = len(m)
    right = int((m["recalls_correct_source_right"] + m["recalls_wrong_source_right"]).sum())
    collisions = int(m["recalls_wrong_source_right"].sum())
    share = collisions / right if right else 0.0
    res = pd.read_csv(arm_dir / "results.csv")
    return {
        "streams": streams,
        "recalls": int(m["recalls"].sum()),
        "right_source_recalls": right,
        "collision_recalls": collisions,
        "collision_share": share,
        "collisions_per_stream": collisions / streams,
        "inherited_recalls": int(m["recalls_wrong_source_wrong"].sum()),
        "unasked_correct_hard": int(m["unasked_correct_hard"].sum()),
        "hard_recurrences_unasked_correct": int(m["hard_recurrences_unasked_correct"].sum()),
        "reasoner_calls": int(res["reasoner_calls"].sum()),
        "meets_bound": share <= C.BOUND_SHARE and collisions / streams <= C.BOUND_PER_STREAM,
    }


def arm_dir(run, form, level, confirm, reset):
    return run / C.arm_name(C.name_of(form, level, confirm, reset))


def stage1(run):
    out, rows = {}, []
    for form in C.FORMS:
        out[form] = {}
        for reset in C.RESETS:
            cands = {}
            for level in C.LEVELS:
                s = stats(arm_dir(run, form, level, "never", reset))
                cands[level] = s
                rows.append({"stage": 1, "form": form, "reset": reset, "level": level, "confirm": "never", **s})
            meeting = [l for l in C.LEVELS if cands[l]["meets_bound"]]
            if not meeting:
                kept, note = "timing", "fails_bound"
            else:
                kept = max(meeting, key=lambda l: (cands[l]["unasked_correct_hard"], LEVEL_RANK[l]))
                note = ""
                if cands[kept]["unasked_correct_hard"] == 0:
                    kept, note = "timing", "bound_leaves_cell_empty"
            out[form][str(reset).lower()] = {"level": kept, "note": note}
    return out, rows


def stage2(run, stage1_choice):
    out, rows = {}, []
    for form in C.FORMS:
        out[form] = {}
        for reset in C.RESETS:
            level = stage1_choice[form][str(reset).lower()]["level"]
            cands = {}
            for k in C.KS:
                s = stats(arm_dir(run, form, level, {"every": {"k": k}}, reset))
                cands[k] = s
                rows.append({"stage": 2, "form": form, "reset": reset, "level": level, "confirm": f"k{k}", **s})
            contra = stats(arm_dir(run, form, level, "on_contradiction", reset))
            rows.append({"stage": 2, "form": form, "reset": reset, "level": level, "confirm": "contra", **contra})
            meeting = [k for k in C.KS if cands[k]["meets_bound"]]
            if meeting:
                k = max(meeting, key=lambda k: (cands[k]["unasked_correct_hard"], -k))
                note = ""
            else:
                k = min(C.KS, key=lambda k: (cands[k]["collision_share"], -k))
                note = "fails_bound"
            out[form][str(reset).lower()] = {"level": level, "k": k, "note": note}
    return out, rows


def write_rows(name, rows):
    with open(C.OUT / name, "w", newline="") as fh:
        w = csv.DictWriter(fh, list(rows[0]))
        w.writeheader()
        w.writerows(rows)


def main():
    if sys.argv[1:] == ["stage1"]:
        run = C.E1_DIR / C.run_id("tune1")
        choice, rows = stage1(run)
        sel = json.load(open(SEL)) if SEL.exists() else {}
        sel["stage1"] = choice
        write_rows("e1-tuning-stage1.csv", rows)
    elif sys.argv[1:] == ["stage2"]:
        sel = json.load(open(SEL))
        run = C.E1_DIR / C.run_id("tune2")
        choice, rows = stage2(run, sel["stage1"])
        sel["stage2"] = choice
        sel["heldout"] = {
            form: {r: {"never": {"level": sel["stage1"][form][r]["level"]},
                       "every": {"level": choice[form][r]["level"], "k": choice[form][r]["k"]},
                       "on_contradiction": {"level": sel["stage1"][form][r]["level"]}}
                   for r in ("true", "false")}
            for form in C.FORMS}
        write_rows("e1-tuning-stage2.csv", rows)
    else:
        sys.exit(__doc__)
    with open(SEL, "w") as fh:
        json.dump(sel, fh, indent=2)
        fh.write("\n")
    print(json.dumps(sel, indent=1))


if __name__ == "__main__":
    main()
