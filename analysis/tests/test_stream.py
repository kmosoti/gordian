"""The stream loader, its schema guard, pairing on seed, the pooled ratios and `stream-summary`
(work item R3b). The episode loader's own tests are untouched; one test here checks that it still
refuses a stream file."""

import json
import re
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import load as load_module
from gordian_analysis import stream as stream_module
from gordian_analysis.cli import main
from gordian_analysis.load import (
    LoadError,
    load_run,
    load_stream_arm,
    load_stream_run,
    pair_arms,
    pair_streams,
)
from gordian_analysis.stream import (
    arm_totals,
    correct_per_cost,
    correct_rate,
    critical_miss_rate,
    escalation_precision,
    escalation_recall,
    family_table,
    pooled_ratio,
    summarize_run,
)

from stream_fixtures import incident, mixed_streams, stream, write_stream_arm, write_stream_run

ROOT = Path(__file__).parents[2]
STREAM_SRC = ROOT / "crates/gordian-run/src/stream"


def edit(path: Path, name: str, fn) -> None:
    """Rewrite `path/name` as text through `fn` (header and rows as lists of fields)."""
    lines = [ln.split(",") for ln in (path / name).read_text().splitlines()]
    lines = fn(lines)
    (path / name).write_text("\n".join(",".join(x) for x in lines) + "\n")


# ---- The schema guard: the loader's columns are the harness's header constants -------------


def _constant(src: str, name: str) -> list[str]:
    # `pub const NAME: &str = "a,b,c";` possibly with the string on the next line.
    m = re.search(rf'pub const {name}: &str =\s*"([^"]+)"', src)
    assert m, f"{name} not found in results.rs"
    return m.group(1).split(",")


@pytest.fixture(scope="module")
def results_rs() -> str:
    path = STREAM_SRC / "results.rs"
    if not path.is_file():
        pytest.skip("harness source not present")
    return path.read_text()


def test_stream_schema_is_the_harness_header(results_rs):
    assert load_module.STREAM_RESULTS_COLUMNS == _constant(results_rs, "RESULTS_HEADER")
    assert load_module.STREAM_INCIDENTS_COLUMNS == _constant(results_rs, "INCIDENTS_HEADER")
    assert load_module.STREAM_MEASURED_COLUMNS == _constant(results_rs, "MEASURED_HEADER")


def test_every_results_column_is_classified_and_the_documented_ones_are_there():
    cols = set(load_module.STREAM_RESULTS_COLUMNS)
    assert set(load_module.STREAM_STRING_COLUMNS) <= cols
    assert set(load_module.STREAM_COUNT_COLUMNS) | set(load_module.STREAM_STRING_COLUMNS) == cols
    # The evaluator's totals, as columns (the deliverable's list).
    for name in [
        "incidents_plain", "incidents_hard", "incidents_decoy", "correct_plain", "correct_hard",
        "missed_plain", "missed_hard", "critical_missed_plain", "critical_missed_hard",
        "wrong_declarations", "decoys_dismissed", "decoys_alarmed", "decoys_silent",
        "false_alarms", "false_alarms_on_background", "escalations_needed",
        "escalations_unneeded", "escalations_background", "hard_incidents_escalated",
        "other_incidents_escalated", "calls_informed", "calls_correct", "reasoner_calls",
        "reasoner_refs", "reasoner_tokens", "reasoner_modelled_ns",
        "arm_role", "total_cost_ns", "reasoner_cost_ns", "substrate_ns",
    ]:  # fmt: skip
        assert name in cols, name


def test_roles_tiers_and_families_are_the_harness_names(results_rs):
    arms = (STREAM_SRC / "arms/mod.rs").read_text()
    roles = re.findall(r'ArmRole::\w+ => "(\w+)"', arms[arms.index("pub fn as_str(self)"):])
    assert set(roles) == set(load_module.STREAM_ARM_ROLES) and len(roles) == 3
    tier_fn = results_rs[results_rs.index("fn tier_name"):]
    tiers = re.findall(r'Tier::\w+ => "(\w+)"', tier_fn[: tier_fn.index("\n}\n")])
    assert tiers == list(load_module.STREAM_TIERS)
    score = (STREAM_SRC / "score.rs").read_text()
    fam_fn = score[score.index("pub fn family_name"):]
    families = re.findall(r'HardKind::\w+ => "(\w+)"', fam_fn[: fam_fn.index("\n}\n")])
    assert families == list(load_module.STREAM_FAMILIES)


def test_the_incident_fields_are_the_evaluators(results_rs):
    # IncidentVerdict (crates/gordian-stream-eval/src/verdict.rs): every field is a column, apart
    # from `id` (written as `incident`), `first_correct_at` and `time_to_first_correct_ns`
    # (written in nanoseconds), and `tier` and `critical`, which are columns too.
    path = ROOT / "crates/gordian-stream-eval/src/verdict.rs"
    if not path.is_file():
        pytest.skip("evaluator source not present")
    src = path.read_text()
    body = src[src.index("pub struct IncidentVerdict"):]
    body = body[: body.index("\n}\n")]
    fields = re.findall(r"pub (\w+):", body)
    rename = {"id": "incident", "first_correct_at": "first_correct_at_ns"}
    for f in fields:
        assert rename.get(f, f) in load_module.STREAM_INCIDENTS_COLUMNS, f
    extra = set(load_module.STREAM_INCIDENTS_COLUMNS) - {rename.get(f, f) for f in fields}
    assert extra == {"run_id", "arm_role", "seed", "family"}


def test_the_stream_totals_are_the_evaluators(results_rs):
    path = ROOT / "crates/gordian-stream-eval/src/verdict.rs"
    if not path.is_file():
        pytest.skip("evaluator source not present")
    src = path.read_text()
    # The loader's pooled definitions are the evaluator's: precision and recall as documented.
    prec = src[src.index("pub fn escalation_precision"):]
    prec = prec[: prec.index("\n    }\n")]
    assert "needed" in prec and "unneeded" in prec and "background" in prec
    rec = src[src.index("pub fn escalation_recall"):]
    rec = rec[: rec.index("\n    }\n")]
    assert "hard_incidents_escalated" in rec and "incidents.hard" in rec


# ---- Loading -------------------------------------------------------------------------------


def test_loads_an_arm_with_exact_integers_and_nullable_instants(tmp_path):
    p = write_stream_arm(tmp_path / "a", mixed_streams())
    arm = load_stream_arm(p)
    assert arm.role == "comparison" and arm.run_id == "r.arm" and arm.name == "a"
    assert arm.results["seed"].tolist() == [1, 2, 3]
    assert arm.results["incidents_hard"].tolist() == [0, 1, 3]
    assert arm.results["arm_position"].tolist() == [0, 0, 0]
    assert str(arm.results["total_cost_ns"].dtype) == "int64"
    inc = arm.incidents
    assert len(inc) == 10 and inc["incident"].tolist()[:3] == [0, 1, 2]
    assert inc["critical"].dtype == bool and inc["correct_by_deadline"].dtype == bool
    # The first correct declaration is empty exactly when there is none.
    assert inc["first_correct_at_ns"].isna().tolist() == (inc["correct_declarations"] == 0).tolist()
    assert inc.loc[inc["tier"] == "hard", "family"].tolist() == [
        "compound", "slow_leak", "cascade", "split_brain",
    ]  # fmt: skip
    assert (inc.loc[inc["tier"] != "hard", "family"] == "").all()


def test_counts_are_read_exactly_not_through_floats(tmp_path):
    big = 9_007_199_254_740_993  # 2**53 + 1: not a float64
    p = write_stream_arm(
        tmp_path / "a", [stream(1, [incident("plain")], substrate_ns=big, total_cost_ns=big)]
    )
    assert load_stream_arm(p).results["substrate_ns"].iloc[0] == big


def test_the_episode_loader_is_unchanged_and_refuses_a_stream_arm(tmp_path):
    p = write_stream_arm(tmp_path / "a", mixed_streams())
    with pytest.raises(LoadError):
        load_run(p)


def test_a_stream_file_must_have_exactly_the_harness_columns_in_order(tmp_path):
    for name in ("results.csv", "incidents.csv", "measured.csv"):
        # an unknown column
        p = write_stream_arm(tmp_path / f"unk-{name}", mixed_streams())
        edit(p, name, lambda t: [t[0] + ["extra"], *[r + ["1"] for r in t[1:]]])
        with pytest.raises(LoadError, match=r"unknown \['extra'\]"):
            load_stream_arm(p)
        # a missing one
        p = write_stream_arm(tmp_path / f"miss-{name}", mixed_streams())
        edit(p, name, lambda t: [t[0][:-1], *[r[:-1] for r in t[1:]]])
        with pytest.raises(LoadError, match="missing"):
            load_stream_arm(p)
        # the same columns in another order
        p = write_stream_arm(tmp_path / f"order-{name}", mixed_streams())
        edit(p, name, lambda t: [[t[0][1], t[0][0], *t[0][2:]], *[[r[1], r[0], *r[2:]] for r in t[1:]]])
        with pytest.raises(LoadError, match="out of order"):
            load_stream_arm(p)


def test_every_documented_results_column_is_required(tmp_path):
    for col in load_module.STREAM_RESULTS_COLUMNS:
        p = write_stream_arm(tmp_path / col, mixed_streams())

        def drop(t, col=col):
            i = t[0].index(col)
            return [[x for j, x in enumerate(r) if j != i] for r in t]

        edit(p, "results.csv", drop)
        with pytest.raises(LoadError, match=f"missing.*{col}"):
            load_stream_arm(p)


@pytest.mark.parametrize("bad", ["-1", "1.5", "", "abc", "1e3", " ", "NaN"])
def test_a_count_that_is_not_a_non_negative_integer_is_refused(tmp_path, bad):
    p = write_stream_arm(tmp_path / "a", mixed_streams())

    def put(t):
        i = t[0].index("reasoner_tokens")
        t[1][i] = bad
        return t

    edit(p, "results.csv", put)
    with pytest.raises(LoadError, match="reasoner_tokens"):
        load_stream_arm(p)


def test_malformed_rows_are_refused_loudly(tmp_path):
    def with_edit(name, file, fn, match):
        p = write_stream_arm(tmp_path / name, mixed_streams())
        edit(p, file, fn)
        with pytest.raises(LoadError, match=match):
            load_stream_arm(p)

    def set_cell(col, value, row=1):
        def go(t):
            t[row][t[0].index(col)] = value
            return t

        return go

    with_edit("role", "results.csv", set_cell("arm_role", "champion"), "arm_role")
    with_edit("irole", "incidents.csv", set_cell("arm_role", "champion"), "arm_role")
    with_edit("tier", "incidents.csv", set_cell("tier", "medium"), "tier")
    # a family belongs to a hard incident and only to one
    with_edit("fam-plain", "incidents.csv", set_cell("family", "compound"), "family")
    with_edit("fam-hard", "incidents.csv", set_cell("family", "", row=5), "family")
    with_edit("fam-name", "incidents.csv", set_cell("family", "leak", row=5), "family")
    with_edit("bool", "incidents.csv", set_cell("missed", "maybe"), "missed")
    # a first correct declaration exists exactly when there is a correct declaration
    with_edit("first-empty", "incidents.csv", set_cell("first_correct_at_ns", ""), "first_correct")
    with_edit("first-extra", "incidents.csv", set_cell("first_correct_at_ns", "5", row=2), "first_correct")
    with_edit("stop", "results.csv", set_cell("stop_reason", ""), "stop_reason")
    # identities the harness guarantees
    with_edit("calls", "results.csv", set_cell("reasoner_calls", "99"), "reasoner_calls")
    with_edit("cost", "results.csv", set_cell("total_cost_ns", "1"), "total_cost_ns")

    p = write_stream_arm(tmp_path / "dup", mixed_streams())
    edit(p, "results.csv", lambda t: [*t, t[1]])
    with pytest.raises(LoadError, match="duplicate seeds"):
        load_stream_arm(p)
    p = write_stream_arm(tmp_path / "dupinc", mixed_streams())
    edit(p, "incidents.csv", lambda t: [*t, t[1]])
    with pytest.raises(LoadError, match="duplicate"):
        load_stream_arm(p)
    p = write_stream_arm(tmp_path / "two-ids", mixed_streams())
    edit(p, "results.csv", lambda t: [t[0], t[1], [t[2][0] + "x", *t[2][1:]], *t[3:]])
    with pytest.raises(LoadError, match="run_id"):
        load_stream_arm(p)


def test_the_two_files_must_describe_the_same_streams(tmp_path):
    p = write_stream_arm(tmp_path / "a", mixed_streams())
    # results.csv counts a hard incident incidents.csv does not have
    edit(p, "incidents.csv", lambda t: [r for r in t if not (r[2] == "3" and r[3] == "1")])
    with pytest.raises(LoadError, match="seed 3"):
        load_stream_arm(p)
    p = write_stream_arm(tmp_path / "b", mixed_streams())
    edit(p, "incidents.csv", lambda t: [*t, [*t[1][:2], "9", *t[1][3:]]])
    with pytest.raises(LoadError, match="seeds"):
        load_stream_arm(p)
    p = write_stream_arm(tmp_path / "c", mixed_streams())
    (p / "incidents.csv").unlink()
    with pytest.raises(LoadError, match="no incidents.csv"):
        load_stream_arm(p)
    p = write_stream_arm(tmp_path / "d", mixed_streams())
    (p / "results.csv").unlink()
    with pytest.raises(LoadError, match="no results.csv"):
        load_stream_arm(p)


def test_measured_is_optional_and_joins_on_seed(tmp_path):
    p = write_stream_arm(tmp_path / "a", mixed_streams(), measured=False)
    assert "arm_position" not in load_stream_arm(p).results.columns
    p = write_stream_arm(tmp_path / "b", mixed_streams())
    edit(p, "measured.csv", lambda t: t[:-1])
    with pytest.raises(LoadError, match="same seeds"):
        load_stream_arm(p)


def test_a_run_is_its_arm_directories_and_the_arms_must_have_played_the_same_seeds(tmp_path):
    arms = {"never": mixed_streams(), "oracle_privileged": mixed_streams()}
    p = write_stream_run(
        tmp_path / "run", arms, roles={"oracle_privileged": "privileged"}
    )
    run = load_stream_run(p)
    assert list(run.arms) == ["never", "oracle_privileged"]
    assert run.arms["oracle_privileged"].role == "privileged"
    assert run.manifest["run_id"] == "r" and run.usage is None
    # a single arm directory is a run of one arm
    assert list(load_stream_run(p / "never").arms) == ["never"]
    arms["never"] = mixed_streams()[:2]
    p = write_stream_run(tmp_path / "other", arms)
    with pytest.raises(LoadError, match="other seeds"):
        load_stream_run(p)
    with pytest.raises(LoadError, match="no arm directory"):
        load_stream_run(tmp_path)
    with pytest.raises(LoadError, match="not a directory"):
        load_stream_run(tmp_path / "missing")


# ---- Pairing on stream seed ----------------------------------------------------------------


def test_pairing_aligns_arms_on_seed_and_checks_the_incidents_agree(tmp_path):
    a = mixed_streams()
    b = [dict(s) for s in reversed(mixed_streams())]  # the same streams, written in another order
    run = load_stream_run(write_stream_run(tmp_path / "run", {"a": a, "b": b}))
    paired = pair_arms(run, "a", "b")
    assert len(paired) == 3
    assert paired.results_a["seed"].tolist() == paired.results_b["seed"].tolist() == [1, 2, 3]
    key = ["seed", "incident"]
    assert paired.incidents_a[key].equals(paired.incidents_b[key])
    with pytest.raises(LoadError, match="no arm 'zzz'"):
        pair_arms(run, "a", "zzz")


def test_pairing_refuses_unmatched_seeds_and_arms_that_did_not_play_the_same_streams(tmp_path):
    full = mixed_streams()
    run = load_stream_run(write_stream_run(tmp_path / "r1", {"a": full, "b": full}))
    a, b = run.arms["a"], run.arms["b"]
    short = write_stream_arm(tmp_path / "short", full[:2])
    with pytest.raises(LoadError, match="unmatched streams"):
        pair_streams(a, load_stream_arm(short))
    # the same seeds and the same number of incidents, but another tier on one: a different world
    changed = [dict(s) for s in full]
    changed[0] = stream(1, [incident("plain"), incident("plain"), incident("plain")])
    other = load_stream_arm(write_stream_arm(tmp_path / "other", changed))
    with pytest.raises(LoadError, match="tier"):
        pair_streams(a, other)
    # another family on a hard incident
    changed = [dict(s) for s in full]
    changed[1] = stream(
        2, [full[1]["incidents"][0], incident("hard", "cascade", critical=True), full[1]["incidents"][2]]
    )
    other = load_stream_arm(write_stream_arm(tmp_path / "fam", changed))
    with pytest.raises(LoadError, match="family"):
        pair_streams(a, other)
    assert len(pair_streams(a, b)) == 3


# ---- Pooled ratios -------------------------------------------------------------------------


def test_pooled_ratio_is_a_ratio_of_sums_and_nan_when_the_denominator_is_zero():
    assert pooled_ratio([1, 1, 0], [1, 4, 5]) == pytest.approx(2 / 10)
    assert np.isnan(pooled_ratio([0, 0], [0, 0]))
    assert pooled_ratio([], []) != pooled_ratio([], [])  # NaN


def test_a_pooled_ratio_is_not_the_mean_of_per_stream_ratios():
    # Stream A: one hard incident, escalated (recall 1). Stream B: five hard incidents, none
    # escalated (recall 0). The mean of the two stream recalls is 0.5; the incident-level pooled
    # recall is 1 of 6, and a stream with no hard incident has no recall to average at all.
    df = pd.DataFrame(
        {
            "incidents_hard": [1, 5, 0],
            "hard_incidents_escalated": [1, 0, 0],
            "escalations_needed": [1, 0, 0],
            "escalations_unneeded": [0, 0, 0],
            "escalations_background": [0, 0, 0],
        }
    )
    per_stream = (df["hard_incidents_escalated"] / df["incidents_hard"]).tolist()
    assert np.isnan(per_stream[2]) and np.nanmean(per_stream) == pytest.approx(0.5)
    assert escalation_recall(df) == pytest.approx(1 / 6)
    # Precision is call-level: ten calls about one hard incident are ten needed calls, and a
    # stream with no calls does not count as a precision of anything.
    df = pd.DataFrame(
        {
            "escalations_needed": [10, 0, 0],
            "escalations_unneeded": [0, 30, 0],
            "escalations_background": [0, 0, 0],
            "incidents_hard": [1, 1, 1],
            "hard_incidents_escalated": [1, 0, 0],
        }
    )
    assert escalation_precision(df) == pytest.approx(10 / 40)
    assert escalation_recall(df) == pytest.approx(1 / 3)  # ten calls on one incident: still one


def test_precision_recall_and_rates_on_the_fixture_match_a_hand_count(tmp_path):
    arm = load_stream_arm(write_stream_arm(tmp_path / "a", mixed_streams()))
    r = arm.results
    # calls: stream 2: 1 (plain) + 2 (hard) + 1 (decoy) + 1 background = 5; stream 3: 1 (hard)
    assert r["reasoner_calls"].tolist() == [0, 5, 1]
    assert escalation_precision(r) == pytest.approx(3 / 6)  # needed 2 + 1, of 6 calls
    assert escalation_recall(r) == pytest.approx(2 / 4)  # hard incidents: 0, 1, 3; escalated 0, 1, 1
    assert correct_rate(r, "plain") == pytest.approx(3 / 4)
    assert correct_rate(r, "hard") == pytest.approx(2 / 4)
    # critical incidents: stream 1 one plain (missed); stream 2 one hard (correct); stream 3 one hard (missed)
    assert critical_miss_rate(r) == pytest.approx(2 / 3)
    cost = int(r["total_cost_ns"].sum())
    assert correct_per_cost(r) == pytest.approx(5 / cost * 1e9)
    with pytest.raises(ValueError, match="deadline"):
        correct_rate(r, "decoy")
    # A pooled count is the sum of the per-incident rows it names.
    inc = arm.incidents
    assert int(r["escalations_needed"].sum()) == int(inc.loc[inc["tier"] == "hard", "escalations"].sum())
    assert int(r["correct_hard"].sum()) == int(inc.loc[inc["tier"] == "hard", "correct_by_deadline"].sum())
    # An arm that never escalated has no precision, which is not a precision of zero.
    quiet = load_stream_arm(
        write_stream_arm(tmp_path / "q", [stream(1, [incident("plain"), incident("hard")])])
    )
    assert np.isnan(escalation_precision(quiet.results))
    assert escalation_recall(quiet.results) == 0.0


def test_family_table_has_a_row_for_every_family_pooled_over_streams(tmp_path):
    arm = load_stream_arm(write_stream_arm(tmp_path / "a", mixed_streams()))
    t = family_table(arm.incidents).set_index("family")
    assert t.index.tolist() == list(load_module.STREAM_FAMILIES)
    assert t["incidents"].tolist() == [1, 1, 1, 1]
    assert t.loc["compound", "correct"] == 1 and t.loc["slow_leak", "missed"] == 1
    assert t.loc["slow_leak", "critical_missed"] == 1 and t.loc["cascade", "escalated"] == 1
    assert t["incidents"].sum() == arm.results["incidents_hard"].sum()
    only = load_stream_arm(
        write_stream_arm(tmp_path / "b", [stream(1, [incident("hard", "compound")])])
    )
    empty = family_table(only.incidents).set_index("family")
    assert empty.loc["cascade", "incidents"] == 0 and np.isnan(empty.loc["cascade", "correct_rate"])


def test_arm_totals_are_exact_sums_with_the_number_of_streams(tmp_path):
    arm = load_stream_arm(write_stream_arm(tmp_path / "a", mixed_streams()))
    t = arm_totals(arm.results)
    assert t["streams"] == 3 and t["incidents_hard"] == 4 and t["reasoner_calls"] == 6
    assert t["total_cost_ns"] == t["substrate_ns"] + t["reasoner_cost_ns"]
    assert all(isinstance(v, int) for v in t.values())


# ---- stream-summary ------------------------------------------------------------------------


def test_stream_summary_prints_every_arm_with_totals_ratios_and_cost(tmp_path, capsys):
    roles = {"oracle_privileged": "privileged", "hidden_ablation": "ablation"}
    run = write_stream_run(
        tmp_path / "run",
        {"never": mixed_streams(), "oracle_privileged": mixed_streams(), "hidden_ablation": mixed_streams()},
        roles=roles,
    )
    assert main(["stream-summary", "--run", str(run)]) == 0
    out = capsys.readouterr().out
    lines = out.splitlines()
    assert "3 streams" in lines[0]
    arms = [ln.split()[0] for ln in lines[2:5]]
    assert arms == ["never", "hidden_ablation", "oracle_privileged"]  # comparison arms first
    assert "0.50" in lines[2] and "3/4 0.75" in lines[2] and "2/4 0.50" in lines[2]
    assert "pooled" in out and "esc prec" in out and "cost/stream" in out


def test_stream_summary_json_has_no_nan_and_names_the_pooled_ratios(tmp_path, capsys):
    run = write_stream_run(
        tmp_path / "run", {"quiet": [stream(1, [incident("plain")])], "busy": mixed_streams()[:1]}
    )
    out = tmp_path / "s.json"
    assert main(["stream-summary", "--run", str(run), "--json", str(out)]) == 0
    capsys.readouterr()
    data = json.loads(out.read_text())  # allow_nan=False in the writer: this parses
    assert data["streams"] == 1 and [a["arm"] for a in data["arms"]] == ["busy", "quiet"]
    quiet = data["arms"][1]
    assert quiet["escalation_precision"] is None and quiet["escalation_recall"] is None
    assert quiet["totals"]["reasoner_calls"] == 0
    assert set(quiet["cost_per_stream_ns"]) == {"substrate", "reasoner", "total"}
    assert [f["family"] for f in quiet["families"]] == list(load_module.STREAM_FAMILIES)


def test_stream_summary_errors_are_exit_two(tmp_path, capsys):
    assert main(["stream-summary", "--run", str(tmp_path / "nope")]) == 2
    assert "not a directory" in capsys.readouterr().err
    run = write_stream_run(tmp_path / "run", {"a": mixed_streams()})
    edit(run / "a", "results.csv", lambda t: [t[0] + ["x"], *[r + ["1"] for r in t[1:]]])
    assert main(["stream-summary", "--run", str(run)]) == 2
    assert "unknown" in capsys.readouterr().err


def test_summarize_run_orders_comparison_arms_before_the_references(tmp_path):
    run = load_stream_run(
        write_stream_run(
            tmp_path / "run",
            {"a_ablation": mixed_streams(), "z_never": mixed_streams(), "m_oracle": mixed_streams()},
            roles={"a_ablation": "ablation", "m_oracle": "privileged"},
        )
    )
    names = [a["arm"] for a in summarize_run(run)["arms"]]
    assert names == ["z_never", "a_ablation", "m_oracle"]
    assert stream_module.COMPARISON_ROLE == load_module.COMPARISON_ROLE == "comparison"
