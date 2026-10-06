"""B3's diagnostics (`scripts/b3_diagnostics.py`): the counts the report's analysis stands on, on frames built
here by hand (what a notice is anchored on, which incidents change hands between a row and its base, what a row
adds to and absorbs from its base). No run output is read."""

import sys
from pathlib import Path
from types import SimpleNamespace

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "experiments" / "exploration" / "scripts"))

import b3_common as C  # noqa: E402
import b3_diagnostics as D  # noqa: E402

INCIDENTS = pd.DataFrame([
    # seed, incident, tier, family
    (1, 0, "hard", "cascade"), (1, 1, "hard", "slow_leak"), (1, 2, "plain", None), (1, 3, "decoy", None),
    (2, 0, "hard", "compound"), (2, 1, "hard", "slow_leak"),
], columns=["seed", "incident", "tier", "family"])


def incidents(**flags):
    df = INCIDENTS.copy()
    n = len(df)
    df["first_observation_at_ns"] = np.arange(n) * 10**9
    df["notices"] = 1
    df["notice_latency_ns"] = 10**9
    df["noticed"] = flags.get("noticed", [True] * n)
    df["anchor_correct"] = flags.get("anchor_correct", [True] * n)
    return df


def events(rows):
    """rows: (seed, anchor, incident or None, offset_ns, anchor_correct)."""
    df = pd.DataFrame(rows, columns=["seed", "anchor", "incident", "anchor_offset_ns", "anchor_correct"])
    df["event"] = "notice"
    df["at_ns"] = 0
    return df


def arm(ni, ev):
    return SimpleNamespace(notice_incidents=ni, notice_events=ev, results=pd.DataFrame({"seed": [1, 2]}))


def run_of(arms_by_stem, default):
    arms = {C.arm_name(s): default for s, _ in D.PAIRS}
    arms.update({C.arm_name(b): default for _, b in D.PAIRS})
    arms.update({C.arm_name(s): a for s, a in arms_by_stem.items()})
    return SimpleNamespace(arms=arms)


def test_a_notice_is_classified_by_what_its_anchor_belongs_to():
    ev = events([(1, 10, None, np.nan, False), (1, 11, 0, 0, True), (1, 12, 1, 0, True), (1, 13, 2, 5, False),
                 (1, 14, 3, 0, True), (2, 15, 1, 0, True)])
    kinds = D.kind_of(ev, INCIDENTS)["kind"].tolist()
    assert kinds == ["background", "hard non-leak", "slow leak", "plain", "decoy", "slow leak"]


def test_a_notice_about_an_incident_of_another_seed_is_not_confused_with_the_same_id():
    # incident 0 is a cascade in seed 1 and a compound in seed 2: both hard non-leak; incident 1 in seed 2 is a leak
    ev = events([(2, 20, 0, 0, True), (2, 21, 1, 0, True)])
    assert D.kind_of(ev, INCIDENTS)["kind"].tolist() == ["hard non-leak", "slow leak"]


def test_transitions_count_gained_lost_kept_and_never_over_the_same_incidents():
    base = arm(incidents(anchor_correct=[True, False, True, True, False, False],
                         noticed=[True, False, True, True, True, False]), events([]))
    new = arm(incidents(anchor_correct=[False, True, True, True, False, True],
                        noticed=[True, True, True, True, False, True]), events([]))
    t = D.transitions(run_of({"ramp_over_r3": new}, base))
    row = t[(t.new == "ramp_over_r3") & (t.group == "hard non-leak") & (t.family == "all")].iloc[0]
    # hard non-leak: (1,0) cascade and (2,0) compound. Anchor-correct: (1,0) True->False lost; (2,0) False both: never
    assert row.incidents == 2
    assert (row.anchor_correct_lost, row.anchor_correct_gained, row.anchor_correct_kept) == (1, 0, 0)
    assert row.anchor_correct_never == 1
    # noticed: (1,0) kept, (2,0) True->False lost
    assert (row.noticed_kept, row.noticed_lost, row.noticed_gained) == (1, 1, 0)
    leak = t[(t.new == "ramp_over_r3") & (t.group == "slow leak") & (t.family == "all")].iloc[0]
    # leaks: (1,1) False->True gained anchor-correct, noticed False->True gained; (2,1) False->True gained both
    assert leak.incidents == 2 and (leak.anchor_correct_gained, leak.noticed_gained) == (2, 2)


def test_added_and_absorbed_notices_are_the_set_difference_by_seed_and_anchor():
    base = arm(INCIDENTS, events([(1, 100, None, np.nan, False), (1, 101, 2, 0, True), (2, 102, 0, 0, True)]))
    new = arm(INCIDENTS, events([(1, 101, 2, 0, True), (2, 102, 0, 0, True),   # kept
                                 (1, 200, 1, 0, True), (2, 201, 1, 0, True),   # two leak notices added
                                 (1, 202, 3, 7_000_000_000, False)]))          # one late decoy notice added
    t = D.added_notices(run_of({"ramp_over_r3": new}, base))
    added = t[(t.new == "ramp_over_r3") & t.side.str.startswith("added")].iloc[0]
    gone = t[(t.new == "ramp_over_r3") & t.side.str.startswith("absorbed")].iloc[0]
    assert (added.notices, added["slow leak"], added.decoy, added.background) == (3, 2, 1, 0)
    assert (added.anchored_on_first_observation, added.anchored_more_than_5s_after) == (2, 1)
    assert added.on_an_incident_the_base_also_noticed == 0   # incidents 1 (seed 1, 2) and 3 had no base notice
    assert (gone.notices, gone.background) == (1, 1)
    # a row identical to its base adds and absorbs nothing
    same = t[(t.new == "split_over_r3") & t.side.str.startswith("added")].iloc[0]
    assert same.notices == 0 and same["slow leak"] == 0


def test_the_hard_non_leak_and_leak_selectors_partition_the_hard_incidents():
    ni = incidents()
    hard = ni[ni.tier == "hard"]
    assert len(D.hard_nonleak(ni)) + len(D.leaks(ni)) == len(hard)
    assert set(D.leaks(ni).family) == {"slow_leak"}
