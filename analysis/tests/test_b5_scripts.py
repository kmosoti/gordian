"""B5's scripts (`scripts/b5_*.py`): the arm spellings the Rust side reads, the rows and arms of each
manifest, the verified-decision arithmetic, the feature AUC script, and (when it exists) the tuning rule.
The scripts are exploration code outside the package, so the test puts the directories on the path. It
reads no run output: every input is built here (the committed selections of B3, M2 and M3 are read, as
the scripts read them)."""

import copy
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import b5_auc as AUC  # noqa: E402
import b5_common as C  # noqa: E402
import b5_manifests as MAN  # noqa: E402
import b5_stats as S  # noqa: E402
from gordian_analysis.load import load_stream_run  # noqa: E402
from notice_fixtures import write_notice_files  # noqa: E402
from selection_fixtures import selecting_streams, write_selection_files  # noqa: E402
from stream_fixtures import write_stream_arm  # noqa: E402

SCORE_KEYS = {"threshold", "contradiction", "silence", "evidence", "services", "age"}


# ---- the spellings the Rust side reads --------------------------------------------------------------


def test_the_policies_are_what_the_rust_side_reads():
    assert C.never_policy() == "never_escalate"
    assert C.always_policy() == {"policy": "always_escalate", "delay_ns": 16_000_000_000}
    assert C.oracle_policy() == {"policy": "oracle_selection", "delay_ns": 16_000_000_000}
    assert C.oracle_policy(8) == {"policy": "oracle_selection", "delay_ns": 8_000_000_000}
    p = C.budgeted_policy(8, {"evidence": 1.5, "threshold": 0.25})
    assert p == {"policy": "public_budgeted", "delay_ns": 16_000_000_000, "k": 8,
                 "score": {"threshold": 0.25, "contradiction": 0.0, "silence": 0.0, "evidence": 1.5,
                           "services": 0.0, "age": 0.0}}
    assert set(p["score"]) == SCORE_KEYS, "every field is written: the Rust side refuses a partial score"
    assert all(isinstance(v, float) for v in p["score"].values())
    assert C.budgeted_policy(2, C.FLAT)["score"] == {k: 0.0 for k in SCORE_KEYS}
    assert C.DELAY_S == 16 and C.KS == [2, 4, 8, 16] and C.SWEEP_DELAYS_S == [8, 12, 16, 20]


def test_the_arm_names_are_distinct_legal_and_short():
    stems = [s for s, _, _ in C.table_rows()] + [s for s, _, _ in C.sweep_rows()]
    names = []
    for stem in stems:
        names += [C.never_arm(stem), C.always_arm(stem), C.oracle_arm(stem)]
        names += [C.fk_arm(stem, k) for k in C.KS] + [C.bud_arm(stem, k) for k in C.KS]
        names += [C.oracle_arm(stem, d) for d in C.SWEEP_DELAYS_S]
    for name in names:
        assert all(c.isalnum() or c in "_-." for c in name), name
        assert len(name) <= 56, name
    oracles = [n for n in names if "privileged" in n]
    assert all(n.endswith("_privileged") for n in oracles)
    assert all("privileged" not in n for n in names if n not in oracles), "only the oracle is privileged"
    # the 16 s oracle is B3's and B4's own arm name
    assert C.oracle_arm(C.COMPARATOR) == f"sel_{C.COMPARATOR}_privileged" == C.C4.oracle_arm(C.COMPARATOR)
    assert C.oracle_arm(C.COMPARATOR, 16) == C.oracle_arm(C.COMPARATOR)
    assert C.oracle_arm(C.COMPARATOR, 8) != C.oracle_arm(C.COMPARATOR, 12)


# ---- the rows ------------------------------------------------------------------------------------------


def test_the_table_rows_are_b3s_the_dataflow_row_and_the_two_units_media_with_their_billing():
    rows = C.table_rows()
    stems = [s for s, _, _ in rows]
    assert len(stems) == len(set(stems)) == 11 + 1 + 6
    assert stems[:11] == [s for s, _ in C.b3_rows()]
    assert C.COMPARATOR in stems[:11]
    billed = {s: b for s, _, b in rows}
    assert not any(billed[s] for s in stems[:11]), "B3's rows are hand-written and unbilled"
    assert billed["df_" + C.COMPARATOR] and all(billed[s] for s in stems if s[:3] in ("m2_", "m3_"))
    by = {s: n for s, n, _ in rows}
    df = by["df_" + C.COMPARATOR]
    hand = by[C.COMPARATOR]
    assert df["noticer"] == "dataflow" and hand["noticer"] == "composed"
    assert (df["base"], df["ramp"], df["split"]) == (hand["base"], hand["ramp"], hand["split"])
    assert "billed" not in df, "billed is the default and is not written"
    for unit in ("m2", "m3"):
        for t in C.TICKS_MS:
            n = by[f"{unit}_t{t}"]
            assert n["noticer"] == "medium" and n["tick_ns"] == t * 1_000_000
    # M2's and M3's frozen graphs at the same tick are different graphs (M3 added the pruning)
    assert by["m2_t500"] != by["m3_t500"]


def test_the_sweep_rows_are_the_comparator_the_reanchor_the_two_media_and_the_dataflow_twins():
    rows = C.sweep_rows()
    assert [s for s, _, _ in rows] == [C.COMPARATOR, "reanchor", "df_" + C.COMPARATOR, "m2_t100", "m3_t500",
                                       "df_reanchor"]
    by = {s: n for s, n, _ in rows}
    ra = by["reanchor"]
    assert by["df_reanchor"] == {"noticer": "dataflow", "base": ra}, "the re-anchor alone, as the base"
    assert C.row_json("df_reanchor") == by["df_reanchor"]
    with pytest.raises(KeyError):
        C.row_json("nothing")


def test_the_tuning_rows_are_the_comparator_and_the_medium_at_100_ms_and_exist():
    assert C.TUNING_ROWS == [C.COMPARATOR, "m2_t100"]
    for stem in C.TUNING_ROWS + C.AUC_ROWS:
        assert C.row_json(stem)


# ---- the manifests' arms ---------------------------------------------------------------------------------


SELECTED = {"score": {str(k): {"threshold": 0.1 * k, "evidence": 1.0} for k in C.KS}}


def test_a_table_row_has_the_oracle_never_always_and_per_k_the_baseline_and_the_tuned_selector():
    arms = MAN.row_arms("xyz", {"noticer": "rung"}, SELECTED)
    assert len(arms) == 3 + 2 * len(C.KS)
    names = [a[0] for a in arms]
    assert len(set(names)) == len(names)
    pol = {a[0]: a[1] for a in arms}
    assert pol[C.oracle_arm("xyz")]["policy"] == "oracle_selection"
    assert pol[C.never_arm("xyz")] == "never_escalate"
    assert pol[C.always_arm("xyz")]["policy"] == "always_escalate"
    for k in C.KS:
        assert pol[C.fk_arm("xyz", k)]["score"] == {key: 0.0 for key in SCORE_KEYS}
        assert pol[C.fk_arm("xyz", k)]["k"] == k
        assert pol[C.bud_arm("xyz", k)]["k"] == k
        assert pol[C.bud_arm("xyz", k)]["score"]["threshold"] == pytest.approx(0.1 * k)
        assert pol[C.bud_arm("xyz", k)]["score"]["evidence"] == 1.0
    assert all(a[2] == {"noticer": "rung"} for a in arms), "every arm of the row has the row's noticer"


def test_every_arm_of_the_held_out_table_and_sweep_is_distinct_and_the_16_s_oracle_is_shared():
    table = []
    for stem, noticer, _ in C.table_rows():
        table += MAN.row_arms(stem, noticer, SELECTED)
    names = [a[0] for a in table]
    assert len(names) == len(set(names)) == 18 * (3 + 2 * 4)
    sweep = [(C.oracle_arm(s, d)) for s, _, _ in C.sweep_rows() for d in C.SWEEP_DELAYS_S]
    assert len(sweep) == len(set(sweep)) == 6 * 4


# ---- verified decisions -----------------------------------------------------------------------------------


def correct(streams_, spots):
    """`streams_` with the incidents at `spots` ((stream index, incident index) pairs) declared correctly
    by their deadline."""
    out = copy.deepcopy(streams_)
    for si, ii in spots:
        inc = out[si]["incidents"][ii]
        inc.update(correct_by_deadline=True, correct_declarations=1, missed=False, critical_miss=False,
                   first_correct_at_ns=5_000_000_000, time_to_first_correct_ns=2_000_000_000)
    return out


# selecting_streams: stream 1 = plain, decoy, plain; stream 2 = leak, compound, decoy, decoy;
# stream 3 = leak, plain. Arm a gets one plain in stream 1, the leak and the compound in stream 2 and the
# plain of stream 3 right; arm b the same and the other plain of stream 1.
A_RIGHT = [(0, 0), (1, 0), (1, 1), (2, 1)]
B_RIGHT = A_RIGHT + [(0, 2)]


@pytest.fixture
def run(tmp_path):
    base = selecting_streams()
    root = tmp_path / "run"
    for name, spots in (("a", A_RIGHT), ("b", B_RIGHT)):
        s = correct(base, spots)
        path = write_stream_arm(root / name, s)
        write_notice_files(path, s)
        write_selection_files(path, s)
    return load_stream_run(root)


def test_verified_decisions_are_the_plain_and_hard_incidents_correct_by_their_deadline(run):
    m = S.Measures(run)
    p = m.points()
    a, b = m.row["a"], m.row["b"]
    # plain: 2 of arm a's (stream 1 and 3); hard: the leak and the compound of stream 2. Hard includes leaks.
    assert p["verified_per_stream"][a] == pytest.approx(4 / 3)
    assert p["verified_plain_per_stream"][a] == pytest.approx(2 / 3)
    assert p["verified_hard_per_stream"][a] == pytest.approx(2 / 3)
    assert p["verified_per_stream"][b] == pytest.approx(5 / 3)
    # hard quality excludes the leaks: the compound of stream 2 is the one non-leak hard incident
    assert p["quality"][a] == pytest.approx(1.0)
    # leaks: stream 2's is right, stream 3's is not
    assert p["leak_quality"][a] == pytest.approx(0.5)
    assert p["hard_incidents_per_stream"][a] == pytest.approx(3 / 3)
    assert p["plain_incidents_per_stream"][a] == pytest.approx(3 / 3)
    counts = S.tuning_counts(run.arms["a"])
    assert counts["verified"] == 4 and counts["verified_plain"] == 2 and counts["verified_hard"] == 2
    assert counts["streams"] == 3 and counts["quality_num"] == 1 and counts["quality_den"] == 1
    assert counts["calls"] == 7, "calls by class: plain 2, decoys 1 + 2, hard 1, background 1"


def test_the_paired_difference_is_over_the_same_resamples_and_exact_for_a_constant_gain(run):
    m = S.Measures(run)
    res = m.paired(["verified_per_stream"], [("verified_per_stream", "b", "a")], seed=1, resamples=500)
    point, lo, hi = res[("verified_per_stream", "b", "a")]
    # b gains one plain decision in stream 1 only: the difference is 1 in a third of the streams
    assert point == pytest.approx(1 / 3)
    assert lo <= point <= hi
    assert lo >= 0.0 and hi <= 1.0, "a gain that is never negative and never above one"
    # an arm against itself is exactly zero at every resample
    zero = m.paired(["verified_per_stream"], [("verified_per_stream", "a", "a")], seed=1, resamples=100)
    assert zero[("verified_per_stream", "a", "a")] == (0.0, 0.0, 0.0)


# ---- the feature AUCs --------------------------------------------------------------------------------------


def feature_log():
    rng = np.random.default_rng(4)
    rows = []
    for seed in range(40):
        for i in range(12):
            cls = ["plain", "plain", "plain", "hard", "decoy", "leak"][i % 6]
            ready = not (cls == "decoy" and seed % 2 == 0)
            evidence = rng.integers(2, 12) + (8 if cls in ("hard", "leak") else 0)
            rows.append({
                "arm": "feat_a", "seed": seed, "anomaly": i, "class": cls, "incident": i, "ready": ready,
                "now_ns": 0, "noticed_at_ns": 0, "anchor_at_ns": 0,
                "contradiction": int(rng.random() < 0.5), "silence": int(rng.random() < 0.5),
                "evidence_n": evidence, "services_n": rng.integers(1, 4), "age_s": 16.0 + rng.random(),
                "score_z": rng.normal(), "peak_z": rng.normal(),
                "evidence": 0.0, "services": 0.0, "age": 0.0, "asked": 1, "first_escalation_at_ns": 1,
            })
    df = pd.DataFrame(rows)
    df["ready"] = df["ready"].map(lambda b: "true" if b else "false")
    return df


def test_the_feature_auc_script_separates_a_feature_that_carries_the_class_and_not_one_that_does_not(tmp_path):
    path = tmp_path / "log.csv"
    feature_log().to_csv(path, index=False)
    df = AUC.load(path)
    table = AUC.auc_rows(df, resamples=800, seed=3)
    get = lambda cmp, feat: table[(table["comparison"] == cmp) & (table["feature"] == feat)].iloc[0]  # noqa: E731
    ev = get("hard_vs_plain", "evidence")
    assert ev["auc"] > 0.9 and ev["lo"] > 0.5 and ev["excludes_half"]
    assert bool(ev["in_the_score"])
    ct = get("hard_vs_plain", "contradiction")
    assert ct["lo"] < 0.5 < ct["hi"] and not ct["excludes_half"]
    assert not bool(get("hard_vs_plain", "rung_score_z")["in_the_score"])
    # leak against decoy: both classes are present among the ready ones
    ld = get("leak_vs_decoy", "evidence")
    assert ld["positives"] > 0 and ld["negatives"] > 0 and ld["auc"] > 0.9
    # the "hard or leak" pool has the positives of both
    hl = get("hard_or_leak_vs_plain", "evidence")
    assert hl["positives"] == get("hard_vs_plain", "evidence")["positives"] + ld["positives"]
    assert len(table) == len(AUC.COMPARISONS) * (len(AUC.FEATURES) + len(AUC.NOT_IN_THE_SCORE))


def test_the_population_counts_ready_and_not_ready_by_class(tmp_path):
    path = tmp_path / "log.csv"
    feature_log().to_csv(path, index=False)
    pop = AUC.population(AUC.load(path))
    row = lambda cls: pop[pop["class"] == cls].iloc[0]  # noqa: E731
    assert row("plain")["notices"] == row("plain")["ready"] == 40 * 6
    assert row("decoy")["notices"] == 40 * 2 and row("decoy")["not_ready"] == 40 * 2 - row("decoy")["ready"]
    assert row("decoy")["not_ready"] > 0
    assert row("hard")["notices_per_stream"] == pytest.approx(2.0)
    assert row("background")["notices"] == 0


def test_an_auc_over_only_not_ready_notices_is_nan_and_not_a_crash(tmp_path):
    df = feature_log()
    df["ready"] = "false"
    path = tmp_path / "log.csv"
    df.to_csv(path, index=False)
    table = AUC.auc_rows(AUC.load(path), resamples=50, seed=1)
    assert table["auc"].isna().all()
    assert not table["excludes_half"].any()
