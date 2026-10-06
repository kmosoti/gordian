"""B3's scripts (`scripts/b3_*.py`): the arithmetic whose errors would change a choice or a row without
anyone noticing: the selection rules (objective, budget, ties), the arm spellings the Rust side
reads, and the table's rows. The scripts are exploration code outside the package, so the test puts
the directories on the path. It reads no run output: every input is built here."""

import sys
from pathlib import Path

import pandas as pd
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))

import b3_common as C  # noqa: E402
import b3_manifests as M  # noqa: E402
import b3_select as SEL  # noqa: E402


def frame(rows):
    base = {"leak_noticed_share": 0.0, "leak_anchor_correct_share": 0.0, "hard_anchor_correct_share": 0.0,
            "notices_on_background_per_stream": 1.0, "p_gap_ms": 2000, "p_min_readings": 4, "p_min_rise": 10,
            "p_max_step": 10, "p_max_drop": 0, "p_min_burst": 2}
    return pd.DataFrame([{**base, **r} for r in rows])


def test_the_grids_are_the_products_the_rules_name():
    assert len(C.ramp_grid()) == 3 * 2 * 3 * 4 * 3 == 216
    assert len(C.split_grid()) == 5 * 3 == 15
    # Every configuration has its own arm name, and the names carry no character the run output or a
    # path would mind.
    stems = [C.name_of("r3", ramp=p) for p in C.ramp_grid()]
    assert len(set(stems)) == len(stems)
    stems += [C.name_of(b, split=p) for b in ("r3", "re2") for p in C.split_grid()]
    assert len(set(stems)) == len(stems)
    for s in stems:
        assert all(c.isalnum() or c in "_-." for c in s), s


def test_the_manifest_spellings_are_what_the_rust_side_reads():
    ramp = C.ramp_params(2000, 10, 2, 4, 15)
    split = C.split_params(3000, 3)
    j = C.composed("re2", ramp=ramp, split=split)
    assert set(j) == {"noticer", "base", "ramp", "split"} and j["noticer"] == "composed"
    assert set(j["ramp"]) == {"gap_ns", "max_step", "max_drop", "min_readings", "min_rise"}
    assert j["ramp"]["gap_ns"] == 2_000_000_000 and j["split"] == {"gap_ns": 3_000_000_000, "min_burst": 3}
    assert j["base"]["noticer"] == "reanchor"
    assert set(j["base"]) == {"noticer", "notice_z", "gap_ns", "min_burst", "isolation"}
    # The default-threshold base writes no `notice_z`, as the Rust side does.
    assert "notice_z" not in C.composed("r3", ramp=ramp)["base"]
    assert "notice_z" not in C.composed("re3", ramp=ramp)["base"]
    # Pieces that are not given are not written.
    assert "split" not in C.composed("r3", ramp=ramp) and "ramp" not in C.composed("r3", split=split)
    assert C.name_of("re2", ramp=ramp, split=split) == "ramp_g2000_s10_d2_n4_r15_split_g3000_b3_over_re2"


def test_the_ramp_choice_is_the_highest_leak_noticed_within_the_budget_and_ties_follow_the_rule():
    df = frame([
        {"stem": "over", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 7.0},
        {"stem": "a", "leak_noticed_share": 0.9, "leak_anchor_correct_share": 0.9},
        {"stem": "b", "leak_noticed_share": 0.95, "leak_anchor_correct_share": 0.2},
        {"stem": "c", "leak_noticed_share": 0.95, "leak_anchor_correct_share": 0.3},
    ])
    pick, tied, feasible, within = SEL.choose(df, C.TUNE_RAMP_OBJECTIVE, C.ramp_tie_keys())
    # `over` is the best on the objective and is outside the budget; of the two that tie on leak
    # noticed, the one with the higher leak anchor-correct wins.
    assert (pick["stem"], tied, feasible, within) == ("c", 2, 3, True)
    # The budget is inclusive.
    df.loc[0, "notices_on_background_per_stream"] = C.BACKGROUND_BUDGET
    assert SEL.choose(df, C.TUNE_RAMP_OBJECTIVE, C.ramp_tie_keys())[0]["stem"] == "over"


def test_ramp_ties_after_the_first_two_keys_go_to_fewer_background_notices_then_conservative_parameters():
    keys = C.ramp_tie_keys()
    df = frame([
        {"stem": "x", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.5},
        {"stem": "y", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.0, "p_min_readings": 3},
        {"stem": "z", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.0, "p_min_readings": 5},
        {"stem": "w", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.0, "p_min_readings": 5,
         "p_min_rise": 15},
        {"stem": "v", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.0, "p_min_readings": 5,
         "p_min_rise": 15, "p_max_step": 14},
        {"stem": "u", "leak_noticed_share": 1.0, "notices_on_background_per_stream": 1.0, "p_min_readings": 5,
         "p_min_rise": 15, "p_max_drop": 2},
    ])
    pick, tied, _, _ = SEL.choose(df, C.TUNE_RAMP_OBJECTIVE, keys)
    # fewest background notices excludes x; then the larger min_readings (z, w, v, u at 5), then the
    # larger min_rise (w, v, u at 15), then the smaller max_step (w, u), then the smaller max_drop (w).
    assert (pick["stem"], tied) == ("w", 6)
    # The gap is the last key: the smaller one.
    df2 = frame([{"stem": "g3", "leak_noticed_share": 1.0, "p_gap_ms": 3000},
                 {"stem": "g16", "leak_noticed_share": 1.0, "p_gap_ms": 1600}])
    assert SEL.choose(df2, C.TUNE_RAMP_OBJECTIVE, keys)[0]["stem"] == "g16"


def test_the_split_choice_ties_go_to_the_larger_gap_then_the_larger_burst_and_the_budget_binds():
    keys = C.split_tie_keys()
    df = frame([
        {"stem": "a", "hard_anchor_correct_share": 0.95, "p_gap_ms": 1000, "p_min_burst": 4},
        {"stem": "b", "hard_anchor_correct_share": 0.95, "p_gap_ms": 5000, "p_min_burst": 2},
        {"stem": "c", "hard_anchor_correct_share": 0.95, "p_gap_ms": 5000, "p_min_burst": 3},
        {"stem": "d", "hard_anchor_correct_share": 0.99, "notices_on_background_per_stream": 6.9},
    ])
    pick, tied, feasible, within = SEL.choose(df, C.TUNE_SPLIT_OBJECTIVE, keys)
    assert (pick["stem"], tied, feasible, within) == ("c", 3, 3, True)
    # With nothing within the budget the one with the fewest notices on background is chosen and the
    # flag says the budget was not met.
    over = frame([
        {"stem": "p", "hard_anchor_correct_share": 0.99, "notices_on_background_per_stream": 8.0},
        {"stem": "q", "hard_anchor_correct_share": 0.90, "notices_on_background_per_stream": 7.5},
    ])
    pick, _, _, within = SEL.choose(over, C.TUNE_SPLIT_OBJECTIVE, keys)
    assert pick["stem"] == "q" and not within


def _sel():
    ramp = C.ramp_params(2000, 10, 2, 4, 10)
    return {"ramp": {"chosen": {"params": ramp}},
            "split_r3": {"chosen": {"params": C.split_params(2000, 2)}},
            "split_re2": {"chosen": {"params": C.split_params(3000, 3)}}}


def test_the_table_rows_are_the_names_the_analysis_reads_and_each_is_one_noticer():
    rows = M.table_rows(_sel())
    stems = [s for s, _ in rows]
    assert stems == ["rung_z3", "rung_z2", "reanchor", "ramp_over_r3", "split_over_r3",
                     "ramp_split_over_r3", "ramp_over_re2", "split_over_re2", "ramp_split_over_re2",
                     "reanchor_z3", "ramp_split_over_re3"]
    spelled = dict(rows)
    assert spelled["rung_z3"] == {"noticer": "rung"}
    assert spelled["rung_z2"] == {"noticer": "rung", "notice_z": 2.0}
    assert spelled["reanchor"]["noticer"] == "reanchor" and spelled["reanchor"]["notice_z"] == C.REANCHOR_Z
    assert "ramp" in spelled["ramp_over_r3"] and "split" not in spelled["ramp_over_r3"]
    assert "split" in spelled["split_over_re2"] and "ramp" not in spelled["split_over_re2"]
    both = spelled["ramp_split_over_re3"]
    assert "ramp" in both and "split" in both and "notice_z" not in both["base"]
    # The ramp's parameters are the one choice in every row that has a ramp; the split's are the
    # base's own choice (the re-anchor's for the rows over it, the rung's for the rows over the rung).
    assert spelled["ramp_over_r3"]["ramp"] == spelled["ramp_over_re2"]["ramp"]
    assert spelled["split_over_r3"]["split"]["gap_ns"] == 2_000_000_000
    assert spelled["split_over_re2"]["split"]["gap_ns"] == 3_000_000_000
    assert spelled["ramp_split_over_re3"]["split"] == spelled["split_over_re2"]["split"]


def test_the_sensitivity_arms_are_distinct_from_the_table_and_from_each_other():
    sel = _sel()
    table = {s for s, _ in M.table_rows(sel)}
    sens = M.sensitivity_rows(sel)
    stems = [s for s, _ in sens]
    assert len(stems) == len(set(stems)) and not (set(stems) & table)
    ramp_chosen = C.name_of("r3", ramp=sel["ramp"]["chosen"]["params"])
    assert ramp_chosen not in stems, "the chosen ramp is the table's, not a sensitivity arm"
    # One parameter at a time: each ramp sensitivity arm differs from the chosen in exactly one
    # parameter, apart from the two looser ramps outside the grid.
    chosen = sel["ramp"]["chosen"]["params"]
    one_off = 0
    for stem, nj in sens:
        if "ramp" in nj and "split" not in nj:
            r = nj["ramp"]
            diffs = sum(
                r[k_ns] != (chosen[k] * 1_000_000 if k == "gap_ms" else chosen[k])
                for k, k_ns in (("gap_ms", "gap_ns"), ("max_step", "max_step"), ("max_drop", "max_drop"),
                                ("min_readings", "min_readings"), ("min_rise", "min_rise"))
            )
            one_off += diffs == 1
    assert one_off >= 8


def test_the_budget_and_the_bar_are_the_queues():
    assert C.BACKGROUND_BUDGET == pytest.approx(6.82)
    assert C.LEAK_NOTICED_BAR == pytest.approx(0.660)
    assert C.COMPARATOR == "reanchor"
