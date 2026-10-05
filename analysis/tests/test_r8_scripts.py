"""R8's scripts (`experiments/exploration/scripts/r8_*.py`): the parts whose errors would change a
score without anyone noticing. The scripts are exploration code outside the package, so the test
puts their directory on the path; it imports nothing that needs the runtime or a model."""

import json
import sys
from pathlib import Path

import numpy as np
import pytest

SCRIPTS = Path(__file__).resolve().parents[2] / "experiments" / "exploration" / "scripts"
sys.path.insert(0, str(SCRIPTS))

import r8_analysis  # noqa: E402
import r8_common as C  # noqa: E402
import r8_rules as R  # noqa: E402
import r8_stats as S  # noqa: E402

# s0, s1 <- s0, s2 <- s1, s3 alone, s4 <- s3: the site s1 has upstream s0, dependent s2, and
# s3 and s4 are not connected to it.
SERVICES = [
    {"id": 0, "depends_on": [], "config_hash": 10},
    {"id": 1, "depends_on": [0], "config_hash": 11},
    {"id": 2, "depends_on": [1], "config_hash": 12},
    {"id": 3, "depends_on": [], "config_hash": 13},
    {"id": 4, "depends_on": [3], "config_hash": 14},
]


def counter(i, at_s, service, name, value):
    return {
        "id": i,
        "at_ns": int(at_s * 1e9),
        "obs": {"Counter": {"service": service, "name": name, "value": value}},
    }


def message(i, at_s, service, text_id, severity="Low"):
    return {
        "id": i,
        "at_ns": int(at_s * 1e9),
        "obs": {"Message": {"service": service, "text_id": text_id, "severity": severity}},
    }


FOCUS = counter(0, 100.0, 1, "ErrorRate", 80)
FREE = 1 << 40


def test_selftest_of_the_analysis_script():
    r8_analysis.selftest()


def test_relations_come_from_the_public_graph_alone():
    assert C.connectivity(SERVICES, 1) == ([2], [0], [3, 4])
    assert C.relations(SERVICES, 1) == {
        0: "connected",
        1: "site",
        2: "connected",
        3: "unconnected",
        4: "unconnected",
    }


def test_rendering_of_each_observation_kind():
    rel = C.relations(SERVICES, 1)
    f = FOCUS["at_ns"]
    r = lambda rec: C.render_obs(rec, f, SERVICES, rel)  # noqa: E731
    assert r(counter(1, 103.4, 2, "Latency", 61)) == "+3 s2 latency=61"
    # Whole seconds, rounded: a reading 0.4 s before the focus shows as +0 (never "-0").
    assert r(counter(1, 99.6, 0, "AuthFailures", 7)) == "+0 s0 authfail=7"
    assert r(counter(1, 94.0, 0, "AuthFailures", 7)) == "-6 s0 authfail=7"
    assert r(message(1, 105.0, 1, 0x100, "High")) == "+5 s1(site) msg OutOfResource High"
    assert r(message(1, 106.0, 3, 0x106)) == "+6 s3(unconnected) msg MixedSignals Low"
    line = r(message(1, 107.0, 2, FREE | 5, "Critical"))
    assert line.startswith("+7 s2(connected) msg #") and line.endswith(" Critical")
    snap = {"id": 1, "at_ns": f, "obs": {"Snapshot": {"service": 2, "config_hash": 12}}}
    assert r(snap) == "+0 s2 snapshot unchanged"
    snap["obs"]["Snapshot"]["config_hash"] = 99
    assert r(snap) == "+0 s2 snapshot CHANGED"
    # The alias is a function of the id alone and separates ids.
    assert C.alias(FREE | 5) == C.alias(FREE | 5)
    assert len({C.alias(FREE | k) for k in range(200)}) > 190


def test_the_question_text_holds_the_focus_and_the_context_and_nothing_else():
    ctx = [message(1, 110.0, 1, FREE | 9)]
    text = C.user_text(SERVICES, FOCUS, ctx)
    assert "its site is s1" in text and "+0 s1 errors=80" in text
    assert text.count("msg #") == 1
    # The empty context is explicit.
    assert "(none)" in C.user_text(SERVICES, FOCUS, [])
    # Nothing that names the truth or the tier can be in a question: it is built from the focus,
    # the context and the graph, so a record's other keys cannot reach it.
    for banned in ("Hard", "Plain", "truth", "decisive", "pool", "family", "mode", "other"):
        assert banned not in text


@pytest.mark.parametrize(
    "reply, expect",
    [
        ("ANSWER: Compound s3", ("ok", "Compound", 3)),
        ("Evidence: x\nANSWER: Cascade s1", ("ok", "Cascade", 1)),
        ("ANSWER: NONE", ("ok", "NONE", None)),
        ("ANSWER: Cascade", ("fail", "no site", None)),
        ("ANSWER: Cascade s9", ("fail", "site out of range", None)),
        ("ANSWER: Dragon s1", ("fail", "unknown kind Dragon", None)),
        ("it is Compound at s1", ("fail", "no ANSWER line", None)),
        ("", ("fail", "no ANSWER line", None)),
    ],
)
def test_the_parser(reply, expect):
    assert C.parse_answer(reply, 5) == expect


def test_correct_needs_kind_and_site_and_a_failed_parse_is_wrong():
    truth = {"kind": {"Hard": "Cascade"}, "site": 1}
    assert C.is_correct(("ok", "Cascade", 1), truth)
    assert not C.is_correct(("ok", "Cascade", 2), truth)
    assert not C.is_correct(("ok", "Compound", 1), truth)
    assert not C.is_correct(("fail", "no ANSWER line", None), truth)
    assert not C.is_correct(("ok", "NONE", None), truth)
    plain = {"kind": {"Known": "ConfigDrift"}, "site": 4}
    assert C.is_correct(("ok", "ConfigDrift", 4), plain)


def test_the_rule_reader_follows_the_prompts_rules():
    f = FOCUS
    only_site = [message(1, 108.0, 1, FREE | 1), message(2, 112.0, 1, FREE | 2)]
    assert R.read(SERVICES, f, only_site) == ("Compound", 1)
    far = only_site + [message(3, 110.0, 3, FREE | 3)]
    assert R.read(SERVICES, f, far) == ("Cascade", 1)
    near = only_site + [message(3, 110.0, 2, FREE | 3)]
    assert R.read(SERVICES, f, near) == ("SplitBrain", 1)
    # Outside the window a free-form message does not count.
    early = [message(1, 102.0, 1, FREE | 1)]
    assert R.read(SERVICES, f, early)[0] != "Compound"
    named = [message(1, 101.0, 1, 0x103)]
    assert R.read(SERVICES, f, named) == ("CredentialExpired", 1)
    mixed = [message(1, 101.0, 1, 0x106), message(2, 102.0, 2, 0x106)]
    assert R.read(SERVICES, f, mixed) == ("SplitBrain", 1)


def test_levels_are_nested_sorted_and_the_control_has_no_evidence():
    q = {
        "seed": 3,
        "incident": 7,
        "decisive": [message(900, 110.0, 1, FREE | 1)],
        "pool": [counter(i, 60.0 + i * 0.1, 0, "Latency", 5) for i in range(600)],
    }
    ctx = C.contexts(q)
    sizes = [len(ctx[m]) for m in C.LEVELS]
    assert sizes == [1, 51, 101, 201, 401]
    for lo, hi in zip(C.LEVELS, C.LEVELS[1:]):
        assert {r["id"] for r in ctx[lo]} <= {r["id"] for r in ctx[hi]}
    for m in C.LEVELS:
        times = [(r["at_ns"], r["id"]) for r in ctx[m]]
        assert times == sorted(times)
    assert all(r["id"] != 900 for r in ctx["control"])
    assert len(ctx["control"]) == C.CONTROL_M
    # The same question gives the same draw every time.
    assert [r["id"] for r in C.contexts(q)[200]] == [r["id"] for r in ctx[200]]
    # And the first decisive position is where the evidence sits.
    assert C.first_decisive_position(ctx[400], q["decisive"]) is not None
    assert C.first_decisive_position(ctx["control"], q["decisive"]) is None


def test_selection_excludes_small_pools_and_counts_them():
    def rec(seed, inc, tier, pool):
        return {"seed": seed, "incident": inc, "tier": tier, "pool_size": pool}

    records = [
        rec(1, 0, "Hard", 500),
        rec(1, 1, "Plain", 500),
        rec(1, 2, "Plain", 100),
        rec(2, 0, "Hard", 399),
        rec(2, 1, "Plain", 900),
        rec(3, 0, "Hard", 400),
    ]
    hard, plain, ex = C.select_questions(records, 5, 5)
    assert [(r["seed"], r["incident"]) for r in hard] == [(1, 0), (3, 0)]
    assert ex["hard_small_pool"] == 1
    assert [r["seed"] for r in plain] == [1, 2]
    assert ex["plain_small_pool"] == 1


def test_the_estimator_on_a_hand_computed_curve():
    # A(m) exactly on the model with delta 0.1: the least squares must return it, with no noise.
    p0, a0, delta = 0.1, 0.8, 0.1
    a = np.array([p0 + (a0 - p0) * np.exp(-delta * m / 100) for m in S.LEVELS])
    d, status = S.fit_delta(a, p0)
    assert status == 0 and abs(d - delta) < 6e-4
    # The free-amplitude fit agrees on the same curve.
    fd, amp = S.fit_free_amplitude(a, p0)
    assert abs(fd - delta) < 6e-4 and abs(amp - (a0 - p0)) < 1e-3
    # A model that improves with distractors gets a negative delta.
    b = np.array([0.5, 0.52, 0.55, 0.58, 0.6])
    dn, _ = S.fit_delta(b, 0.1)
    assert dn < 0


def test_outcome_json_round_trips_and_has_the_plans_fields(tmp_path):
    rng = np.random.default_rng(5)
    mat = (rng.random((60, 6)) < np.array([0.7, 0.6, 0.55, 0.45, 0.35, 0.1])).astype(float)
    res = S.analyse(mat, b=300)
    text = json.dumps(res)
    back = json.loads(text)
    for key in ("A", "A_interval", "p0", "delta", "delta_interval", "precondition", "outcome"):
        assert key in back
    assert len(back["A"]) == 5 and len(back["A_interval"]) == 5
    assert back["precondition"]["holds"] in (True, False)
