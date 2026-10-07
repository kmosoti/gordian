"""The notice files of work item B1: the loader, its schema guard and the pooled notice measures.

The measures are the evaluator's (`crates/gordian-stream-eval/RULES.md`, N1 to N12); the fixtures
here are made up in the harness's format (`notice_fixtures.py`) and the expected values below are
counted by hand from `noticing_streams()`.
"""

import re
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

from gordian_analysis import load as load_module
from gordian_analysis.load import LoadError, load_stream_arm, load_stream_run
from gordian_analysis.stream import notice_latency, notice_per_stream, notice_points
from notice_fixtures import noticing_streams, write_notice_files
from stream_fixtures import mixed_streams, write_stream_arm

ROOT = Path(__file__).parents[2]
RESULTS_RS = ROOT / "crates/gordian-run/src/stream/results.rs"


def arm_dir(tmp_path, streams=None, noticer="rung", name="arm") -> Path:
    streams = streams or noticing_streams()
    path = write_stream_arm(tmp_path / name, streams)
    write_notice_files(path, streams, noticer=noticer)
    return path


def edit(path: Path, name: str, fn) -> None:
    lines = [ln.split(",") for ln in (path / name).read_text().splitlines()]
    lines = fn(lines)
    (path / name).write_text("\n".join(",".join(x) for x in lines) + "\n")


def _constant(src: str, name: str) -> list[str]:
    m = re.search(rf'pub const {name}: &str =\s*"([^"]+)"', src)
    assert m, f"{name} not found in results.rs"
    return m.group(1).split(",")


# ---- the schema guard -------------------------------------------------------------------------


def test_the_loaders_columns_are_the_harness_headers():
    if not RESULTS_RS.is_file():
        pytest.skip("harness source not present")
    src = RESULTS_RS.read_text()
    assert load_module.STREAM_NOTICES_COLUMNS == _constant(src, "NOTICES_HEADER")
    assert load_module.STREAM_NOTICE_INCIDENTS_COLUMNS == _constant(src, "NOTICE_INCIDENTS_HEADER")
    assert load_module.STREAM_NOTICE_EVENTS_COLUMNS == _constant(src, "NOTICE_EVENTS_HEADER")


def test_the_old_files_keep_their_columns_so_that_old_runs_and_hashes_stand():
    # The notice columns are in files of their own: the three older headers have none of them.
    for cols in (
        load_module.STREAM_RESULTS_COLUMNS,
        load_module.STREAM_INCIDENTS_COLUMNS,
        load_module.STREAM_MEASURED_COLUMNS,
    ):
        assert not [c for c in cols if "notice" in c and c not in ("anomalies_noticed", "noticer_ns")]
    assert len(load_module.STREAM_RESULTS_LEGACY_COLUMNS) == 59
    assert len(load_module.STREAM_RESULTS_COLUMNS) == 61  # work item E1 appended two
    assert len(load_module.STREAM_INCIDENTS_COLUMNS) == 17


# ---- loading ----------------------------------------------------------------------------------


def test_a_run_without_notice_files_loads_as_before_with_no_notice_tables(tmp_path):
    path = write_stream_arm(tmp_path / "old", mixed_streams())
    arm = load_stream_arm(path)
    assert arm.notices is None and arm.notice_incidents is None and arm.notice_events is None
    with pytest.raises(ValueError, match="no notice files"):
        notice_points(arm)


def test_the_notice_files_load_keyed_like_the_files_beside_them(tmp_path):
    arm = load_stream_arm(arm_dir(tmp_path))
    assert list(arm.notices.columns) == load_module.STREAM_NOTICES_COLUMNS
    assert list(arm.notice_incidents.columns) == load_module.STREAM_NOTICE_INCIDENTS_COLUMNS
    assert list(arm.notice_events.columns) == load_module.STREAM_NOTICE_EVENTS_COLUMNS
    assert arm.notices["seed"].tolist() == [1, 2, 3]
    key = ["seed", "incident", "tier", "family"]
    assert arm.notice_incidents[key].equals(arm.incidents[key])
    # Nullable instants are empty exactly where there is nothing to report.
    ni = arm.notice_incidents
    assert ni.loc[ni["noticed"], "first_notice_at_ns"].notna().all()
    assert ni.loc[~ni["noticed"], "notice_latency_ns"].isna().all()
    # A retirement has no verdict, a notice has one.
    ev = arm.notice_events
    assert ev.loc[ev["event"] == "retire", "anchor_correct"].isna().all()
    assert ev.loc[ev["event"] == "notice", "anchor_correct"].notna().all()
    assert int((ev["event"] == "notice").sum()) == int(arm.results["anomalies_noticed"].sum())


def test_a_run_directory_loads_notices_for_every_arm_that_has_them(tmp_path):
    run = tmp_path / "run"
    arm_dir(run, name="a", noticer="rung")
    arm_dir(run, name="b", noticer="change_triggered")
    loaded = load_stream_run(run)
    assert {n: a.notices["noticer"].iloc[0] for n, a in loaded.arms.items()} == {
        "a": "rung",
        "b": "change_triggered",
    }


def test_an_arm_with_no_notice_at_all_has_an_empty_event_table(tmp_path):
    streams = mixed_streams()
    from notice_fixtures import not_noticed, with_notices

    quiet = [with_notices(s, [not_noticed() for _ in s["incidents"]]) for s in streams]
    arm = load_stream_arm(arm_dir(tmp_path, quiet))
    assert len(arm.notice_events) == 0
    assert int(arm.notices["notices"].sum()) == 0
    assert notice_points(arm)["hard_noticed_share"] == 0.0


# ---- refusals ---------------------------------------------------------------------------------


def test_the_three_notice_files_come_together(tmp_path):
    path = arm_dir(tmp_path)
    (path / "notice_events.csv").unlink()
    with pytest.raises(LoadError, match="come together"):
        load_stream_arm(path)


@pytest.mark.parametrize("name", ["notices.csv", "notice_incidents.csv", "notice_events.csv"])
def test_a_notice_file_must_have_exactly_the_harness_columns(tmp_path, name):
    path = arm_dir(tmp_path)
    edit(path, name, lambda rows: [r[:-1] for r in rows])
    with pytest.raises(LoadError, match="columns"):
        load_stream_arm(path)


def test_notices_must_be_the_anomalies_the_rung_reports_noticing(tmp_path):
    streams = noticing_streams()
    streams[0] = {**streams[0], "anomalies_noticed": streams[0]["anomalies_noticed"] + 1}
    path = write_stream_arm(tmp_path / "arm", streams)
    write_notice_files(path, noticing_streams())
    with pytest.raises(LoadError, match="anomalies_noticed"):
        load_stream_arm(path)


def test_the_notices_by_anchor_must_add_up(tmp_path):
    path = arm_dir(tmp_path)

    def bump(rows):
        col = rows[0].index("notices_on_plain")
        rows[1][col] = str(int(rows[1][col]) + 1)
        return rows

    edit(path, "notices.csv", bump)
    with pytest.raises(LoadError, match="do not add up"):
        load_stream_arm(path)


def test_the_incident_notice_rows_must_be_the_incidents(tmp_path):
    path = arm_dir(tmp_path)

    def retier(rows):
        col = rows[0].index("tier")
        rows[1][col] = "decoy" if rows[1][col] != "decoy" else "plain"
        return rows

    edit(path, "notice_incidents.csv", retier)
    with pytest.raises(LoadError, match="not the incidents"):
        load_stream_arm(path)


def test_an_incident_cannot_be_anchor_correct_without_being_noticed(tmp_path):
    path = arm_dir(tmp_path)

    def flip(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("noticed")] == "false":
                r[h.index("anchor_correct")] = "true"
                break
        return rows

    edit(path, "notice_incidents.csv", flip)
    with pytest.raises(LoadError, match="anchor-correct but not noticed"):
        load_stream_arm(path)


def test_an_incident_cannot_be_site_correct_without_being_noticed(tmp_path):
    path = arm_dir(tmp_path)

    def flip(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("noticed")] == "false":
                r[h.index("site_correct")] = "true"
                break
        return rows

    edit(path, "notice_incidents.csv", flip)
    with pytest.raises(LoadError, match="site-correct but not noticed"):
        load_stream_arm(path)


def test_one_notice_that_is_both_needs_the_incident_to_be_both_anchor_and_site_correct(tmp_path):
    path = arm_dir(tmp_path)

    def flip(rows):
        h = rows[0]
        for r in rows[1:]:
            # a noticed incident that is site-correct but whose anchor is late
            if r[h.index("noticed")] == "true" and r[h.index("anchor_correct")] == "false":
                r[h.index("site_correct")] = "true"
                r[h.index("anchor_site_correct")] = "true"
                break
        return rows

    edit(path, "notice_incidents.csv", flip)
    with pytest.raises(LoadError, match="anchor-and-site-correct but not both"):
        load_stream_arm(path)


def test_the_stream_row_must_agree_with_the_incident_rows_on_the_site_check(tmp_path):
    for column in ("site_correct_hard", "anchor_site_correct_plain"):
        path = arm_dir(tmp_path, name=column)

        def bump(rows, column=column):
            col = rows[0].index(column)
            rows[2][col] = str(int(rows[2][col]) + 1)
            return rows

        edit(path, "notices.csv", bump)
        with pytest.raises(LoadError, match=column):
            load_stream_arm(path)


def test_the_stream_counts_of_site_correct_notices_must_nest(tmp_path):
    path = arm_dir(tmp_path)

    def bump(rows):
        col = rows[0].index("notices_anchor_site_correct")
        rows[1][col] = str(int(rows[1][col]) + 5)
        return rows

    edit(path, "notices.csv", bump)
    with pytest.raises(LoadError, match="notices_anchor_site_correct <= notices_site_correct"):
        load_stream_arm(path)


def test_the_event_rows_must_agree_with_the_site_counts(tmp_path):
    path = arm_dir(tmp_path)

    def clear(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("site_correct")] == "true":
                r[h.index("site_correct")] = "false"
                r[h.index("anchor_site_correct")] = "false"
                break
        return rows

    edit(path, "notice_events.csv", clear)
    with pytest.raises(LoadError, match="notices_site_correct"):
        load_stream_arm(path)


def test_an_event_flag_for_both_must_be_the_conjunction(tmp_path):
    path = arm_dir(tmp_path)

    def flip(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("event")] == "notice" and r[h.index("anchor_correct")] == "false":
                r[h.index("anchor_site_correct")] = "true"
                break
        return rows

    edit(path, "notice_events.csv", flip)
    with pytest.raises(LoadError, match="anchor_site_correct is not"):
        load_stream_arm(path)


def test_noticed_first_notice_and_latency_must_agree(tmp_path):
    path = arm_dir(tmp_path)

    def empty_first(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("noticed")] == "true":
                r[h.index("first_notice_at_ns")] = ""
                break
        return rows

    edit(path, "notice_incidents.csv", empty_first)
    with pytest.raises(LoadError, match="first_notice_at_ns"):
        load_stream_arm(path)


def test_the_stream_row_must_agree_with_the_incident_rows(tmp_path):
    path = arm_dir(tmp_path)

    def bump(rows):
        col = rows[0].index("noticed_hard")
        rows[2][col] = str(int(rows[2][col]) + 1)
        return rows

    edit(path, "notices.csv", bump)
    with pytest.raises(LoadError, match="noticed_hard"):
        load_stream_arm(path)


def test_the_event_rows_must_agree_with_the_counts(tmp_path):
    path = arm_dir(tmp_path)
    lines = (path / "notice_events.csv").read_text().splitlines()
    (path / "notice_events.csv").write_text("\n".join(lines[:-1]) + "\n")
    with pytest.raises(LoadError, match="notice_events.csv disagrees"):
        load_stream_arm(path)


def test_an_event_is_a_notice_or_a_retirement_and_a_retirement_has_no_verdict(tmp_path):
    path = arm_dir(tmp_path)
    edit(path, "notice_events.csv", lambda rows: [rows[0]] + [[*r[:4], "promote", *r[5:]] for r in rows[1:]])
    with pytest.raises(LoadError, match="is not one of"):
        load_stream_arm(path)
    path = arm_dir(tmp_path, name="second")

    def verdict_on_retirement(rows):
        h = rows[0]
        for r in rows[1:]:
            if r[h.index("event")] == "retire":
                r[h.index("anchor_correct")] = "true"
                break
        return rows

    edit(path, "notice_events.csv", verdict_on_retirement)
    with pytest.raises(LoadError, match="empty exactly on retirements"):
        load_stream_arm(path)
    for flag in ("site_correct", "anchor_site_correct"):
        path = arm_dir(tmp_path, name=f"third-{flag}")

        def on_retirement(rows, flag=flag):
            h = rows[0]
            for r in rows[1:]:
                if r[h.index("event")] == "retire":
                    r[h.index(flag)] = "true"
                    break
            return rows

        edit(path, "notice_events.csv", on_retirement)
        with pytest.raises(LoadError, match=f"{flag} is empty exactly on retirements"):
            load_stream_arm(path)


# ---- the measures ----------------------------------------------------------------------------


def test_per_stream_numerators_and_denominators_match_a_hand_count(tmp_path):
    t = notice_per_stream(load_stream_arm(arm_dir(tmp_path)))
    assert t.index.tolist() == [1, 2, 3]
    # hard incidents outside the slow leak: stream 2's compound; stream 3's cascade and split brain
    assert t["hard_n"].tolist() == [0, 1, 2]
    assert t["hard_noticed"].tolist() == [0, 1, 1]
    assert t["hard_correct"].tolist() == [0, 0, 1]
    assert t["leak_n"].tolist() == [0, 0, 1]
    assert t["leak_noticed"].tolist() == [0, 0, 1]
    assert t["leak_correct"].tolist() == [0, 0, 0]
    assert t["plain_n"].tolist() == [2, 1, 1]
    assert t["plain_noticed"].tolist() == [1, 1, 1]
    assert t["notices"].tolist() == [4, 5, 5]
    assert t["notices_background"].tolist() == [2, 1, 0]
    assert t["notices_plain"].tolist() == [1, 2, 3]
    assert t["notices_hard"].tolist() == [0, 2, 2]
    assert t["notices_decoy"].tolist() == [1, 0, 0]
    assert t["notices_on_incidents"].tolist() == [2, 4, 5]
    assert t["incidents"].tolist() == [3, 3, 4]
    assert t["retirements"].tolist() == [1, 2, 0]
    # The site check (N14, N15), counted by hand from `noticing_streams()`: stream 1's plain
    # incident is site-correct and both; its decoy is anchor-correct at the wrong service. Stream
    # 2's plain incident is both and its hard one is site-correct with a late anchor. Stream 3's
    # leak is neither, its split brain and its plain incident are both.
    assert t["hard_site"].tolist() == [0, 1, 1]
    assert t["hard_both"].tolist() == [0, 0, 1]
    assert t["leak_site"].tolist() == [0, 0, 0]
    assert t["leak_both"].tolist() == [0, 0, 0]
    assert t["plain_site"].tolist() == [1, 1, 1]
    assert t["plain_both"].tolist() == [1, 1, 1]
    assert t["decoy_site"].tolist() == [0, 0, 0]
    assert t["decoy_both"].tolist() == [0, 0, 0]
    assert t["notices_site"].tolist() == [1, 2, 2]
    assert t["notices_both"].tolist() == [1, 1, 2]


def test_the_pooled_measures_are_ratios_of_sums_not_means_of_ratios(tmp_path):
    p = notice_points(load_stream_arm(arm_dir(tmp_path, noticer="rung")))
    assert p["noticer"] == "rung"
    assert p["streams"] == 3
    assert p["hard_incidents"] == 3
    assert p["hard_noticed_share"] == pytest.approx(2 / 3)
    assert p["hard_anchor_correct_share"] == pytest.approx(1 / 3)
    assert p["leak_incidents"] == 1
    assert p["leak_noticed_share"] == 1.0
    assert p["leak_anchor_correct_share"] == 0.0
    assert p["plain_noticed_share"] == pytest.approx(3 / 4)
    assert p["notices_per_stream"] == pytest.approx(14 / 3)
    assert p["notices_on_background_per_stream"] == pytest.approx(1.0)
    assert p["notices_on_plain_per_stream"] == pytest.approx(2.0)
    assert p["notices_on_hard_per_stream"] == pytest.approx(4 / 3)
    assert p["notices_on_decoy_per_stream"] == pytest.approx(1 / 3)
    assert p["notices_per_incident"] == pytest.approx(11 / 10)
    assert p["retirements_per_stream"] == pytest.approx(1.0)
    # The site check and precision (N14 to N16), counted by hand: 14 notices, 11 on incidents (6 on
    # plain, 4 on hard, 1 on decoys), 5 site-correct, 4 anchor-correct and site-correct.
    assert p["hard_site_correct_share"] == pytest.approx(2 / 3)
    assert p["hard_anchor_site_correct_share"] == pytest.approx(1 / 3)
    assert p["leak_site_correct_share"] == 0.0
    assert p["leak_anchor_site_correct_share"] == 0.0
    assert p["notice_precision"] == pytest.approx(11 / 14)
    assert p["precision_plain"] == pytest.approx(6 / 14)
    assert p["precision_hard"] == pytest.approx(4 / 14)
    assert p["precision_decoy"] == pytest.approx(1 / 14)
    assert p["precision_plain"] + p["precision_hard"] + p["precision_decoy"] == pytest.approx(
        p["notice_precision"]
    )
    assert p["strict_precision"] == pytest.approx(4 / 14)
    assert p["site_correct_notice_share"] == pytest.approx(5 / 14)
    # A mean of per-stream shares would give another number: stream 2 has one hard incident and
    # stream 3 has two, and stream 1 has none (its share is undefined, not zero).
    per_stream = notice_per_stream(load_stream_arm(arm_dir(tmp_path, name="again")))
    shares = (per_stream["hard_noticed"] / per_stream["hard_n"]).dropna()
    assert shares.mean() != pytest.approx(p["hard_noticed_share"])


def test_a_share_is_nan_when_there_is_nothing_to_divide_by(tmp_path):
    from notice_fixtures import with_notices
    from stream_fixtures import incident, stream

    streams = [with_notices(stream(1, [incident("plain")]), [noticed_row()])]
    p = notice_points(load_stream_arm(arm_dir(tmp_path, streams)))
    assert np.isnan(p["hard_noticed_share"]) and np.isnan(p["leak_noticed_share"])


def noticed_row():
    from notice_fixtures import noticed

    return noticed()


def test_latency_is_reported_among_the_noticed_with_the_counts_beside_it(tmp_path):
    table = notice_latency(load_stream_arm(arm_dir(tmp_path))).set_index("group")
    assert table.loc["hard", "incidents"] == 3 and table.loc["hard", "noticed"] == 2
    assert table.loc["hard", "median_s"] == pytest.approx(3.0)
    assert table.loc["hard", "p90_s"] == pytest.approx(3.8)
    assert table.loc["slow_leak", "incidents"] == 1
    assert table.loc["slow_leak", "median_s"] == pytest.approx(18.0)
    assert table.loc["plain", "incidents"] == 4 and table.loc["plain", "noticed"] == 3
    assert table.loc["plain", "median_s"] == pytest.approx(1.0)
    assert table.loc["plain", "p90_s"] == pytest.approx(1.4)


def test_the_notice_measures_do_not_depend_on_the_order_of_the_rows(tmp_path):
    path = arm_dir(tmp_path)
    before = notice_points(load_stream_arm(path))
    lines = (path / "notice_incidents.csv").read_text().splitlines()
    shuffled = [lines[0]] + lines[1:][::-1]
    (path / "notice_incidents.csv").write_text("\n".join(shuffled) + "\n")
    assert notice_points(load_stream_arm(path)) == before
