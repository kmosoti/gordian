"""L1's scripts (`scripts/l1_*.py`): the arithmetic behind the criterion's slope clause and the
paired slope differences reported beside it, and the arm list the manifests are written from. The
scripts are exploration code outside the package, so the test puts the directories on the path. It
reads no run output: every input is built here."""

import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))

import l1_common as C  # noqa: E402
import l1_stats as L  # noqa: E402


class Fake:
    """The part of `b2_stats.Measures` the slope code reads: rows, per-stream columns and seeds."""

    def __init__(self, hard_n, correct):
        self.names = list(correct)
        self.row = {n: i for i, n in enumerate(self.names)}
        k = len(hard_n)
        self.seeds = np.arange(100, 100 + k)
        self.n = k
        zeros = np.zeros((len(self.names), k))
        self.col = {
            "hard_n": np.tile(np.asarray(hard_n, float), (len(self.names), 1)),
            "hard_correct": np.stack([np.asarray(correct[n], float) for n in self.names]),
            "leak_n": zeros,
            "leak_noticed": zeros,
        }


def make():
    rng = np.random.default_rng(3)
    hard_n = rng.integers(0, 4, size=40)
    a = np.array([rng.binomial(n, 0.9) for n in hard_n])
    b = np.array([rng.binomial(n, 0.7) for n in hard_n])
    return Fake(hard_n, {"a": a, "b": b})


def test_the_curve_slope_is_the_least_squares_slope_of_w1s_curve():
    m = make()
    point, lo, hi = L.curve_slope(m, "a", "anchor", 30, resamples=200)
    c = L.curve(m, "a", "anchor").iloc[:30].dropna(subset=["efficiency"])
    own = np.polyfit(c["stream"], c["efficiency"], 1)[0] * 100
    assert abs(point - own) < 1e-9
    assert lo <= hi


def test_the_slope_difference_is_paired_and_antisymmetric_at_the_point():
    m = make()
    same = L.curve_slope_diff(m, "a", "a", "anchor", 30, resamples=200)
    assert same == (0.0, 0.0, 0.0)
    ab = L.curve_slope_diff(m, "a", "b", "anchor", 30, resamples=200)
    ba = L.curve_slope_diff(m, "b", "a", "anchor", 30, resamples=200)
    assert abs(ab[0] + ba[0]) < 1e-12
    sa = L.curve_slope(m, "a", "anchor", 30, resamples=200)[0]
    sb = L.curve_slope(m, "b", "anchor", 30, resamples=200)[0]
    assert abs(ab[0] - (sa - sb)) < 1e-9
    # same resampled streams for both arms: the interval of the difference is the negated interval
    # of the reverse difference
    assert abs(ab[1] + ba[2]) < 1e-9 and abs(ab[2] + ba[1]) < 1e-9


def test_a_paired_window_difference_of_one_arm_with_itself_is_zero():
    m = make()
    w = L.Window(m, 0, 40)
    assert w.paired("hard_correct", "hard_n", "a", "a", resamples=100) == (0.0, 0.0, 0.0)


def test_the_arms_are_the_criterions_four_the_fifth_control_and_the_labelled_sensitivity_arms():
    arms = C.arms()
    names = [n for n, _ in arms]
    assert names[:5] == ["learned", "learned_off", "frozen", "reanchor", "ramp_split_over_re2"]
    assert len(set(names)) == len(names)
    assert C.arm_name(C.RAMP_SPLIT) == "sel_ramp_split_over_re2_privileged"
    spec = dict(arms)[C.RAMP_SPLIT]
    assert spec["noticer"] == "composed" and spec["base"] == C.REANCHOR
    assert set(spec) == {"noticer", "base", "ramp", "split"}
    # every learning arm has its own state key: arms with one key would share what they learn
    keys = [s["state_key"] for _, s in arms if s["noticer"] == "learned"]
    assert len(set(keys)) == len(keys)


def test_the_learned_arms_start_from_the_public_priors_and_never_read_the_frozen_constants():
    spec = dict(C.arms())["learned"]
    assert spec["learning"] and spec["carry"]
    assert spec["prior_window_ns"] == 0 and spec["prior_ramp_threshold"] == 2.0
    off = dict(C.arms())["learned_off"]
    assert not off["learning"] and not off["carry"]
    # the structure the learned arm is given is the frozen graph; the frozen arm is that graph
    assert spec["structure"] == C.frozen()


def test_the_resampling_weights_keep_the_stream_count_and_the_interval_is_ordered():
    rng = np.random.default_rng(5)
    w = L.counts(rng, 25, 50)
    assert w.shape == (50, 25) and (w >= 0).all() and (w.sum(axis=1) == 25).all()
    lo, hi = L.interval([1.0, 2.0, 3.0, np.nan, 4.0, 5.0])
    assert lo <= hi and 1.0 <= lo and hi <= 5.0
    assert all(np.isnan(v) for v in L.interval([np.nan]))


def test_a_curve_slope_leaves_out_streams_before_the_first_incident_and_is_zero_when_flat():
    inc = np.array([0, 0, 2, 2, 2, 2], float)
    flat = np.array([0, 0, 1, 1, 1, 1], float)  # half correct every time: a flat curve
    s = L._slopes_of(inc, flat, np.ones((1, 6)))[0]
    assert abs(s) < 1e-12
    rising = np.array([0, 0, 0, 1, 2, 2], float)  # nothing right until stream 4
    assert L._slopes_of(inc, rising, np.ones((1, 6)))[0] > 0
    # a window with fewer than two points that have an incident has no slope
    assert np.isnan(L._slopes_of(np.array([0.0, 0.0, 3.0]), np.array([0.0, 0.0, 3.0]), np.ones((1, 3)))[0])
