"""The B5 table: every row (B3's eleven, C1's dataflow row, M2's and M3's media) under the call-budgeted
selector at each k, beside the first-k baseline of the same budget, the selection oracle at 16 s (the
labelled ceiling for hard quality and cost), `always_escalate` at 16 s and `never_escalate`, on the 200
held-out streams at b = 5, rho = 0.7, with 90% cluster-bootstrap intervals over streams and the paired
differences against the comparator (ramp + split over the re-anchor) under the same selector at the
same k.

Usage: b5_table.py   (after the held-out table run; writes experiments/exploration/b5-table.csv,
                      b5-paired.csv, b5-selectivity.csv and b5-table.md)

Verified decisions, hard quality, critical misses, calls and cost are `b5_stats.py`'s (R2, R3 of
`b5_common.py`). The table makes no claim about which arm is better; the paired differences are numbers.
Each row's billing is written in its label (R5). Nothing here chose anything: the score of each k was
chosen on the tuning streams (`b5_select.py`).
"""

import pandas as pd

import b2_table as T2
import b5_common as C
import b5_stats as S
from gordian_analysis.load import load_stream_run

SELECTORS = ["oracle", "always", "never"] + [f"{kind}{k}" for k in C.KS for kind in ("fk", "bud")]
SELECTOR_TEXT = {
    "oracle": "selection oracle at 16 s (ceiling for hard quality and cost)",
    "always": "always escalate at 16 s", "never": "never escalate",
    **{f"fk{k}": f"first {k} anomalies to become ready (score-free, k = {k})" for k in C.KS},
    **{f"bud{k}": f"budgeted public score, k = {k}" for k in C.KS},
}

MAIN = [
    ("verified_per_stream", "verified decisions / stream", "{:.2f}"),
    ("verified_plain_per_stream", "of them plain", "{:.2f}"),
    ("verified_hard_per_stream", "of them hard", "{:.2f}"),
    ("critical_misses_per_stream", "critical misses / stream", "{:.2f}"),
    ("quality", "hard quality", "{:.3f}"),
    ("calls_per_stream", "calls / stream", "{:.2f}"),
    ("cost_s_per_stream", "cost s / stream", "{:.3f}"),
]
BESIDE = [
    ("leak_quality", "leak quality", "{:.3f}"),
    ("plain_accuracy", "plain accuracy", "{:.3f}"),
    ("esc_hard_per_stream", "calls on hard / stream", "{:.2f}"),
    ("esc_leak_per_stream", "calls on leaks / stream", "{:.2f}"),
    ("esc_plain_per_stream", "calls on plain / stream", "{:.2f}"),
    ("esc_decoy_per_stream", "calls on decoys / stream", "{:.2f}"),
    ("esc_background_per_stream", "calls on background / stream", "{:.2f}"),
    ("hard_anchor_correct_share", "anchor-correct", "{:.3f}"),
    ("notices_on_background_per_stream", "background notices / stream", "{:.2f}"),
    ("substrate_s_per_stream", "substrate s / stream", "{:.4f}"),
]
PAIRED = [m for m, _, _ in MAIN] + [m for m, _, _ in BESIDE] + [
    "critical_miss_share", "verified_plain_per_stream", "hard_incidents_per_stream"]
SELECTIVITY = ["verified_per_stream", "verified_plain_per_stream", "verified_hard_per_stream",
               "critical_misses_per_stream", "quality", "calls_per_stream", "cost_s_per_stream"]


def label(stem):
    import b3_table as T3

    sel3 = C.C3.load_selected()
    if stem.startswith("df_"):
        what = "comparator's rules" if stem == "df_" + C.COMPARATOR else "re-anchor alone"
        return f"Dataflow (C1), {what}, billed"
    if stem[:3] in ("m2_", "m3_"):
        unit, tick = stem[:2].upper(), stem.split("_t")[1]
        return f"Medium {tick} ms ({unit}'s frozen graph), billed"
    return T3.label(stem, sel3) + (" (comparator)" if stem == C.COMPARATOR else "") + ", unbilled"


def arm_for(stem, selector):
    if selector == "oracle":
        return C.oracle_arm(stem)
    if selector == "never":
        return C.never_arm(stem)
    if selector == "always":
        return C.always_arm(stem)
    kind = selector.rstrip("0123456789")
    k = int(selector[len(kind):])
    return {"fk": C.fk_arm, "bud": C.bud_arm}[kind](stem, k)


def main():
    run = load_stream_run(C.RUNS / C.run_id("table"))
    seeds = [int(s) for s in next(iter(run.arms.values())).results["seed"]]
    assert seeds == list(range(C.HELDOUT_SEEDS[0], C.HELDOUT_SEEDS[0] + C.HELDOUT_SEEDS[1])), "held-out seeds"
    m = S.Measures(run)
    pts = m.points()
    ci = m.boot(C.BOOT_SEED, C.N_RESAMPLES)
    stems = [s for s, _, _ in C.table_rows()]
    billed = {s: b for s, _, b in C.table_rows()}

    def row_of(arm, **extra):
        i = m.row[arm]
        row = {"arm": arm, **extra}
        for key in S.MEASURES:
            row[key] = float(pts[key][i])
            row[f"{key}_lo"] = float(ci[key][0][i])
            row[f"{key}_hi"] = float(ci[key][1][i])
        return row

    rows = [row_of(arm_for(stem, sel), row=stem, billed=billed[stem], selector=sel, noticer=label(stem))
            for stem in stems for sel in SELECTORS]
    table = pd.DataFrame(rows)
    table.to_csv(C.OUT / "b5-table.csv", index=False)

    # Paired differences against the comparator under the same selector (same k, same resamples).
    pairs, meta = [], []
    for stem in stems:
        for sel in SELECTORS:
            for measure in PAIRED:
                pairs.append((measure, arm_for(stem, sel), arm_for(C.COMPARATOR, sel)))
                meta.append((stem, sel, measure))
    res = m.paired(PAIRED, pairs, C.BOOT_SEED, C.N_RESAMPLES)
    pd.DataFrame([
        {"row": stem, "selector": sel, "against": C.COMPARATOR, "measure": measure,
         "difference": res[p][0], "lower": res[p][1], "higher": res[p][2]}
        for (stem, sel, measure), p in zip(meta, pairs)
    ]).to_csv(C.OUT / "b5-paired.csv", index=False)

    # Selectivity, row by row: what the score buys over the first k, over never asking, and how far the
    # budgeted arms sit from always asking and from the oracle (paired within a row).
    spairs, smeta = [], []
    for stem in stems:
        for k in C.KS:
            for measure in SELECTIVITY:
                for name, a, b in (
                    ("bud_minus_fk", C.bud_arm(stem, k), C.fk_arm(stem, k)),
                    ("bud_minus_never", C.bud_arm(stem, k), C.never_arm(stem)),
                    ("fk_minus_never", C.fk_arm(stem, k), C.never_arm(stem)),
                    ("bud_minus_always", C.bud_arm(stem, k), C.always_arm(stem)),
                    ("bud_minus_oracle", C.bud_arm(stem, k), C.oracle_arm(stem)),
                ):
                    spairs.append((measure, a, b))
                    smeta.append((stem, k, name, measure))
        for measure in SELECTIVITY:
            for name, a, b in (("always_minus_never", C.always_arm(stem), C.never_arm(stem)),
                               ("oracle_minus_never", C.oracle_arm(stem), C.never_arm(stem))):
                spairs.append((measure, a, b))
                smeta.append((stem, 0, name, measure))
    sres = m.paired(SELECTIVITY, spairs, C.BOOT_SEED, C.N_RESAMPLES)
    pd.DataFrame([
        {"row": stem, "k": k, "contrast": name, "measure": measure,
         "difference": sres[p][0], "lower": sres[p][1], "higher": sres[p][2]}
        for (stem, k, name, measure), p in zip(smeta, spairs)
    ]).to_csv(C.OUT / "b5-selectivity.csv", index=False)

    text = []
    for sel in SELECTORS:
        sub = table[table["selector"] == sel]
        text += [f"### {SELECTOR_TEXT[sel]}", "", T2.md(sub, MAIN), "", T2.md(sub, BESIDE), ""]
    (C.OUT / "b5-table.md").write_text("\n".join(text))
    print("\n".join(text))


if __name__ == "__main__":
    main()
