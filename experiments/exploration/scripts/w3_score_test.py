#!/usr/bin/env python3
"""Tests of `w3_score.py` on a synthetic stream whose answers are known by construction.

Run: python -B w3_score_test.py      (from this directory; no pytest needed)

One stream (seed 1), five services, public graph 1 depends on 0 and 3 depends on 2, an edge added
at 300 s (4 depends on 2). Incidents:

  0  hard cascade, site 0, partner 2: site alarm obs 100 at 10 s, partner alarm obs 101 at 10.1 s
  1  hard cascade (mimic), site 1, partner 4: site alarm obs 300 at 200 s (not a counted alarm),
     partner alarm obs 301 at 208 s
  3  plain, altered by the added edge, site 2, new downstream service 4: site alarm obs 200 at 320 s,
     alarm at 4 obs 201 at 320.2 s
  4  hard cascade, site 3, partner 0: site alarm obs 400 at 450 s, partner alarm obs 401 at 462 s
     (12 s: beyond the widest band)

plus background alarms. Five predictions with known verdicts; the expected numbers are derived by
hand in the comments.
"""

import pathlib
import tempfile
import unittest

import numpy as np
import pandas as pd

import w3_score as S

NS = 1_000_000_000


def build(root: pathlib.Path):
    graph = [(1, s, d) for s, d in [(0, ""), (1, "0"), (2, ""), (3, "2"), (4, "")]]
    pd.DataFrame(graph, columns=["seed", "service", "depends_on"]).to_csv(root / "graph.csv", index=False)
    pd.DataFrame([(1, 0, 300 * NS, "edge_add", "", "", 4, 2, 5)],
                 columns=["seed", "index", "at_ns", "kind", "kind_a", "kind_b", "dependent", "dependency", "n_services"]).to_csv(root / "regimes.csv", index=False)
    inc = [
        dict(seed=1, incident=0, tier="hard", family="cascade", mode="contradict", site=0, other=2, edge_altered=0),
        dict(seed=1, incident=1, tier="hard", family="cascade", mode="mimic", site=1, other=4, edge_altered=0),
        dict(seed=1, incident=3, tier="plain", family="known", mode="", site=2, other=np.nan, edge_altered=1),
        dict(seed=1, incident=4, tier="hard", family="cascade", mode="mimic", site=3, other=0, edge_altered=0),
    ]
    pd.DataFrame(inc).to_csv(root / "incidents.csv", index=False)
    fi = [(1, 0, 10 * NS, ""), (1, 1, 200 * NS, ""), (1, 3, 320 * NS, "4"), (1, 4, 450 * NS, "")]
    pd.DataFrame(fi, columns=["seed", "incident", "first_site_alarm_ns", "new_down_site"]).to_csv(root / "floor-incidents.csv", index=False)
    al = [  # obs, at, service, owner
        (100, 10.0, 0, 0), (101, 10.1, 2, 0),
        (300, 200.0, 1, 1), (301, 208.0, 4, 1),
        (200, 320.0, 2, 3), (201, 320.2, 4, 3),
        (400, 450.0, 3, 4), (401, 462.0, 0, 4),
        (50, 100.0, 2, -1), (51, 100.5, 4, -1),
        (60, 150.0, 2, -1), (61, 155.0, 0, -1),
        (70, 70.0, 4, -1),
        (80, 9.5, 3, -1),  # service 3 is not quiet at 10 s
    ]
    pd.DataFrame([(1, o, int(t * NS), s, w, "Decisive" if w >= 0 else "bg:X") for o, t, s, w in al],
                 columns=["seed", "obs", "at_ns", "service", "owner", "role"]).to_csv(root / "alarms.csv", index=False)


PRED_COLS = ["seed", "segment", "made_at_ns", "alarm_at_ns", "predicting_service", "predicted_service", "band", "band_ns", "alarm_obs", "anomaly", "followed"]


def preds():
    rows = [
        # P1: cascade 0 -> 2 at the root's alarm; the partner follows in 0.1 s: hit
        (1, 0, int(10.2 * NS), 10 * NS, 0, 2, 0, int(0.4 * NS), 100, 0, 1),
        # P2: 0 -> 4: nothing alarms at 4 near 10 s: not followed, no relation
        (1, 0, int(10.2 * NS), 10 * NS, 0, 4, 1, 2 * NS, 100, 0, 0),
        # P3: 2 -> 4 on incident 3's alarm after the edge: the added edge, 0.2 s lead: hit
        (1, 0, int(320.2 * NS), 320 * NS, 2, 4, 1, 2 * NS, 200, 3, 1),
        # P4: 2 -> 4 on a background alarm at 100 s (before the edge): followed by a background alarm, no relation
        (1, 0, int(100.1 * NS), 100 * NS, 2, 4, 1, 2 * NS, 50, -1, 1),
        # P5: 2 -> 0 at a background alarm at 150 s: the reverse of the cascade; followed (155 s) by a background alarm
        (1, 0, int(150.1 * NS), 150 * NS, 2, 0, 2, 10 * NS, 60, -1, 1),
    ]
    return pd.DataFrame(rows, columns=PRED_COLS)


class ScoreTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(cls.tmp.name)
        build(root)
        preds().to_csv(root / "pred.csv", index=False)
        pd.DataFrame([(1, int(t * NS), s, o, c) for t, s, o, c in [(10.0, 0, 100, 1), (200.0, 1, 300, 0), (320.0, 2, 200, 1)]],
                     columns=["seed", "at_ns", "service", "obs", "counted"]).to_csv(root / "fa.csv", index=False)
        pd.DataFrame([(1, 0, 2, 0, 10 * NS), (1, 2, 4, 1, 320 * NS)], columns=["seed", "a", "b", "band", "first_held_at_ns"]).to_csv(root / "edges.csv", index=False)
        cls.out = S.run(root / "pred.csv", root, root / "fa.csv", root / "edges.csv", "toy", perms=400)
        cls.H = S.Hidden(root)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_relations(self):
        sc = self.out["predictions_scored"]
        self.assertEqual(list(sc["relation"]), ["cascade", "none", "added_edge", "none", "cascade_rev"])
        self.assertEqual(list(sc["true_edge"]), [True, False, True, False, False])

    def test_followed_and_leads(self):
        sc = self.out["predictions_scored"]
        self.assertEqual(list(sc["followed_hidden"]), [True, False, True, True, True])
        np.testing.assert_allclose(sc["lead_from_alarm_ns"].to_numpy()[[0, 2, 3, 4]] / NS, [0.1, 0.2, 0.5, 5.0], atol=1e-6)
        # lead from the step that made the prediction: 10.1 - 10.2 = -0.1 s (the partner was delivered in the same step)
        self.assertAlmostEqual(sc["lead_from_made_ns"].iloc[0] / NS, -0.1, places=6)

    def test_event_hits(self):
        sc = self.out["predictions_scored"]
        self.assertEqual(list(sc["event_hit"]), [True, False, True, False, False])
        self.assertEqual(list(sc["predicting_owner"]), [0, 0, 3, -1, -1])

    def test_precision_rows(self):
        pr = self.out["precision"].set_index("group")
        row = pr.loc["all predictions"]
        self.assertEqual(row["predictions"], 5)
        self.assertEqual(row["on_true_edge_pair"], 2)
        self.assertAlmostEqual(float(row["precision_pair"]), 0.4)
        self.assertEqual(row["event_hits"], 2)
        self.assertEqual(row["followed_hidden"], 4)
        self.assertEqual(row["followed_public"], 4)
        self.assertEqual(pr.loc["pair relation cascade", "predictions"], 1)
        self.assertEqual(pr.loc["pair relation added_edge", "predictions"], 1)
        self.assertEqual(pr.loc["pair relation cascade_rev", "predictions"], 1)
        self.assertEqual(pr.loc["band 0.4 s", "predictions"], 1)

    def test_chance_candidates(self):
        sc = self.out["predictions_scored"]
        # P1 (a = 0): unconnected 2, 3, 4 (1 depends on 0), but 3 alarmed at 9.5 s and is not quiet: 2 candidates;
        # only 2 is a true edge pair
        self.assertEqual((sc["cand_n"].iloc[0], sc["cand_true"].iloc[0]), (2, 1))
        # P3 (a = 2 at 320 s): candidates 0, 1, 4 (3 depends on 2); the true edge pair is (2, 4)
        self.assertEqual((sc["cand_n"].iloc[2], sc["cand_true"].iloc[2]), (3, 1))
        # followed: P1 window (10, 10.4]: only service 2 alarms: 1 of 2
        self.assertEqual(sc["cand_followed"].iloc[0], 1)
        ps = self.out["perm_summary"].iloc[0]
        mean_expected = sc["chance_true_edge"].mean()
        self.assertAlmostEqual(float(ps["uniform_true_edge_precision_mean"]), mean_expected, delta=0.06)

    def test_coverage_funnel(self):
        cv = self.out["coverage_events"].set_index("incident")
        self.assertEqual(set(cv.index), {0, 1, 3, 4})
        self.assertAlmostEqual(cv.loc[0, "lead_true_s"], 0.1, places=6)
        self.assertTrue(cv.loc[0, "covered"] and cv.loc[3, "covered"])
        self.assertFalse(cv.loc[1, "covered"] or cv.loc[4, "covered"])
        self.assertFalse(cv.loc[4, "coverable_by_a_band"])
        self.assertEqual(list(cv.loc[[0, 1, 3, 4], "site_alarm_status"]), ["counted", "explained by an upstream burst", "counted", "not a first alarm"])
        self.assertTrue(cv.loc[1, "coverable_by_a_band"] and not cv.loc[1, "trial_openable_counted_alarm"])
        row = self.out["coverage"].set_index("group").loc["all true partner alarms"]
        self.assertEqual((row["true_partner_alarms"], row["lead_within_10s"], row["plus_predicting_alarm_counted"],
                          row["plus_partner_quiet_trial_licensed"], row["covered"]), (4, 3, 2, 2, 2))
        self.assertAlmostEqual(float(row["coverage_of_all"]), 0.5)
        self.assertAlmostEqual(float(row["coverage_of_trial_openable"]), 1.0)
        self.assertEqual((row["lost_site_alarm_explained_by_an_upstream_burst"], row["lost_site_alarm_not_a_first_alarm"]), (1, 0))

    def test_edges(self):
        e = self.out["edges"].set_index("group").loc["all learned edges (seed, a, b, band)"]
        self.assertEqual((e["edges"], e["on_true_edge_pair"]), (2, 2))

    def test_wrong_alarm_obs_is_refused(self):
        bad = preds().copy()
        bad.loc[0, "alarm_obs"] = 101  # an alarm at service 2, not the predicting service 0
        with self.assertRaises(AssertionError):
            S.score_predictions(bad, self.H)


if __name__ == "__main__":
    unittest.main(verbosity=2)
