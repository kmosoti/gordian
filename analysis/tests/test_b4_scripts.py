"""B4's scripts (`scripts/b4_*.py`): the arithmetic whose errors would change a choice or a row without
anyone noticing: the two tuning rules (the follow-up rule's feasibility, objective and ties; the
selector's quality floor, efficiency and ties), the arm spellings the Rust side reads, and the
statistics over the selection files. The scripts are exploration code outside the package, so the test
puts the directories on the path. It reads no run output: every input is built here."""

import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import b4_common as C  # noqa: E402
import b4_select as SEL  # noqa: E402
import b4_stats as S  # noqa: E402
from gordian_analysis.load import load_stream_run  # noqa: E402
from notice_fixtures import write_notice_files  # noqa: E402
from selection_fixtures import selecting_streams, write_selection_files  # noqa: E402
from stream_fixtures import write_stream_arm  # noqa: E402

FOLLOWUP = {1: {1, 2}, 2: {0, 2}}


# ---- the spellings the Rust side reads -------------------------------------------------------------


def test_the_policies_are_what_the_rust_side_reads():
    assert C.never_policy() == "never_escalate"
    assert C.always_policy() == {"policy": "always_escalate", "delay_ns": 16_000_000_000}
    assert C.thr_policy(0) == {"policy": "public_threshold", "delay_ns": 16_000_000_000}
    assert C.thr_policy(2) == {"policy": "public_threshold", "delay_ns": 16_000_000_000,
                               "persist_ns": 2_000_000_000}
    assert C.chg_policy(5) == {"policy": "public_change", "delay_ns": 16_000_000_000, "k": 5}
    assert C.DELAY_S == 16
    # The arm names carry nothing a path or the run output would mind, and are within the limit.
    for stem, _ in C.rows(C.C3.load_selected(), C.follow_json(C.follow_params(3, 6, 3, 4))):
        for name in ([C.thr_arm(stem, t) for t in C.THR_PERSIST_S] + [C.chg_arm(stem, k) for k in C.CHG_K]
                     + [C.oracle_arm(stem), C.never_arm(stem), C.always_arm(stem)]):
            assert all(c.isalnum() or c in "_-." for c in name), name
            assert len(name) <= 50, name


def test_the_follow_up_spelling_is_what_the_rust_side_reads():
    p = C.follow_params(3, 6, 4, None)
    j = C.follow_json(p)
    assert set(j) == {"readings", "horizon_ns", "min_gain", "max_fall"}
    assert j == {"readings": 3, "horizon_ns": 6_000_000_000, "min_gain": 4, "max_fall": 4_294_967_295}
    assert C.follow_json(C.follow_params(1, 4, 0, 8))["max_fall"] == 8
    assert len(C.follow_grid()) == 6 * 2 * 4 * 4 == 192
    stems = [C.follow_stem_name(q) for q in C.follow_grid()]
    assert len(set(stems)) == len(stems)
    noticer = C.C3.composed("re2", ramp=C.C3.ramp_params(1600, 10, 4, 5, 15))
    with_f = C.with_follow(noticer, j)
    assert with_f["ramp"]["follow"] == j and "follow" not in noticer["ramp"], "a copy, not the original"


def test_the_rows_are_b3s_the_medium_and_the_follow_up_variants_of_the_ramp_rows():
    sel3 = C.C3.load_selected()
    rows = C.rows(sel3, None)
    stems = [s for s, _ in rows]
    assert len(stems) == 12 and stems[-1] == C.MEDIUM_STEM and stems[8] == C.COMPARATOR
    j = C.follow_json(C.follow_params(3, 6, 3, 4))
    full = C.rows(sel3, j)
    assert [s for s, _ in full][12:] == [C.follow_stem(s) for s in C.FOLLOW_BASES]
    by = dict(full)
    for base in C.FOLLOW_BASES:
        a, b = by[base], by[C.follow_stem(base)]
        assert b["ramp"]["follow"] == j
        assert {k: v for k, v in b["ramp"].items() if k != "follow"} == a["ramp"]
        assert b["base"] == a["base"] and b.get("split") == a.get("split")
    # The medium is M2's frozen graph at 100 ms.
    assert by[C.MEDIUM_STEM]["noticer"] == "medium" and by[C.MEDIUM_STEM]["tick_ns"] == 100_000_000


# ---- the follow-up rule's choice -----------------------------------------------------------------------


def follow_frame(rows):
    base = {"background_per_stream": 5.0, "leak_retired": 0, "quality_loss": 0.0, "decoy_retired_before": 0,
            "plain_retired": 0, "hard_retired": 0, "p_readings": 3, "p_min_gain": 3, "p_max_fall": 4,
            "p_horizon_s": 6}
    df = pd.DataFrame([{**base, **r} for r in rows])
    df["collateral"] = df["plain_retired"] + df["hard_retired"]
    df["fall_rank"] = df["p_max_fall"].map(lambda v: 10**9 if v == "x" else v)
    return df


def test_the_follow_up_choice_is_the_most_decoys_retired_within_the_constraints():
    df = follow_frame([
        {"stem": "over_budget", "decoy_retired_before": 50, "background_per_stream": 7.0},
        {"stem": "loses_a_leak", "decoy_retired_before": 40, "leak_retired": 1},
        {"stem": "loses_quality", "decoy_retired_before": 30, "quality_loss": 0.006},
        {"stem": "ok_small", "decoy_retired_before": 5},
        {"stem": "ok_big", "decoy_retired_before": 9},
    ])
    pick, feasible, tied = SEL.choose_follow(df, 0)
    assert (pick["stem"], feasible, tied) == ("ok_big", 2, 1)
    # A tolerance of one leak admits the config that loses one; a loss of quality of exactly 0.005 and a
    # background of exactly the budget are inside.
    pick, feasible, _ = SEL.choose_follow(df, 1)
    assert (pick["stem"], feasible) == ("loses_a_leak", 3)
    df.loc[2, "quality_loss"] = 0.005
    df.loc[0, "background_per_stream"] = C.BACKGROUND_BUDGET
    pick, feasible, _ = SEL.choose_follow(df, 0)
    assert (pick["stem"], feasible) == ("over_budget", 4)


def test_follow_up_ties_go_to_the_least_collateral_then_the_more_patient_rule():
    df = follow_frame([
        {"stem": "a", "decoy_retired_before": 9, "plain_retired": 4},
        {"stem": "b", "decoy_retired_before": 9, "plain_retired": 1, "hard_retired": 1, "p_readings": 2},
        {"stem": "c", "decoy_retired_before": 9, "plain_retired": 2, "p_readings": 6},
        {"stem": "d", "decoy_retired_before": 9, "plain_retired": 2, "p_readings": 4},
    ])
    pick, _, tied = SEL.choose_follow(df, 0)
    # b and c and d tie on 2 collateral notices at best (a has 4): the larger `readings` wins.
    assert (pick["stem"], tied) == ("c", 4)
    df2 = follow_frame([
        {"stem": "x", "decoy_retired_before": 3, "p_max_fall": 4},
        {"stem": "y", "decoy_retired_before": 3, "p_max_fall": "x"},
    ])
    assert SEL.choose_follow(df2, 0)[0]["stem"] == "y", "never withdrawing on a reversal is the largest max_fall"


def test_when_nothing_is_feasible_the_least_leak_loss_is_taken_and_the_rule_still_ranks():
    df = follow_frame([
        {"stem": "a", "decoy_retired_before": 9, "leak_retired": 3},
        {"stem": "b", "decoy_retired_before": 2, "leak_retired": 2},
        {"stem": "c", "decoy_retired_before": 7, "leak_retired": 2},
    ])
    pick, feasible, _ = SEL.choose_follow(df, 0)
    assert (pick["stem"], feasible) == ("c", 0)


# ---- the selector's choice ---------------------------------------------------------------------------------


def grid(rows):
    return pd.DataFrame([{"value": v, "quality": q, "cost_s": c, "calls": n} for v, q, c, n in rows])


def test_the_selector_choice_is_the_best_quality_per_second_above_the_floor():
    # Quality 0.50 at 1.0 s (0.50 per s), 0.46 at 0.5 s (0.92 per s), 0.20 at 0.1 s (2.0 per s, below
    # 0.9 of the best: 0.45), 0.45 at 0.8 s (0.5625 per s).
    g = grid([(1, 0.50, 1.0, 9), (2, 0.46, 0.5, 5), (3, 0.20, 0.1, 1), (4, 0.45, 0.8, 7)])
    pick, tied = SEL.choose_selector(g)
    assert (pick["value"], tied) == (2, 1)
    # The floor is inclusive: 0.45 is exactly 0.9 of 0.5.
    g = grid([(1, 0.50, 1.0, 9), (2, 0.45, 0.2, 5)])
    assert SEL.choose_selector(g)[0]["value"] == 2
    g = grid([(1, 0.50, 1.0, 9), (2, 0.449, 0.2, 5)])
    assert SEL.choose_selector(g)[0]["value"] == 1


def test_selector_ties_go_to_the_fewer_calls_then_the_more_selective_value():
    g = grid([(1, 0.40, 0.4, 6), (2, 0.40, 0.4, 3), (5, 0.40, 0.4, 3)])
    pick, tied = SEL.choose_selector(g)
    assert (pick["value"], tied) == (5, 3)
    g = grid([(1, 0.40, 0.4, 6), (2, 0.40, 0.4, 3)])
    assert SEL.choose_selector(g)[0]["value"] == 2


# ---- the statistics over the selection files --------------------------------------------------------------


@pytest.fixture
def run(tmp_path):
    streams = selecting_streams()
    root = tmp_path / "run"
    for name, fu in (("a", FOLLOWUP), ("b", {})):
        path = write_stream_arm(root / name, streams)
        write_notice_files(path, streams)
        write_selection_files(path, streams, followup=fu)
    return load_stream_run(root)


def test_the_selection_measures_are_pooled_counts_over_streams(run):
    m = S.Measures(run)
    p = m.points()
    a = m.row["a"]
    # Calls by class over the three streams: plain 2 (stream 1), decoy 1 + 2, hard 1, background 1.
    assert p["esc_plain_per_stream"][a] == pytest.approx(2 / 3)
    assert p["esc_decoy_per_stream"][a] == pytest.approx(3 / 3)
    assert p["esc_hard_per_stream"][a] == pytest.approx(1 / 3)
    assert p["esc_background_per_stream"][a] == pytest.approx(1 / 3)
    assert p["esc_leak_per_stream"][a] == 0
    # Cost share of a class: its tokens over all tokens (a call is 100 tokens): decoys 3 of 7 calls.
    assert p["esc_decoy_cost_share"][a] == pytest.approx(3 / 7)
    assert p["esc_plain_cost_share"][a] == pytest.approx(2 / 7)
    assert sum(p[f"esc_{c}_cost_share"][a] for c in S.CLASSES) == pytest.approx(1.0)
    # Notices on decoys: stream 1 has 1, stream 2 has 3 (a2, a3, a4), stream 3 none; decoys are 1, 2 and 0.
    assert p["decoy_notices_per_decoy"][a] == pytest.approx(4 / 3)
    # Decoy notices escalated: a2 of stream 1 and a3 of stream 2 (2 of 4); retired before escalation: in
    # stream 2, a2 (retired, never asked) and a4 is not retired (retire is 4 of 5, the first four).
    assert p["notices_decoy_escalated_share"][a] == pytest.approx(2 / 4)
    # The follow-up counts: the leak notice of stream 2 (a0), a decoy notice of stream 2 (a2) and
    # a decoy (a2) and a plain (a1) of stream 1.
    assert p["followup_retired_leak_per_stream"][a] == pytest.approx(1 / 3)
    assert p["leak_lost_share"][a] == pytest.approx(1 / 2)  # one of the two slow-leak incidents
    assert p["decoy_followup_hit_share"][a] == pytest.approx(1 / 3)
    # No rule in the other arm: nothing retired by it, and the same calls.
    b = m.row["b"]
    assert p["followup_retired_leak_per_stream"][b] == 0
    assert p["leak_lost_share"][b] == 0
    assert p["esc_decoy_per_stream"][b] == p["esc_decoy_per_stream"][a]
    assert p["unattributed_per_stream"][a] == 0
    # Plain accuracy and critical misses are the results': three plain incidents correct? none here.
    assert np.isfinite(p["plain_accuracy"][a])


def test_a_paired_difference_is_the_difference_over_the_same_resamples(run):
    m = S.Measures(run)
    out = m.paired(["followup_retired_leak_per_stream", "esc_decoy_per_stream"],
                   [("followup_retired_leak_per_stream", "a", "b"), ("esc_decoy_per_stream", "a", "b")],
                   seed=1, resamples=200)
    point, lo, hi = out[("followup_retired_leak_per_stream", "a", "b")]
    assert point == pytest.approx(1 / 3) and lo <= point <= hi
    point, lo, hi = out[("esc_decoy_per_stream", "a", "b")]
    assert (point, lo, hi) == (0.0, 0.0, 0.0), "identical calls in both arms: no difference in any resample"
    # The same seed gives the same interval, and a bootstrap's intervals bracket the points.
    again = m.paired(["esc_decoy_per_stream"], [("esc_decoy_per_stream", "a", "b")], seed=1, resamples=200)
    assert again[("esc_decoy_per_stream", "a", "b")] == out[("esc_decoy_per_stream", "a", "b")]
    pts, ci = m.points(["esc_plain_per_stream"]), m.boot(3, 200, measures=["esc_plain_per_stream"])
    assert ci["esc_plain_per_stream"][0][0] <= pts["esc_plain_per_stream"][0] <= ci["esc_plain_per_stream"][1][0]
