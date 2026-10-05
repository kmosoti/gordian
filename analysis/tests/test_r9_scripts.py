"""R9's scripts (`experiments/exploration/scripts/r9_*.py`): the parts whose errors would change a
score without anyone noticing. The scripts are exploration code outside the package, so the test puts
their directory on the path. It reads no run output: every input is built here."""

import re
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[2] / "experiments" / "exploration" / "scripts"
sys.path.insert(0, str(SCRIPTS))

import r9_common as C  # noqa: E402
import r9_evaluate as E  # noqa: E402
import r9_reader as R  # noqa: E402
import r9_stats as S  # noqa: E402

# s0, s1 <- s0, s2 <- s1 (a chain), s3 alone, s4 <- s3: the site s1 has upstream s0, dependent s2,
# and s3 and s4 are not connected to it.
SERVICES = [
    {"id": 0, "depends_on": [], "config_hash": 10},
    {"id": 1, "depends_on": [0], "config_hash": 11},
    {"id": 2, "depends_on": [1], "config_hash": 12},
    {"id": 3, "depends_on": [], "config_hash": 13},
    {"id": 4, "depends_on": [3], "config_hash": 14},
]
T0 = 100.0
FREE = 1 << 40


def counter(i, at_s, service, name, value):
    return {"id": i, "at_ns": int(at_s * 1e9),
            "obs": {"Counter": {"service": service, "name": name, "value": value}}}


def message(i, at_s, service, text_id, severity="Low"):
    return {"id": i, "at_ns": int(at_s * 1e9),
            "obs": {"Message": {"service": service, "text_id": text_id, "severity": severity}}}


FOCUS = counter(0, T0, 1, "ErrorRate", 80)


def test_the_reader_names_no_hidden_field():
    src = (SCRIPTS / "r9_reader.py").read_text()
    code = "\n".join(line for line in src.splitlines() if not line.lstrip().startswith("#"))
    # strip docstrings, which describe the rules in words
    code = re.sub(r'""".*?"""', "", code, flags=re.S)
    for word in ("pool_roles", "truth", "decisive", "tier", "label", "pool", "incident", "family"):
        assert not re.search(rf"\b{word}\b", code), word


def test_the_reader_ignores_fields_it_is_not_given():
    ctx = [message(1, T0 + 7.0, 1, FREE | 5), message(2, T0 + 9.0, 1, FREE | 5),
           message(3, T0 + 12.0, 1, FREE | 6)]
    base = R.read(SERVICES, FOCUS, ctx)
    tagged = [dict(r, label="decisive", tier="Hard", role="own:Pulse") for r in ctx]
    again = R.read(SERVICES, dict(FOCUS, truth="x"), tagged)
    assert base[:2] == again[:2]
    assert base[1] == 1  # the site is the first alarm's service


def test_a_plain_burst_is_read_by_the_public_rules():
    # an authentication failure burst at the site: the characteristic message names the kind
    burst = [counter(1, T0 + 0.01, 1, "AuthFailures", 70), message(2, T0 + 0.02, 1, 0x103, "High")]
    kind, site, _ = R.read(SERVICES, FOCUS, burst)
    assert (kind, site) == ("CredentialExpired", 1)
    # no message: the counter names it
    kind, _, _ = R.read(SERVICES, FOCUS, [counter(1, T0 + 0.01, 1, "Restarts", 90)])
    assert kind == "DependencyDown"


def test_free_form_messages_at_the_site_in_phase_two_make_a_hard_reading():
    # no background at all: three free-form messages at the site 6 to 16 s after the first alarm
    ctx = [message(1, T0 + 7.0, 1, FREE | 5), message(2, T0 + 10.0, 1, FREE | 5),
           message(3, T0 + 14.0, 1, FREE | 7)]
    kind, _, trace = R.read(SERVICES, FOCUS, ctx)
    assert kind in R.HARD_CLASSES
    assert len(trace["cited_ff"]) == 3


def test_the_reader_is_deterministic():
    ctx = [message(1, T0 + 7.0, 1, FREE | 5), counter(2, T0 + 0.1, 3, "ErrorRate", 80)]
    assert R.read(SERVICES, FOCUS, ctx) == R.read(SERVICES, FOCUS, list(ctx))


def test_relations_come_from_the_public_graph_alone():
    assert R.relations(SERVICES, 1) == {0: "upstream", 1: "site", 2: "dependent", 3: "unconnected",
                                        4: "unconnected"}


def test_poisson_tail_is_a_tail():
    assert R.poisson_tail_score(0, 1.0) == 0.0
    assert R.poisson_tail_score(1, 1e-6) > 10
    assert R.poisson_tail_score(3, 1.0) > R.poisson_tail_score(2, 1.0) > R.poisson_tail_score(1, 1.0)
    assert R.poisson_tail_score(5, 1e-12) == 20.0  # capped


def rec(seed, inc, tier, pool_size):
    return {"seed": seed, "incident": inc, "tier": tier, "pool_size": pool_size}


def test_selection_takes_one_per_stream_by_seed_mod_k_and_excludes_small_pools(tmp_path):
    rows = [
        rec(10, 0, "Hard", 500), rec(10, 1, "Hard", 100), rec(10, 2, "Hard", 600), rec(10, 3, "Plain", 450),
        rec(11, 0, "Hard", 90), rec(11, 1, "Plain", 900), rec(11, 2, "Plain", 410),
        rec(12, 0, "Hard", 700), rec(12, 1, "Hard", 800), rec(12, 2, "Hard", 900),
    ]
    import json

    p = tmp_path / "d.jsonl"
    p.write_text("\n".join(json.dumps(r) for r in rows) + "\n")
    hard, plain, ex = C.select_from_dump(p, n_plain=2)
    # stream 10: eligible (500, 600), k = 2, seed mod 2 = 0 -> incident 0; stream 11: none eligible;
    # stream 12: eligible 700, 800, 900, k = 3, 12 mod 3 = 0 -> incident 0
    assert [(q["seed"], q["incident"]) for q in hard] == [(10, 0), (12, 0)]
    assert ex["hard_small_pool"] == 2 and ex["streams_without_hard"] == 1 and ex["hard_eligible"] == 5
    # plain: stream 10 has one (450); stream 11 has two (900, 410), 11 mod 2 = 1 -> incident 2
    assert [(q["seed"], q["incident"]) for q in plain] == [(10, 3), (11, 2)]


def test_ablations_remove_only_what_they_say():
    dec = [message(10, 7.0, 1, FREE | 1), message(11, 9.0, 1, FREE | 1)]
    pool = [message(20, 8.0, 2, FREE | 2), counter(21, 0.1, 1, "ErrorRate", 80),
            counter(22, 3.0, 3, "Latency", 70), message(23, 11.0, 1, FREE | 3)]
    for r in dec + pool:
        r["at_ns"] = int(r["at_ns"])
    roles = ["background:FreeForm", "own:Presentation", "other:Pulse", "own:Pulse"]
    q = {"decisive": dec, "pool": pool, "pool_roles": roles}
    ctx = sorted(dec + pool[:3], key=lambda r: r["at_ns"])
    out = E.ablations(q, ctx)
    ids = lambda rs: sorted(r["id"] for r in rs)  # noqa: E731
    assert ids(out["lookalike_ff"]) == [10, 11, 21, 22]  # the background free-form message is gone
    assert ids(out["only_own"]) == [10, 11, 21]  # background and other incidents are gone
    assert ids(out["full_own"]) == [10, 11, 20, 21, 22, 23]  # its own pulse 23 is added


def test_stats_selftest():
    S.selftest()


def test_the_rerun_values_follow_the_readings():
    rv = S.rerun_values(0.046, 0.0501, 0.2)
    assert [(r["run_delta"], r["source"]) for r in rv] == [
        (0.05, "R7 run reused"), (0.05, "R7 run reused"), (0.2, "R7 run reused")]
    assert rv[1]["same_as"] == "lo"
    rv = S.rerun_values(0.0, 0.02, 0.2999)
    assert [r["source"] for r in rv] == ["R6 files (delta 0)", "new run", "new run"]


def test_the_precondition_is_the_plan_s():
    assert S.reader_precondition(0.80, 0.71)["holds"]
    assert not S.reader_precondition(0.79, 0.75)["holds"]
    assert not S.reader_precondition(0.90, 0.70)["holds"]  # the lower bound must be above 0.70


def test_the_misfit_is_the_largest_residual_over_the_five_levels():
    a = [0.9, 0.9, 0.9, 0.9, 0.9]
    res, worst = S.misfit(a, 0.0, 0.0)  # a flat fit has no residual
    assert worst == pytest.approx(0.0)
    a = [0.9, 0.8, 0.9, 0.9, 0.9]
    res, worst = S.misfit(a, 0.0, 0.0)
    assert worst == pytest.approx(0.1) and res[1] == pytest.approx(-0.1)
