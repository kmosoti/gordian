"""B2's scripts (`scripts/b2_*.py`): the arithmetic whose errors would change a number in the table
without anyone noticing. The scripts are exploration code outside the package, so the test puts the
directories on the path. It reads no run output: every input is built here, in the format the harness
writes (`notice_fixtures.py`)."""

import sys
from pathlib import Path

import numpy as np
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))

import b2_common as C  # noqa: E402
import b2_select as SEL  # noqa: E402
import b2_stats as B  # noqa: E402
from gordian_analysis.load import load_stream_run  # noqa: E402
from notice_fixtures import noticing_streams, write_notice_files  # noqa: E402
from stream_fixtures import write_stream_arm  # noqa: E402


def run_of(tmp_path, arms):
    """A run directory with the arms `arms` (name -> streams), each loaded as a harness arm."""
    for name, streams in arms.items():
        path = write_stream_arm(tmp_path / "run" / name, streams)
        write_notice_files(path, streams)
    return load_stream_run(tmp_path / "run")


def test_the_new_measures_are_pooled_ratios_of_the_hand_counts(tmp_path):
    # `noticing_streams()` has 14 notices over three streams: 11 on incidents (6 on plain incidents, 4
    # on hard, 1 on decoys), 5 site-correct, 4 anchor-correct and site-correct (see test_notices.py).
    run = run_of(tmp_path, {"a": noticing_streams()})
    p = {k: float(v[0]) for k, v in B.Measures(run).points().items()}
    assert p["notice_precision"] == pytest.approx(11 / 14)
    assert p["precision_plain"] == pytest.approx(6 / 14)
    assert p["precision_hard"] == pytest.approx(4 / 14)
    assert p["precision_decoy"] == pytest.approx(1 / 14)
    assert p["precision_plain"] + p["precision_hard"] + p["precision_decoy"] == pytest.approx(
        p["notice_precision"]
    )
    assert p["strict_precision"] == pytest.approx(4 / 14)
    assert p["site_correct_notice_share"] == pytest.approx(5 / 14)
    assert p["hard_site_correct_share"] == pytest.approx(2 / 3)
    assert p["hard_anchor_site_correct_share"] == pytest.approx(1 / 3)
    assert p["leak_site_correct_share"] == 0.0
    # B1's measures are unchanged by the additions.
    assert p["hard_noticed_share"] == pytest.approx(2 / 3)
    assert p["hard_anchor_correct_share"] == pytest.approx(1 / 3)
    assert p["notices_on_background_per_stream"] == pytest.approx(1.0)
    assert p["notices_per_incident"] == pytest.approx(11 / 10)


def test_an_interval_contains_its_point_and_a_difference_of_an_arm_with_itself_is_zero(tmp_path):
    streams = noticing_streams()
    run = run_of(tmp_path, {"a": streams, "b": streams})
    m = B.Measures(run)
    pts = m.points()
    ci = m.boot(7, resamples=400, measures=["notice_precision", "hard_anchor_correct_share"])
    for k in ("notice_precision", "hard_anchor_correct_share"):
        lo, hi = ci[k]
        assert lo[0] <= pts[k][0] + 1e-12 and pts[k][0] <= hi[0] + 1e-12, k
        assert (lo == lo[0]).all() and (hi == hi[0]).all(), "two identical arms have one interval"
    point, lo, hi = m.paired_difference("notice_precision", "a", "b", 7, resamples=300)
    assert point == 0.0 and lo == 0.0 and hi == 0.0


def test_the_resamples_are_the_same_whatever_the_measures_asked_for(tmp_path):
    # B1's intervals are reproduced by drawing the same stream counts: the draws do not depend on
    # how many measures are read from them.
    run = run_of(tmp_path, {"a": noticing_streams()})
    m = B.Measures(run)
    one = m.boot(11, resamples=500, measures=["notice_precision"])
    many = m.boot(11, resamples=500)
    assert one["notice_precision"][0][0] == many["notice_precision"][0][0]
    assert one["notice_precision"][1][0] == many["notice_precision"][1][0]


def test_a_measure_is_nan_when_its_denominator_is_zero_in_every_resample(tmp_path):
    from notice_fixtures import not_noticed, with_notices

    streams = noticing_streams()
    quiet = [with_notices(s, [not_noticed() for _ in s["incidents"]]) for s in streams]
    run = run_of(tmp_path, {"quiet": quiet})
    p = B.Measures(run).points()
    assert np.isnan(p["notice_precision"][0]) and np.isnan(p["strict_precision"][0])
    assert p["notices_per_stream"][0] == 0.0


def test_the_choice_rule_takes_the_best_within_the_budget_and_breaks_ties_toward_fewer_moves():
    import pandas as pd

    df = pd.DataFrame(
        [
            # name, anchor-correct, background notices per stream, reading, gap, burst
            ("a", 0.90, 5.0, "site", 100, 2),
            ("b", 0.95, 12.0, "site", 100, 3),  # the best, over the budget
            ("c", 0.93, 8.0, "any", 100, 3),  # the best within the budget, tied with d, e and f
            ("d", 0.93, 8.5, "site", 100, 3),  # `site` before `any` at the same gap and burst
            ("e", 0.93, 7.0, "any", 300, 2),  # the larger gap moves fewer anchors: first of all
            ("f", 0.93, 7.0, "site", 300, 2),  # `site` before `any`, the gap and burst being equal
            ("g", 0.93, 7.0, "site", 300, 3),  # the larger burst moves fewer anchors than f's
        ],
        columns=["noticer", C.TUNE_OBJECTIVE, "notices_on_background_per_stream", "p_isolation",
                 "p_gap_ms", "p_burst"],
    )
    pick, ties, feasible = SEL.choose(C.with_tie_columns(df), C.tie_keys_stage1())
    assert pick["noticer"] == "g" and ties == 5 and feasible == 6
    # Without g, the literal reading wins the tie at equal gap and burst.
    pick, _, _ = SEL.choose(C.with_tie_columns(df[df.noticer != "g"]), C.tie_keys_stage1())
    assert pick["noticer"] == "f"
    # The threshold ladder breaks ties toward the larger threshold.
    ladder = pd.DataFrame(
        [("z1", 0.9, 8.0, 1.0), ("z2", 0.9, 8.0, 2.0), ("z3", 0.9, 8.0, 3.0), ("z0", 0.95, 9.5, 0.5)],
        columns=["noticer", C.TUNE_OBJECTIVE, "notices_on_background_per_stream", "p_z"],
    )
    pick, ties, feasible = SEL.choose(ladder, [("p_z", False)])
    assert pick["noticer"] == "z3" and ties == 3 and feasible == 3
    # The budget is a bound, inclusive: a configuration exactly at it is within it.
    edge = pd.DataFrame(
        [("x", 0.9, C.BACKGROUND_BUDGET, 3.0)],
        columns=["noticer", C.TUNE_OBJECTIVE, "notices_on_background_per_stream", "p_z"],
    )
    assert SEL.choose(edge, [("p_z", False)])[0]["noticer"] == "x"
    over = edge.assign(notices_on_background_per_stream=C.BACKGROUND_BUDGET + 0.01)
    with pytest.raises(SystemExit):
        SEL.choose(over, [("p_z", False)])


def test_the_grids_and_names_are_what_the_manifests_and_the_tables_read():
    grid = C.stage1_grid()
    assert len(grid) == len(C.REANCHOR_ISOLATION) * len(C.REANCHOR_GAP_MS) * len(C.REANCHOR_MIN_BURST)
    names = [n for n, _, _ in grid]
    assert len(set(names)) == len(names)
    # The default threshold is not part of the name or the spelling; another one is.
    assert C.reanchor_name("site", 200, 3) == C.reanchor_name("site", 200, 3, 3.0)
    assert C.reanchor_name("site", 200, 3, 2.0) == "reanchor_site_g200_b3_z2"
    plain = C.spec("reanchor", isolation="site", gap_ms=200, burst=3, z=3.0)
    assert plain == {"noticer": "reanchor", "gap_ns": 200_000_000, "min_burst": 3, "isolation": "site"}
    low = C.spec("reanchor", isolation="any", gap_ms=50, burst=2, z=1.5)
    assert low["notice_z"] == 1.5 and low["isolation"] == "any" and low["gap_ns"] == 50_000_000
    assert C.hold_policy(0) == {"policy": "oracle_selection", "hold_until_asked": True}
    assert C.hold_policy(16) == {"policy": "oracle_selection", "hold_until_asked": True,
                                 "delay_ns": 16_000_000_000}
    assert C.fixed_policy() == {"policy": "oracle_selection", "delay_ns": 16_000_000_000}
    assert C.BACKGROUND_BUDGET == 9.03 and C.TARGET_ANCHOR_CORRECT == 0.944
    # The table is B1's four rows and the new one.
    assert C.TABLE[:2] == ["rung_z3", "rung_z2"] and C.TABLE[-1] == "reanchor"
