"""The B4 table: every B3 row, M2's medium at 100 ms, and the follow-up rule on the ramp rows, each under
every public selector (the threshold rule and the change rule, each at the parameter chosen on the tuning
streams for that row), beside the selection oracle at R5's delay as the labelled ceiling and
`always_escalate` and `never_escalate` as the bounds, on the 200 held-out streams at b = 5, rho = 0.7,
with 90% cluster-bootstrap intervals over streams, and the paired differences against ramp + split over
the re-anchor under the same selector.

Usage: b4_table.py   (after the held-out run; writes experiments/exploration/b4-table.csv,
                      b4-paired.csv, b4-followup.csv, b4-sensitivity.csv, b4-latency.csv and
                      b4-public-selector-table.md)

Quality, cost, calls, the notice measures and the selection measures (escalations by class, their cost
share, the fate of every class of notice) are `b4_stats.py`'s. The table makes no claim about which arm is
better; the paired differences are numbers. A row's selector parameter was chosen on the tuning streams for
that row (`b4_select.py`); nothing here chose anything.
"""

import pandas as pd

import b2_table as T2
import b4_common as C
import b4_stats as S
from gordian_analysis.load import load_stream_run
from gordian_analysis.stream import notice_latency

SELECTORS = ["thr", "chg", "always", "never", "oracle"]
SELECTOR_TEXT = {
    "thr": "public threshold", "chg": "public change", "always": "always escalate (16 s)",
    "never": "never escalate", "oracle": "selection oracle (ceiling)",
}

# The columns of the acceptance table (queue, B4 item 4).
MAIN = [
    ("quality", "hard-incident quality", "{:.3f}"),
    ("critical_misses_per_stream", "critical misses / stream", "{:.2f}"),
    ("plain_accuracy", "plain accuracy", "{:.3f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
    ("esc_decoy_per_stream", "escalations on decoys / stream", "{:.2f}"),
    ("esc_plain_per_stream", "escalations on plain / stream", "{:.2f}"),
    ("esc_decoy_cost_share", "decoy share of reasoner cost", "{:.3f}"),
    ("esc_plain_cost_share", "plain share of reasoner cost", "{:.3f}"),
]
# Beside it: the noticing the row has, and the fate of the notices on decoys.
BESIDE = [
    ("hard_anchor_correct_share", "anchor-correct", "{:.3f}"),
    ("leak_noticed_share", "leak noticed", "{:.3f}"),
    ("leak_quality", "leak quality", "{:.3f}"),
    ("notices_on_background_per_stream", "background notices / stream", "{:.2f}"),
    ("strict_precision", "strict precision", "{:.3f}"),
    ("decoy_notices_per_decoy", "notices on decoys / decoy", "{:.2f}"),
    ("notices_decoy_escalated_share", "decoy notices escalated", "{:.3f}"),
    ("notices_decoy_retired_before_share", "decoy notices retired before escalation", "{:.3f}"),
    ("notices_decoy_followup_before_share", "... by the follow-up rule", "{:.3f}"),
    ("leak_lost_share", "leaks lost to the rule", "{:.3f}"),
    ("esc_hard_per_stream", "escalations on hard / stream", "{:.2f}"),
    ("esc_leak_per_stream", "escalations on leaks / stream", "{:.2f}"),
    ("esc_background_per_stream", "escalations on background / stream", "{:.2f}"),
    ("unattributed_per_stream", "calls about no notice / stream", "{:.2f}"),
]
PAIRED = [m for m, _, _ in MAIN] + [
    "esc_hard_per_stream", "esc_leak_per_stream", "esc_background_per_stream",
    "hard_anchor_correct_share", "leak_noticed_share", "leak_quality",
    "notices_on_background_per_stream", "strict_precision", "decoy_notices_per_decoy",
    "notices_decoy_escalated_share", "notices_decoy_retired_before_share",
    "critical_miss_share", "substrate_s_per_stream",
]
FOLLOW_PAIRED = [
    "quality", "plain_accuracy", "critical_misses_per_stream", "calls_per_stream", "cost_s_per_stream",
    "esc_decoy_per_stream", "esc_plain_per_stream", "esc_leak_per_stream", "leak_quality",
    "leak_noticed_share", "leak_lost_share", "decoy_followup_hit_share",
    "followup_retired_decoy_per_stream", "followup_retired_leak_per_stream",
    "followup_retired_plain_per_stream", "followup_retired_hard_per_stream",
    "followup_retired_background_per_stream", "followup_before_decoy_per_stream",
    "notices_on_background_per_stream", "hard_anchor_correct_share",
]


def label(stem):
    sel3 = C.C3.load_selected()
    base = C.C3.__dict__  # noqa: F841 (labels are B3's)
    import b3_table as T3

    follow = stem.endswith("_follow")
    core = stem[: -len("_follow")] if follow else stem
    if core == C.MEDIUM_STEM:
        text = "Medium, 100 ms (M2)"
    else:
        text = T3.label(core, sel3)
    return text + (" + follow-up rule" if follow else "")


def arm_for(stem, selector, sel4):
    s = sel4["select"][stem]
    return {"oracle": C.oracle_arm(stem), "never": C.never_arm(stem), "always": C.always_arm(stem),
            "thr": C.thr_arm(stem, s["thr_t_s"]), "chg": C.chg_arm(stem, s["chg_k"])}[selector]


def param_text(stem, selector, sel4):
    s = sel4["select"][stem]
    return {"thr": f"t = {s['thr_t_s']} s", "chg": f"k = {s['chg_k']}"}.get(selector, "")


def main():
    sel4 = C.load_selected()
    sel3 = C.C3.load_selected()
    run = load_stream_run(C.RUNS / C.run_id("heldout"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    m = S.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    stems = [s for s, _ in C.rows(sel3, sel4["follow"]["chosen"]["json"])]

    def row_of(arm, **extra):
        i = m.row[arm]
        row = {"arm": arm, **extra}
        for key in S.MEASURES:
            row[key] = float(pts[key][i])
            row[f"{key}_lo"] = float(ci[key][0][i])
            row[f"{key}_hi"] = float(ci[key][1][i])
        return row

    rows = []
    for stem in stems:
        for sel in SELECTORS:
            rows.append(row_of(arm_for(stem, sel, sel4), config=stem, selector=sel, noticer=label(stem),
                               parameter=param_text(stem, sel, sel4)))
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "b4-table.csv", index=False)

    # Paired differences against the comparator row under the same selector (the same resamples).
    pairs, meta = [], []
    for stem in stems:
        for sel in SELECTORS:
            for measure in PAIRED:
                pairs.append((measure, arm_for(stem, sel, sel4), arm_for(C.COMPARATOR, sel, sel4)))
                meta.append((stem, sel, measure))
    res = m.paired(PAIRED, pairs, C.BOOT_SEED, C.N_RESAMPLES)
    paired = pd.DataFrame([
        {"config": stem, "selector": sel, "against": C.COMPARATOR, "measure": measure,
         "difference": res[p][0], "lower": res[p][1], "higher": res[p][2]}
        for (stem, sel, measure), p in zip(meta, pairs)
    ])
    paired.to_csv(C.OUT / "b4-paired.csv", index=False)

    # The follow-up rule's effect on its own base: a follow row against the same row without it.
    fpairs, fmeta = [], []
    for base in C.FOLLOW_BASES:
        for sel in SELECTORS:
            for measure in FOLLOW_PAIRED:
                fpairs.append((measure, arm_for(C.follow_stem(base), sel, sel4), arm_for(base, sel, sel4)))
                fmeta.append((base, sel, measure))
    fres = m.paired(FOLLOW_PAIRED, fpairs, C.BOOT_SEED, C.N_RESAMPLES)
    pd.DataFrame([
        {"base": base, "selector": sel, "measure": measure,
         "difference": fres[p][0], "lower": fres[p][1], "higher": fres[p][2]}
        for (base, sel, measure), p in zip(fmeta, fpairs)
    ]).to_csv(C.OUT / "b4-followup.csv", index=False)

    # Sensitivity: every arm of the run that the table does not use, as it is (no claim).
    used = set(table["arm"])
    sens = [row_of(a, config=a, selector="", noticer=a, parameter="") for a in m.names if a not in used]
    pd.DataFrame(sens).to_csv(C.OUT / "b4-sensitivity.csv", index=False)

    lat = []
    for stem in stems:
        t = notice_latency(run.arms[C.oracle_arm(stem)])
        t.insert(0, "config", stem)
        lat.append(t)
    pd.concat(lat).to_csv(C.OUT / "b4-latency.csv", index=False)

    text = []
    for sel in SELECTORS:
        sub = table[table["selector"] == sel]
        text += [f"### {SELECTOR_TEXT[sel]}", "", T2.md(sub.assign(noticer=sub["noticer"] + " " + sub["parameter"].map(
            lambda p: f"({p})" if p else "")), MAIN), "", T2.md(sub, BESIDE), ""]
    (C.OUT / "b4-public-selector-table.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
