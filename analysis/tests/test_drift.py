"""The drift and position diagnostics (work item A8)."""

import json
import math

import numpy as np
import pytest

from gordian_analysis.cli import main
from gordian_analysis.drift import (
    EXCEEDS,
    METHOD_PAIRED,
    METHOD_STRATIFIED,
    UNRESOLVED,
    WITHIN,
    PositionEffect,
    check_same_episodes_same_play,
    drift_report,
    load_drift,
    margin_verdict,
    position_effect_paired,
    position_effect_stratified,
)
from gordian_analysis.load import LoadError, load_run

from conftest import write_drift, write_positioned_run


def run_cli(capsys, *argv):
    rc = main(list(argv))
    out = capsys.readouterr()
    return rc, out.out, out.err


# ---- drift.csv ---------------------------------------------------------------------------------


def test_drift_schema_is_the_harness_header():
    # drift.rs: DRIFT_HEADER, the single source of the column names.
    import re
    from pathlib import Path

    from gordian_analysis.drift import DRIFT_COLUMNS

    path = Path(__file__).parents[2] / "crates/gordian-run/src/drift.rs"
    if not path.is_file():
        pytest.skip("harness source not present")
    header = re.search(r'DRIFT_HEADER: &str = "([^"]+)"', path.read_text()).group(1).split(",")
    assert DRIFT_COLUMNS == header


def test_drift_report_hand_values(tmp_path):
    # ns = 100, 110, 90, 100: mean 100; deviations 0, 10, -10, 0; sample variance 200 / 3;
    # sd = sqrt(66.667) = 8.16497; CV = 0.0816497; last / first = 100 / 100 = 1.
    df = load_drift(write_drift(tmp_path, [100, 110, 90, 100]))
    r = drift_report(df)
    assert r.n_blocks == 4 and r.reps_per_block == 2000
    assert r.ns.mean == pytest.approx(100.0)
    assert r.ns.sd == pytest.approx(math.sqrt(200 / 3))
    assert r.ns.cv == pytest.approx(math.sqrt(200 / 3) / 100)
    assert r.ns.ratio_last_first == pytest.approx(1.0)
    assert (r.ns.first, r.ns.last, r.ns.minimum, r.ns.maximum) == (100, 100, 90, 110)


def test_drift_report_two_blocks_and_direction(tmp_path):
    # 200 then 100: mean 150, sd = sqrt(((50)^2 + (50)^2) / 1) = 70.7107, CV = 0.471405,
    # last / first = 0.5 (the machine got faster).
    r = drift_report(load_drift(write_drift(tmp_path, [200, 100], min_ns=[10, 20])))
    assert r.ns.cv == pytest.approx(math.sqrt(5000) / 150)
    assert r.ns.ratio_last_first == pytest.approx(0.5)
    # min_ns is reported separately: 10 then 20 is a ratio of 2.
    assert r.min_ns.ratio_last_first == pytest.approx(2.0)
    assert r.blocks[1] == {"block": 1, "units_done": 50, "ns": 100, "min_ns": 20}


def test_constant_timings_have_zero_cv(tmp_path):
    r = drift_report(load_drift(write_drift(tmp_path, [500, 500, 500])))
    assert r.ns.cv == 0.0 and r.ns.ratio_last_first == 1.0


def test_one_block_has_no_cv(tmp_path):
    with pytest.raises(LoadError, match="at least two"):
        drift_report(load_drift(write_drift(tmp_path, [500])))


def test_load_drift_accepts_the_file_or_the_directory(tmp_path):
    write_drift(tmp_path, [1, 2, 3])
    assert load_drift(tmp_path).equals(load_drift(tmp_path / "drift.csv"))


@pytest.mark.parametrize(
    "text, message",
    [
        ("run_id,block,units_done,reps,ns\nr,0,0,2000,5\n", "columns are"),
        ("run_id,block,units_done,reps,ns,min_ns\n", "no rows"),
        ("run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,x,5\n", "'ns'"),
        ("run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,5.5,5\n", "non-negative integer"),
        ("run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,-5,5\n", "non-negative integer"),
        ("run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,0,5\n", "must be positive"),
        ("run_id,block,units_done,reps,ns,min_ns\nr,1,0,2000,5,5\n", "0, 1, 2"),
        (
            "run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,5,5\nr,0,50,2000,5,5\n",
            "0, 1, 2",
        ),
        (
            "run_id,block,units_done,reps,ns,min_ns\nr,0,50,2000,5,5\nr,1,0,2000,5,5\n",
            "units_done decreases",
        ),
        (
            "run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,5,5\ns,1,50,2000,5,5\n",
            "exactly one run_id",
        ),
    ],
)
def test_load_drift_rejects_malformed_files(tmp_path, text, message):
    (tmp_path / "drift.csv").write_text(text)
    with pytest.raises(LoadError, match=message):
        load_drift(tmp_path)


def test_load_drift_missing_file(tmp_path):
    with pytest.raises(LoadError, match="no drift.csv"):
        load_drift(tmp_path)


def test_blocks_with_different_repetitions_are_refused(tmp_path):
    (tmp_path / "drift.csv").write_text(
        "run_id,block,units_done,reps,ns,min_ns\nr,0,0,2000,5,5\nr,1,50,1000,5,5\n"
    )
    with pytest.raises(LoadError, match="different numbers"):
        drift_report(load_drift(tmp_path))


def test_cli_drift_end_to_end(capsys, tmp_path):
    write_drift(tmp_path, [100, 110, 90, 100], run_id="aa")
    out_json = tmp_path / "out.json"
    rc, out, err = run_cli(capsys, "drift", "--run", str(tmp_path), "--json", str(out_json))
    assert rc == 0 and err == ""
    r = json.loads(out_json.read_text())
    assert r["n_blocks"] == 4 and r["run_id"] == "aa"
    assert r["ns"]["cv"] == pytest.approx(math.sqrt(200 / 3) / 100)
    assert r["ns"]["ratio_last_first"] == pytest.approx(1.0)
    assert "CV=0.0816" in out and "last/first=1.0000" in out
    assert "No tolerance is applied" in out


def test_cli_drift_error_exit(capsys, tmp_path):
    rc, out, err = run_cli(capsys, "drift", "--run", str(tmp_path))
    assert rc == 2 and "no drift.csv" in err and out == ""


# ---- the stratified estimator, one arm ---------------------------------------------------------


def arm_rows(entries):
    """entries: (seed, class, position, cost_ns)."""
    return [
        {"seed": s, "class": c, "arm_position": p, "measured_component_ns": cost}
        for s, c, p, cost in entries
    ]


def test_stratified_statistic_hand_values(tmp_path):
    e = math.e
    # Class A: position 0 costs e^1 and e^3 (log mean 2); later costs e^0 (log 0): d = 2,
    # w = 2 * 1 / 3 = 2/3. Class B: position 0 costs e^2 (2); later costs e^0 and e^2 (mean 1):
    # d = 1, w = 1 * 2 / 3 = 2/3. T = (2/3 * 2 + 2/3 * 1) / (4/3) = 1.5.
    rows = arm_rows(
        [
            (1, "A", 0, e**1), (2, "A", 0, e**3), (3, "A", 1, e**0),
            (4, "B", 0, e**2), (5, "B", 1, e**0), (6, "B", 1, e**2),
        ]
    )  # fmt: skip
    # costs are written as integers by the CSV writer? they are floats; the loader reads floats.
    run = load_run(write_positioned_run(tmp_path / "arm", rows))
    r = position_effect_stratified(run, seed=1, n_resamples=200, n_permutations=200)
    assert r.method == METHOD_STRATIFIED
    assert r.log_ratio == pytest.approx(1.5)
    assert r.ratio == pytest.approx(math.exp(1.5))
    assert (r.n_first, r.n_later, r.n_strata) == (3, 3, 2)


def test_a_class_with_one_kind_of_position_contributes_nothing(tmp_path):
    rows = arm_rows(
        [
            (1, "A", 0, 10.0), (2, "A", 1, 20.0),
            (3, "B", 0, 5.0), (4, "B", 0, 5.0),  # B never plays later: dropped
        ]
    )  # fmt: skip
    run = load_run(write_positioned_run(tmp_path / "arm", rows))
    r = position_effect_stratified(run, seed=1, n_resamples=100, n_permutations=100)
    assert r.n_strata == 1 and (r.n_first, r.n_later) == (1, 1)
    assert r.log_ratio == pytest.approx(math.log(0.5))


def test_stratified_refuses_a_one_arm_run_and_a_run_without_positions(tmp_path):
    only_first = arm_rows([(s, "A", 0, 10.0) for s in range(1, 5)])
    run = load_run(write_positioned_run(tmp_path / "one", only_first))
    with pytest.raises(LoadError, match="no later position"):
        position_effect_stratified(run, seed=1)
    from conftest import write_run

    old = load_run(write_run(tmp_path / "old", [{"seed": s} for s in range(1, 4)]))
    with pytest.raises(LoadError, match="no arm_position"):
        position_effect_stratified(old, seed=1)


def test_nonpositive_cost_has_no_log(tmp_path):
    rows = arm_rows([(1, "A", 0, 0.0), (2, "A", 1, 5.0)])
    run = load_run(write_positioned_run(tmp_path / "arm", rows))
    with pytest.raises(LoadError, match="zero or negative"):
        position_effect_stratified(run, seed=1)


def synthetic_arm(rng, *, theta, classes=6, per_class=30, noise=0.4, spread=1.0):
    """Costs lognormal around a class level, times exp(theta) at position 0.

    Position is drawn at random per episode, independent of everything else, as the harness
    does. Returns the rows `write_positioned_run` takes.
    """
    levels = rng.normal(0.0, spread, size=classes)
    entries = []
    seed = 0
    for c in range(classes):
        for _ in range(per_class):
            seed += 1
            position = int(rng.integers(0, 2))
            log_cost = 10 + levels[c] + noise * rng.normal() + (theta if position == 0 else 0.0)
            entries.append((seed, f"c{c}", position, float(math.exp(log_cost))))
    return arm_rows(entries)


def test_stratified_recovers_a_known_effect_and_the_interval_covers_it(tmp_path):
    rng = np.random.default_rng(20)
    # 180 episodes, noise sd 0.4 per episode: the standard error of the effect is about
    # 0.4 * sqrt(2 / 90) = 0.06, so an effect of 0.30 is five standard errors out.
    theta = 0.30
    run = load_run(write_positioned_run(tmp_path / "arm", synthetic_arm(rng, theta=theta)))
    r = position_effect_stratified(run, seed=3, n_resamples=2000, n_permutations=2000)
    assert abs(r.log_ratio - theta) < 0.12
    assert r.low < theta < r.high
    assert r.low > 0 and r.p_value < 0.01
    assert r.confidence == 0.90


def test_stratified_permutation_p_values_are_calibrated_under_the_null(tmp_path):
    # With no position effect, a test at level 0.05 should reject about 5% of the time.
    # 200 simulated arms; rejections ~ Binomial(200, 0.05), mean 10, sd 3.1. Bound at 24 (4.5 sd).
    rng = np.random.default_rng(7)
    rejections = 0
    for i in range(200):
        rows = synthetic_arm(rng, theta=0.0, classes=3, per_class=12)
        run = load_run(write_positioned_run(tmp_path / f"n{i}", rows))
        r = position_effect_stratified(run, seed=i, n_resamples=50, n_permutations=199)
        rejections += r.p_value <= 0.05
    assert rejections <= 24, rejections
    assert rejections >= 1, "a test that never rejects has no power; the p-value is wrong"


def test_class_heterogeneity_does_not_make_a_null_effect_look_real(tmp_path):
    # Classes differ in cost by e^4 = 55x. A pooled comparison of position 0 against later
    # would be dominated by which classes happened to draw position 0 more often; stratifying
    # keeps the estimate near zero.
    rng = np.random.default_rng(11)
    rows = synthetic_arm(rng, theta=0.0, classes=8, per_class=25, spread=2.0)
    run = load_run(write_positioned_run(tmp_path / "arm", rows))
    r = position_effect_stratified(run, seed=1, n_resamples=500, n_permutations=500)
    assert abs(r.log_ratio) < 0.15
    df = run.results
    pooled = np.log(df.loc[df.arm_position == 0, "measured_total_ns"]).mean() - np.log(
        df.loc[df.arm_position > 0, "measured_total_ns"]
    ).mean()
    # The pooled difference is what the stratification protects against; it is typically larger.
    assert abs(r.log_ratio) <= abs(pooled) + 0.15


def test_stratified_is_reproducible_from_its_seed(tmp_path):
    rng = np.random.default_rng(5)
    run = load_run(write_positioned_run(tmp_path / "arm", synthetic_arm(rng, theta=0.05)))
    a = position_effect_stratified(run, seed=9, n_resamples=300, n_permutations=300)
    b = position_effect_stratified(run, seed=9, n_resamples=300, n_permutations=300)
    c = position_effect_stratified(run, seed=10, n_resamples=300, n_permutations=300)
    assert a == b
    assert (a.low, a.p_value) != (c.low, c.p_value)
    assert a.log_ratio == c.log_ratio  # the estimate does not depend on the seed


# ---- the paired estimator, two copies ----------------------------------------------------------


def copies(tmp_path, entries, **b_override):
    """Two arms that played the same episodes: entries are (seed, pos_a, cost_a, pos_b, cost_b)."""
    a = [
        {"seed": s, "class": "x", "arm_position": pa, "measured_component_ns": ca}
        for s, pa, ca, _, _ in entries
    ]
    b = [
        {"seed": s, "class": "x", "arm_position": pb, "measured_component_ns": cb, **b_override}
        for s, _, _, pb, cb in entries
    ]
    ra = load_run(write_positioned_run(tmp_path / "a", a, run_id="r.a"))
    rb = load_run(write_positioned_run(tmp_path / "b", b, run_id="r.b"))
    return ra, rb


def test_paired_hand_values_and_exact_sign_flip_p(tmp_path):
    e = math.e
    # d_e = log(cost at position 0) - log(cost at the later position):
    #   ep1: a first, a = e^2, b = e^1 -> 1
    #   ep2: b first, b = e^3, a = e^1 -> 2
    #   ep3: a first, equal            -> 0
    #   ep4: b first, b = e^2, a = e^1 -> 1
    # mean d = 1. Sign-flip over 2^4 patterns: |sum(s d)| >= 4 only when every nonzero d has
    # the same sign, which is 2 patterns for the sign on the zero times 2 = 4 of 16, so p = 0.25.
    a, b = copies(
        tmp_path,
        [(1, 0, e**2, 1, e**1), (2, 1, e**1, 0, e**3), (3, 0, e**1, 1, e**1), (4, 1, e**1, 0, e**2)],
    )
    r = position_effect_paired(a, b, seed=1, n_resamples=2000, n_permutations=20000)
    assert r.method == METHOD_PAIRED
    assert r.log_ratio == pytest.approx(1.0)
    assert r.ratio == pytest.approx(math.e)
    assert (r.n_first, r.n_later) == (4, 4)
    assert r.p_value == pytest.approx(0.25, abs=0.02)


def test_paired_is_symmetric_in_the_two_arms(tmp_path):
    e = math.e
    a, b = copies(
        tmp_path, [(1, 0, e**2, 1, e**1), (2, 1, e**1, 0, e**3), (3, 0, e**5, 1, e**4)]
    )
    ab = position_effect_paired(a, b, seed=1, n_resamples=100, n_permutations=100)
    ba = position_effect_paired(b, a, seed=1, n_resamples=100, n_permutations=100)
    assert ab.log_ratio == pytest.approx(ba.log_ratio)


def test_paired_cancels_episode_difficulty_that_the_stratified_estimate_cannot(tmp_path):
    # Episodes differ in cost by orders of magnitude within one class. The copies' timings of
    # one episode share that difficulty, so the paired estimate sees only the position effect
    # (here exactly log 1.05) while the one-arm estimate is dominated by which episodes happened
    # to be first.
    rng = np.random.default_rng(3)
    entries = []
    for s in range(1, 81):
        base = 10 + 2.0 * rng.normal()
        a_first = bool(rng.integers(0, 2))
        ca = math.exp(base + (math.log(1.05) if a_first else 0.0) + 0.01 * rng.normal())
        cb = math.exp(base + (0.0 if a_first else math.log(1.05)) + 0.01 * rng.normal())
        entries.append((s, 0 if a_first else 1, ca, 1 if a_first else 0, cb))
    a, b = copies(tmp_path, entries)
    paired = position_effect_paired(a, b, seed=1, n_resamples=2000, n_permutations=2000)
    assert paired.log_ratio == pytest.approx(math.log(1.05), abs=0.005)
    assert paired.high - paired.low < 0.01
    one_arm = position_effect_stratified(a, seed=1, n_resamples=500, n_permutations=500)
    assert (one_arm.high - one_arm.low) > 10 * (paired.high - paired.low)


def test_paired_drops_episodes_where_neither_copy_is_first(tmp_path):
    e = math.e
    # Three arms in the run: the two copies at positions (0,1), (1,0), (2,1), (1,2).
    a, b = copies(
        tmp_path,
        [
            (1, 0, e**2, 1, e**1), (2, 1, e**1, 0, e**3),
            (3, 2, e**9, 1, e**1), (4, 1, e**1, 2, e**9),
        ],
    )  # fmt: skip
    r = position_effect_paired(a, b, seed=1, n_resamples=100, n_permutations=100)
    assert r.n_first == 2
    assert r.log_ratio == pytest.approx(1.5)  # (1 + 2) / 2; the e^9 episodes are not used


def test_paired_refuses_arms_that_did_not_play_identically(tmp_path):
    e = math.e
    entries = [(1, 0, e**2, 1, e**1), (2, 1, e**1, 0, e**3)]
    a, b = copies(tmp_path, entries, bill_compute=12345)
    with pytest.raises(LoadError, match="did not play every episode identically.*bill_compute"):
        position_effect_paired(a, b, seed=1)
    with pytest.raises(LoadError, match="bill_compute"):
        check_same_episodes_same_play(a, b)


def test_paired_refuses_different_episode_sets(tmp_path):
    a = load_run(
        write_positioned_run(
            tmp_path / "a", arm_rows([(1, "x", 0, 5.0), (2, "x", 1, 5.0)]), run_id="r.a"
        )
    )
    b = load_run(
        write_positioned_run(
            tmp_path / "b", arm_rows([(1, "x", 1, 5.0), (3, "x", 0, 5.0)]), run_id="r.b"
        )
    )
    with pytest.raises(LoadError, match="same .seed, class. episodes"):
        position_effect_paired(a, b, seed=1)


def test_paired_refuses_two_arms_at_one_position(tmp_path):
    e = math.e
    a, b = copies(tmp_path, [(1, 0, e, 0, e), (2, 1, e, 1, e)])
    with pytest.raises(LoadError, match="same position"):
        position_effect_paired(a, b, seed=1)


def test_paired_sign_flip_p_is_calibrated_under_the_null(tmp_path):
    # Copies with no position effect: 200 simulated runs of 30 episodes, ~Binomial(200, 0.05)
    # rejections at 0.05; bound at 24.
    rng = np.random.default_rng(13)
    rejections = 0
    for i in range(200):
        entries = []
        for s in range(1, 31):
            base = 10 + rng.normal()
            a_first = int(rng.integers(0, 2))
            entries.append(
                (s, 1 - a_first, math.exp(base + 0.2 * rng.normal()),
                 a_first, math.exp(base + 0.2 * rng.normal()))
            )  # fmt: skip
        a, b = copies(tmp_path / f"s{i}", entries)
        r = position_effect_paired(a, b, seed=i, n_resamples=50, n_permutations=199)
        rejections += r.p_value <= 0.05
    assert 1 <= rejections <= 24, rejections


# ---- the margin --------------------------------------------------------------------------------


def effect(low, high):
    mid = (low + high) / 2
    return PositionEffect(
        method=METHOD_PAIRED, metric="measured_total_ns", n_first=10, n_later=10, n_strata=1,
        log_ratio=mid, ratio=math.exp(mid), low=low, high=high, confidence=0.9, p_value=0.5,
        n_resamples=1, n_permutations=1, seed=1,
    )  # fmt: skip


def test_margin_verdict_categories():
    L = math.log1p(0.05)
    assert margin_verdict(effect(-0.5 * L, 0.5 * L), 0.05) == WITHIN
    assert margin_verdict(effect(1.01 * L, 2 * L), 0.05) == EXCEEDS  # wholly above +L
    assert margin_verdict(effect(-2 * L, -1.01 * L), 0.05) == EXCEEDS  # wholly below -L
    # Reaches past the margin without lying wholly beyond it: neither established nor excluded.
    assert margin_verdict(effect(0.5 * L, 1.5 * L), 0.05) == UNRESOLVED
    assert margin_verdict(effect(-1.5 * L, 0.5 * L), 0.05) == UNRESOLVED
    # An interval that straddles zero and is wide is unresolved, never "within".
    assert margin_verdict(effect(-3 * L, 3 * L), 0.05) == UNRESOLVED


def test_margin_edges_are_strict():
    L = math.log1p(0.05)
    assert margin_verdict(effect(L, 2 * L), 0.05) == UNRESOLVED  # low == L is not beyond it
    assert margin_verdict(effect(-L, L), 0.05) == UNRESOLVED  # touching is not inside


@pytest.mark.parametrize("bad", [0.0, -0.1, float("nan"), float("inf")])
def test_margin_must_be_a_positive_relative_cost(bad):
    with pytest.raises(ValueError, match="positive relative"):
        margin_verdict(effect(0.0, 0.1), bad)


# ---- the CLI -----------------------------------------------------------------------------------


def test_cli_position_stratified_and_paired_end_to_end(capsys, tmp_path):
    rng = np.random.default_rng(21)
    rows = synthetic_arm(rng, theta=0.1)
    write_positioned_run(tmp_path / "arm", rows, run_id="x.arm")
    out_json = tmp_path / "out.json"
    rc, out, err = run_cli(
        capsys, "position", "--arm", str(tmp_path / "arm"), "--seed", "4", "--resamples", "300",
        "--permutations", "300", "--margin", "0.02", "--json", str(out_json),
    )  # fmt: skip
    assert rc == 0 and err == ""
    r = json.loads(out_json.read_text())
    assert r["effect"]["method"] == METHOD_STRATIFIED
    assert r["effect"]["seed"] == 4 and r["effect"]["n_resamples"] == 300
    assert r["verdict"] in {EXCEEDS, WITHIN, UNRESOLVED}
    assert "stratified permutation" in out and "VERDICT" in out
    assert r["arm"]["run_id"] == "x.arm"

    # Paired: two copies.
    e = math.e
    entries = [(1, 0, e**2, 1, e**1), (2, 1, e**1, 0, e**3), (3, 0, e**1, 1, e**1), (4, 1, e**1, 0, e**2)]
    a = [{"seed": s, "class": "x", "arm_position": pa, "measured_component_ns": ca} for s, pa, ca, _, _ in entries]
    b = [{"seed": s, "class": "x", "arm_position": pb, "measured_component_ns": cb} for s, _, _, pb, cb in entries]
    write_positioned_run(tmp_path / "a", a, run_id="x.a")
    write_positioned_run(tmp_path / "b", b, run_id="x.b")
    rc, out, err = run_cli(
        capsys, "position", "--arm", str(tmp_path / "a"), "--paired-with", str(tmp_path / "b"),
        "--seed", "4", "--resamples", "300", "--permutations", "300",
    )  # fmt: skip
    assert rc == 0 and err == ""
    assert "Method: paired copies" in out
    assert "No margin given" in out and "VERDICT" not in out
    assert "ratio = 2.71828" in out


def test_cli_position_errors(capsys, tmp_path):
    write_positioned_run(tmp_path / "one", arm_rows([(s, "A", 0, 9.0) for s in range(1, 4)]))
    rc, out, err = run_cli(capsys, "position", "--arm", str(tmp_path / "one"), "--seed", "1")
    assert rc == 2 and "no later position" in err and out == ""
    rng = np.random.default_rng(2)
    write_positioned_run(tmp_path / "two", synthetic_arm(rng, theta=0.0, classes=2, per_class=10))
    rc, out, err = run_cli(
        capsys, "position", "--arm", str(tmp_path / "two"), "--seed", "1", "--resamples", "50",
        "--permutations", "50", "--margin", "-1",
    )  # fmt: skip
    assert rc == 2 and "positive relative" in err and out == ""
    rc, out, err = run_cli(capsys, "position", "--arm", str(tmp_path / "missing"), "--seed", "1")
    assert rc == 2 and "no results.csv" in err
