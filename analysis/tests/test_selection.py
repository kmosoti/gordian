"""The selection files of work item B4: the loader, its schema guard and the per-stream measures.

The measures are the evaluator's (`crates/gordian-stream-eval/RULES.md`, E1 to E8); the fixtures
here are made up in the harness's format (`selection_fixtures.py`) and the expected values below are
counted by hand from `selecting_streams()`.
"""

import re
from pathlib import Path

import pandas as pd
import pytest

from gordian_analysis import load as load_module
from gordian_analysis.load import LoadError, load_stream_arm
from gordian_analysis.stream import selection_per_stream
from notice_fixtures import write_notice_files
from selection_fixtures import selecting_streams, write_selection_files
from stream_fixtures import write_stream_arm

ROOT = Path(__file__).parents[2]
RESULTS_RS = ROOT / "crates/gordian-run/src/stream/results.rs"

# Seed 2's notices in order: a0 (the leak), a1 (hard), a2 (a decoy), a3 and a4 (a decoy, which has the
# incident's two calls on a3). `followup` below retires a0 and a2 by the rule.
FOLLOWUP = {1: {1, 2}, 2: {0, 2}}


def arm_dir(tmp_path, streams=None, name="arm", followup=FOLLOWUP) -> Path:
    streams = streams or selecting_streams()
    path = write_stream_arm(tmp_path / name, streams)
    write_notice_files(path, streams)
    write_selection_files(path, streams, followup=followup)
    return path


def edit(path: Path, name: str, fn) -> None:
    lines = [ln.split(",") for ln in (path / name).read_text().splitlines()]
    lines = fn(lines)
    (path / name).write_text("\n".join(",".join(x) for x in lines) + "\n")


def _constant(src: str, name: str) -> list[str]:
    m = re.search(rf'pub const {name}: &str =\s*"([^"]+)"', src)
    assert m, f"{name} not found in results.rs"
    return m.group(1).split(",")


def col(lines, name):
    return lines[0].index(name)


# ---- the schema guard -------------------------------------------------------------------------


def test_the_loaders_columns_are_the_harness_headers():
    if not RESULTS_RS.is_file():
        pytest.skip("harness source not present")
    src = RESULTS_RS.read_text()
    assert load_module.STREAM_SELECTION_COLUMNS == _constant(src, "SELECTION_HEADER")
    assert load_module.STREAM_SELECTION_NOTICES_COLUMNS == _constant(src, "SELECTION_NOTICES_HEADER")


def test_the_older_files_have_none_of_the_selection_columns():
    selection = set(load_module.STREAM_SELECTION_COLUMNS) - {"run_id", "arm_role", "seed", "noticer"}
    for cols in (
        load_module.STREAM_RESULTS_COLUMNS,
        load_module.STREAM_INCIDENTS_COLUMNS,
        load_module.STREAM_NOTICES_COLUMNS,
        load_module.STREAM_NOTICE_INCIDENTS_COLUMNS,
        load_module.STREAM_NOTICE_EVENTS_COLUMNS,
    ):
        assert not selection & set(cols)


# ---- loading ------------------------------------------------------------------------------------


def test_the_selection_files_load_with_the_arm(tmp_path):
    arm = load_stream_arm(arm_dir(tmp_path))
    assert list(arm.selection.columns) == load_module.STREAM_SELECTION_COLUMNS
    assert list(arm.selection_notices.columns) == load_module.STREAM_SELECTION_NOTICES_COLUMNS
    assert arm.selection["seed"].tolist() == [1, 2, 3]
    assert len(arm.selection_notices) == 5 + 5 + 2  # notices on incidents and background, all streams
    assert set(arm.selection_notices["class"]) == {"plain", "decoy", "background", "leak", "hard"}
    # Seed 1: the first notice of the plain incident carries its two calls and nothing else does.
    s1 = arm.selection_notices[arm.selection_notices["seed"] == 1]
    assert s1["escalations"].tolist() == [2, 0, 1, 1, 0]
    assert s1["retire_cause"].tolist() == ["quiet", "followup", "followup", "", ""]
    assert s1["retired_before_escalation"].tolist() == [False, True, False, False, False]
    assert s1["first_escalation_at_ns"].isna().tolist() == [False, True, False, False, True]


def test_a_run_without_selection_files_loads_without_them(tmp_path):
    streams = selecting_streams()
    path = write_stream_arm(tmp_path / "old", streams)
    write_notice_files(path, streams)
    arm = load_stream_arm(path)
    assert arm.selection is None and arm.selection_notices is None
    with pytest.raises(ValueError, match="selection files"):
        selection_per_stream(arm)


def test_the_selection_files_come_together_and_need_the_notice_files(tmp_path):
    path = arm_dir(tmp_path)
    (path / "selection_notices.csv").unlink()
    with pytest.raises(LoadError, match="come together"):
        load_stream_arm(path)
    streams = selecting_streams()
    path2 = write_stream_arm(tmp_path / "nonotices", streams)
    write_notice_files(path2, streams)
    write_selection_files(path2, streams)
    for f in ("notices.csv", "notice_incidents.csv", "notice_events.csv"):
        (path2 / f).unlink()
    with pytest.raises(LoadError, match="notice files"):
        load_stream_arm(path2)


# ---- the measures -------------------------------------------------------------------------------


def test_per_stream_counts_are_the_files_and_the_incident_level_counts_are_by_hand(tmp_path):
    arm = load_stream_arm(arm_dir(tmp_path))
    t = selection_per_stream(arm)
    assert t.index.tolist() == [1, 2, 3]
    # Seed 1: calls plain 2, decoy 1, background 1; notices plain 2 (a0, a1), decoy 1 (a2), background 2.
    assert t.loc[1, "calls_plain"] == 2 and t.loc[1, "calls_decoy"] == 1
    assert t.loc[1, "calls_background"] == 1 and t.loc[1, "calls_hard"] == 0
    assert t.loc[1, "tokens_plain"] == 200 and t.loc[1, "modelled_ns_plain"] == 2 * 25_000_000
    assert t.loc[1, "notices_plain"] == 2 and t.loc[1, "notices_decoy"] == 1
    assert t.loc[1, "notices_background"] == 2
    assert t.loc[1, "escalated_plain"] == 1 and t.loc[1, "escalated_decoy"] == 1
    # a1 (plain) is retired by the rule before it was asked about; a2 (the decoy) was asked, then retired.
    assert t.loc[1, "retired_before_escalation_plain"] == 1
    assert t.loc[1, "retired_before_escalation_decoy"] == 0
    assert t.loc[1, "followup_retired_plain"] == 1 and t.loc[1, "followup_retired_decoy"] == 1
    assert t.loc[1, "followup_before_escalation_plain"] == 1
    assert t.loc[1, "followup_before_escalation_decoy"] == 0
    assert t.loc[1, "escalations_unattributed"] == 0
    # Seed 2: a0 is the leak's only notice, retired by the rule before escalation: one leak incident hit
    # and lost. a2 (a decoy, never asked about) is retired by the rule: one decoy incident hit.
    assert t.loc[2, "leak_followup_hit"] == 1 and t.loc[2, "leak_lost"] == 1
    assert t.loc[2, "decoy_followup_hit"] == 1
    assert t.loc[2, "calls_hard"] == 1 and t.loc[2, "calls_decoy"] == 2 and t.loc[2, "calls_leak"] == 0
    assert t.loc[2, "followup_retired_leak"] == 1 and t.loc[2, "followup_before_escalation_leak"] == 1
    # Seed 3: nothing retired.
    assert t.loc[3, "followup_retired_leak"] == 0 and t.loc[3, "leak_lost"] == 0
    assert t.loc[3, "notices_leak"] == 1 and t.loc[3, "notices_background"] == 1
    assert (t >= 0).all().all()


def test_a_leak_that_was_asked_about_is_not_lost_even_if_a_notice_of_it_was_retired_by_the_rule(tmp_path):
    arm = load_stream_arm(arm_dir(tmp_path))
    sn = arm.selection_notices.copy()
    # Give seed 2's leak incident a second, escalated notice: hit but not lost. Built on the frame
    # (the per-stream function reads it), not on files, which the loader would check against others.
    extra = sn[(sn["seed"] == 2) & (sn["class"] == "leak")].copy()
    extra["escalations"] = 1
    extra["retire_cause"] = ""
    extra["retired_at_ns"] = pd.NA
    extra["retired_before_escalation"] = False
    arm.selection_notices = pd.concat([sn, extra], ignore_index=True)
    t = selection_per_stream(arm)
    assert t.loc[2, "leak_followup_hit"] == 1
    assert t.loc[2, "leak_lost"] == 0


# ---- the loader refuses what cannot be right --------------------------------------------------------


def test_the_loader_refuses_a_column_it_does_not_know(tmp_path):
    path = arm_dir(tmp_path)
    edit(path, "selection.csv", lambda ls: [[*ls[0], "extra"]] + [[*r, "0"] for r in ls[1:]])
    with pytest.raises(LoadError, match="stream schema"):
        load_stream_arm(path)


def test_the_classes_calls_must_add_up_to_the_reasoner_calls(tmp_path):
    path = arm_dir(tmp_path)

    def bump(ls):
        i = col(ls, "calls_plain")
        ls[1][i] = str(int(ls[1][i]) + 1)
        return ls

    edit(path, "selection.csv", bump)
    with pytest.raises(LoadError, match="calls by class"):
        load_stream_arm(path)


def test_tokens_and_cost_must_add_up_too(tmp_path):
    for field, text in (("tokens_decoy", "tokens"), ("modelled_ns_decoy", "modelled_ns")):
        path = arm_dir(tmp_path, name=field)

        def bump(ls, field=field):
            i = col(ls, field)
            ls[1][i] = str(int(ls[1][i]) + 1)
            return ls

        edit(path, "selection.csv", bump)
        with pytest.raises(LoadError, match=text):
            load_stream_arm(path)


def test_the_notices_by_class_must_be_the_notice_counts(tmp_path):
    path = arm_dir(tmp_path)

    def move(ls):
        a, b = col(ls, "notices_plain"), col(ls, "notices_decoy")
        ls[1][a] = str(int(ls[1][a]) - 1)
        ls[1][b] = str(int(ls[1][b]) + 1)
        return ls

    edit(path, "selection.csv", move)
    with pytest.raises(LoadError, match="notices disagree"):
        load_stream_arm(path)


def test_more_escalated_than_notices_is_refused(tmp_path):
    path = arm_dir(tmp_path)

    def bump(ls):
        i = col(ls, "escalated_background")
        ls[3][i] = "9"
        return ls

    edit(path, "selection.csv", bump)
    with pytest.raises(LoadError, match="more escalated than notices"):
        load_stream_arm(path)


def test_escalated_and_retired_before_escalation_cannot_overlap(tmp_path):
    path = arm_dir(tmp_path)

    def bump(ls):
        # Seed 1 has 2 plain notices, 1 escalated and 1 retired before escalation: one more of
        # either overlaps.
        i = col(ls, "retired_before_escalation_plain")
        ls[1][i] = "2"
        return ls

    edit(path, "selection.csv", bump)
    with pytest.raises(LoadError):
        load_stream_arm(path)


def test_a_class_that_is_not_one_is_refused(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        ls[1][col(ls, "class")] = "mimic"
        return ls

    edit(path, "selection_notices.csv", bad)
    with pytest.raises(LoadError, match="class"):
        load_stream_arm(path)


def test_background_has_no_incident_and_only_background(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        i = [k for k, r in enumerate(ls) if r[col(ls, "class")] == "background"][0]
        ls[i][col(ls, "incident")] = "0"
        return ls

    edit(path, "selection_notices.csv", bad)
    with pytest.raises(LoadError, match="background"):
        load_stream_arm(path)


def test_the_retirement_columns_must_agree_with_each_other(tmp_path):
    for name, change, match in (
        ("cause", lambda ls: ls[1].__setitem__(col(ls, "retire_cause"), ""), "empty together"),
        ("cause2", lambda ls: ls[1].__setitem__(col(ls, "retire_cause"), "mimic"), "retire_cause"),
        ("rbe", lambda ls: ls[1].__setitem__(col(ls, "retired_before_escalation"), "true"),
         "retired_before_escalation"),
        ("first", lambda ls: ls[1].__setitem__(col(ls, "first_escalation_at_ns"), ""), "first_escalation"),
    ):
        path = arm_dir(tmp_path, name=name)

        def bad(ls, change=change):
            change(ls)
            return ls

        edit(path, "selection_notices.csv", bad)
        with pytest.raises(LoadError, match=match):
            load_stream_arm(path)


def test_the_per_notice_rows_must_be_the_per_stream_counts(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        # Seed 1's second notice (a1) is retired by the rule: call it quiet.
        ls[2][col(ls, "retire_cause")] = "quiet"
        return ls

    edit(path, "selection_notices.csv", bad)
    with pytest.raises(LoadError, match="followup_retired"):
        load_stream_arm(path)


def test_the_notices_of_the_selection_file_are_the_notice_records(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        ls[1][col(ls, "anomaly")] = "77"
        return ls

    edit(path, "selection_notices.csv", bad)
    with pytest.raises(LoadError, match="in order"):
        load_stream_arm(path)


def test_the_calls_about_notices_and_the_unattributed_ones_are_the_reasoner_calls(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        ls[1][col(ls, "escalations_unattributed")] = "3"
        return ls

    edit(path, "selection.csv", bad)
    with pytest.raises(LoadError, match="unattributed"):
        load_stream_arm(path)


def test_the_retirements_must_be_the_notice_recorded_ones(tmp_path):
    path = arm_dir(tmp_path)

    def bad(ls):
        # Seed 1's fourth notice is background and was not retired: retire it.
        i = [k for k, r in enumerate(ls) if r[col(ls, "seed")] == "1"][3]
        ls[i][col(ls, "retire_cause")] = "quiet"
        ls[i][col(ls, "retired_at_ns")] = "31000000000"
        ls[i][col(ls, "retired_before_escalation")] = "false"
        return ls

    edit(path, "selection_notices.csv", bad)
    with pytest.raises(LoadError):
        load_stream_arm(path)
