#!/usr/bin/env python3
"""Tests of the memory simulator and the key builders of `w3_floor.py` on hand-made cases.

Run: python -B w3_floor_test.py      (from this directory)
"""

import unittest
from types import SimpleNamespace as NS_

import pandas as pd

import w3_floor as F


def inc(rows):
    df = pd.DataFrame(rows, columns=["seed", "incident", "tier", "tc", "site", "is_rec"])
    return df


def run(df, keys, bind, look, form="family", carried=False, teacher=("hard",)):
    seeds = sorted(df["seed"].unique())
    return F.simulate(df, seeds, keys, bind, look, form, carried, teacher).set_index(["seed", "incident"])


class SimTest(unittest.TestCase):
    def setUp(self):
        # stream 1: incident 0 hard (family A), 1 plain, 2 hard (family A), 3 decoy; stream 2: incident 0 plain
        self.df = inc([(1, 0, "hard", "hard:A", 5, False), (1, 1, "plain", "plain:x", 5, False), (1, 2, "hard", "hard:A", 5, True),
                       (1, 3, "decoy", "none", 6, False), (2, 0, "plain", "plain:x", 5, False)])
        self.keys = {(1, 0): "k", (1, 1): "k", (1, 2): "k", (1, 3): "k", (2, 0): "k"}

    def test_arrival_order_most_recent_earlier_source(self):
        look = {k: k[1] for k in self.keys}
        bind = {k: k[1] + 0.5 for k in self.keys}
        r = run(self.df, self.keys, bind, look)
        self.assertEqual(list(r["recalled"]), [False, True, True, True, False])
        # the plain incident recalls hard:A: wrong; the second hard incident recalls it: right; the decoy recalls it: wrong
        self.assertEqual(list(r["right"]), [False, False, True, False, False])

    def test_an_incident_does_not_recall_its_own_answer(self):
        # answers arrive at 19 s, lookups at 20 s: incident 0 would find its own answer
        look = {k: 20.0 + 100 * k[1] for k in self.keys}
        bind = {k: 19.0 + 100 * k[1] for k in self.keys}
        r = run(self.df, self.keys, bind, look)
        self.assertFalse(r.loc[(1, 0), "recalled"])
        self.assertTrue(r.loc[(1, 2), "recalled"] and r.loc[(1, 2), "right"])

    def test_a_source_is_available_only_after_its_answer(self):
        look = {(1, 0): 10.0, (1, 1): 15.0, (1, 2): 40.0, (1, 3): 41.0, (2, 0): 5.0}
        bind = {(1, 0): 19.0, (1, 1): 100.0, (1, 2): 100.0, (1, 3): 100.0, (2, 0): 100.0}
        r = run(self.df, self.keys, bind, look)
        self.assertFalse(r.loc[(1, 1), "recalled"])      # looked up at 15 s, source answers at 19 s
        self.assertTrue(r.loc[(1, 2), "recalled"])       # 40 s
        self.assertTrue(r.loc[(1, 3), "recalled"])

    def test_teacher(self):
        look = {k: k[1] for k in self.keys}
        bind = {k: k[1] + 0.5 for k in self.keys}
        r = run(self.df, self.keys, bind, look, teacher=("hard", "decoy"))
        # with the decoy as a teacher the next stream's carried memory would hold "none"; within the stream nothing after incident 3
        r2 = run(self.df, self.keys, bind, look, carried=True, teacher=("hard", "decoy"))
        self.assertFalse(r.loc[(2, 0), "recalled"])
        self.assertTrue(r2.loc[(2, 0), "recalled"])
        self.assertEqual(r2.loc[(2, 0), "tc"], "plain:x")
        self.assertFalse(r2.loc[(2, 0), "right"])         # the carried answer is the decoy's: "none"

    def test_reset_and_carry(self):
        look = {k: k[1] for k in self.keys}
        bind = {k: k[1] + 0.5 for k in self.keys}
        reset = run(self.df, self.keys, bind, look, carried=False)
        carry = run(self.df, self.keys, bind, look, carried=True)
        self.assertFalse(reset.loc[(2, 0), "recalled"])
        self.assertTrue(carry.loc[(2, 0), "recalled"])
        # the carried answer is the last hard answer (incident 2 of stream 1): hard:A, wrong for a plain incident
        self.assertFalse(carry.loc[(2, 0), "right"])

    def test_site_form_separates_sites(self):
        look = {k: k[1] for k in self.keys}
        bind = {k: k[1] + 0.5 for k in self.keys}
        r = run(self.df, self.keys, bind, look, form="site")
        # the decoy (incident 3) is at site 6, the hard sources at site 5: no recall
        self.assertFalse(r.loc[(1, 3), "recalled"])
        self.assertTrue(r.loc[(1, 1), "recalled"])

    def test_vote_needs_a_leader_above_tau(self):
        # stream 1 has hard sources of two classes at one key; incident 4 looks the key up
        df = inc([(1, 0, "hard", "hard:A", 5, False), (1, 1, "hard", "hard:A", 5, False), (1, 2, "hard", "hard:B", 5, False),
                  (1, 3, "plain", "plain:x", 5, False), (1, 4, "hard", "hard:A", 5, False)])
        keys = {(1, i): "k" for i in range(5)}
        look = {k: 10.0 + k[1] for k in keys}
        bind = {k: float(k[1]) for k in keys}
        # incident 3 looks up at 13 s: sources 0, 1, 2 (A, A, B): A leads with 2/3
        last = F.simulate(df, [1], keys, bind, look, "family", False, ("hard",), None).set_index("incident")
        self.assertEqual((last.loc[3, "recalled"], last.loc[3, "right"]), (True, False))      # the last answer is B
        v = F.simulate(df, [1], keys, bind, look, "family", False, ("hard",), 0.5).set_index("incident")
        self.assertTrue(v.loc[4, "right"])                    # incident 4 recalls A (3 answers: A, A, B; its own is excluded): 2/3 > 0.5
        v9 = F.simulate(df, [1], keys, bind, look, "family", False, ("hard",), 0.8).set_index("incident")
        self.assertFalse(v9.loc[4, "recalled"])               # 2/3 is not above 0.8
        self.assertFalse(v9.loc[0, "recalled"])               # the first looked-up incident has no earlier answer by another incident
        # a tie recalls nothing
        df2 = inc([(1, 0, "hard", "hard:A", 5, False), (1, 1, "hard", "hard:B", 5, False), (1, 2, "plain", "plain:x", 5, False)])
        keys2 = {(1, i): "k" for i in range(3)}
        look2 = {k: 10.0 + k[1] for k in keys2}
        bind2 = {k: float(k[1]) for k in keys2}
        t2 = F.simulate(df2, [1], keys2, bind2, look2, "family", False, ("hard",), 0.4).set_index("incident")
        self.assertFalse(t2.loc[2, "recalled"])

    def test_vote_carries_all_answers_across_streams(self):
        df = inc([(1, 0, "hard", "hard:A", 5, False), (1, 1, "hard", "hard:A", 5, False), (2, 0, "hard", "hard:B", 5, False), (2, 1, "plain", "plain:x", 5, False)])
        keys = {(s, i): "k" for s in (1, 2) for i in (0, 1)}
        look = {k: 10.0 + k[1] for k in keys}
        bind = {k: float(k[1]) for k in keys}
        r = F.simulate(df, [1, 2], keys, bind, look, "family", True, ("hard",), 0.5).set_index(["seed", "incident"])
        # stream 2, incident 1: held A, A (carried) and B (this stream, bound at 0 s): leader A with 2/3
        self.assertTrue(r.loc[(2, 1), "recalled"])
        self.assertFalse(r.loc[(2, 1), "right"])
        # stream 2 incident 0 (B) recalls A: the leader of the two carried answers (its own is excluded)
        self.assertEqual((r.loc[(2, 0), "recalled"], r.loc[(2, 0), "right"]), (True, False))

    def test_gate_band_and_bands(self):
        self.assertEqual([F.gate_band(x * F.NS) for x in (0.5, 1, 2.9, 3, 5.9, 6, 9.9, 10, 30)], [0, 1, 1, 2, 2, 3, 3, 4, 4])

    def test_e1_key_levels(self):
        r = NS_(site_tags="T", anchor="A", bands="E:1", gate_delay_ns=2.0 * F.NS)
        self.assertEqual(F.e1_key(r, "kinds"), "T|A")
        self.assertEqual(F.e1_key(r, "bands"), "T|A|E:1")
        self.assertEqual(F.e1_key(r, "timing"), "T|A|E:1|g1")

    def test_ladder_levels_are_nested(self):
        r = NS_(site_tags="T", anchor="A", oth_tags="O", oth_n=1, verdict_all="C", bands="E:1", partner="after:0:P", streak=5)
        k = [F.ladder_key(r, lv) for lv in F.LEVELS]
        for a, b in zip(k, k[1:]):
            self.assertTrue(b.startswith(a) and len(b) > len(a))
        self.assertTrue(k[3].endswith("|1"))
        r.streak = 4
        self.assertTrue(F.ladder_key(r, "K4").endswith("|0"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
