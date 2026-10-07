#!/usr/bin/env python3
"""W3 item 4 (hidden side): score A2's predictions against the hidden incidents and graph.

Usage:
    w3_score.py --predictions PRED.csv [--first-alarms FA.csv] [--edges EDGES.csv] --hidden DIR
                [--arm NAME] [--out-dir DIR] [--permutations N]

`PRED.csv` is a `predictions-<arm>.csv` of A2 (Lab 1): seed, segment, made_at_ns, alarm_at_ns,
predicting_service, predicted_service, band, band_ns, alarm_obs, anomaly, followed, ... One row per
prediction: "an alarm at the predicting service has just begun; an alarm at the predicted service
will follow within `band_ns`". `FA.csv` is the arm's `first-alarms-<arm>.csv` (seed, at_ns, service,
obs, counted: the first alarms the learner counted, the only ones it opens a trial on); `EDGES.csv`
its `edges-<arm>.csv` (seed, a, b, band, ...). `--hidden DIR` holds what `laws --alarms` wrote for the
same seeds (`alarms.csv`, `graph.csv`, `incidents.csv`, `regimes.csv`) plus `floor-incidents.csv`.
Side: **hidden** (every verdict) joined to **public** (what the predictions are). Nothing here
reaches an arm.

What is scored, per prediction `(seed, a -> b, alarm at t_a, window w)`:

- *followed (hidden)*: an abnormal observation at `b` in `(t_a, t_a + w]`, from the stream's own
  alarms (any owner), its lead time, and whose it is (the predicting alarm's incident, another
  incident, background);
- *relation of the pair (a, b)*: the hidden relation that holds between the two services at `t_a`:
  `cascade` (a is the root and b the partner of a cascade incident, hard or decoy, of this stream),
  `cascade_rev` (the reverse), `peer` (the two are a split brain's site and peer), `added_edge`
  (b depends on a in the true graph at `t_a` and did not in the public one), `added_edge_rev`, or
  `none`. *True hidden edge* is `cascade` or `added_edge`, the brief's "cascade partner or added
  edge", in the direction predicted;
- *event hit*: the predicting alarm belongs to an incident for which `a -> b` is a true partner
  relation (cascade partner; a service newly downstream of the site after the added edge) and the
  alarm at `b` that follows within the window belongs to the same incident.

Per true partner alarm (one per cascade incident: the partner's first alarm; one per incident
altered by the added edge: the first alarm at a service newly downstream of its site), a funnel:
all; its lead from the site's first alarm is within the widest band (10 s); the predicting alarm at
the site is one the learner counted (a trial is opened only on those: the explanation filter);
the partner was quiet (no abnormal observation in the previous 2 s: the trial is licensed); an
edge for the pair was held (a prediction was made on this very alarm, naming this partner, with the
alarm inside its window): *covered*. The chief asked for coverage over all true partner alarms and
over those a trial could have been opened on; both are the first and the third funnel rows.

Chance: for each prediction, the share of the services it could have named (the services the
public graph does not connect to `a`, either way, that were quiet, with no abnormal observation in
the previous 2 s: the only partners a trial is licensed for) that would have scored the same way;
the expected count, and a Monte-Carlo permutation distribution of the totals (a uniform draw of the
predicted service among those candidates for every prediction). A shuffle of the predicted
services among the predictions of a stream was tried and dropped: it also assigns services the
public graph connects to `a`, which no prediction names, and so understates the chance.
"""

from __future__ import annotations

import argparse
import bisect
import pathlib
from collections import defaultdict

import numpy as np
import pandas as pd

NS = 1_000_000_000
BOOT_SEED = 9_950
WIDEST_BAND_NS = 10 * NS
QUIET_NS = 2 * NS


# ---- the hidden side ------------------------------------------------------------------------


class Hidden:
    """The streams' hidden facts, indexed for the joins."""

    def __init__(self, root: pathlib.Path):
        self.alarms = pd.read_csv(root / "alarms.csv", dtype={"role": str})
        self.graph = pd.read_csv(root / "graph.csv", keep_default_na=False)
        self.inc = pd.read_csv(root / "incidents.csv", keep_default_na=False, na_values=[""])
        self.reg = pd.read_csv(root / "regimes.csv", keep_default_na=False, na_values=[""])
        fi = pd.read_csv(root / "floor-incidents.csv", keep_default_na=False, na_values=[""], dtype={"new_down_site": str})
        self.inc = self.inc.merge(fi[["seed", "incident", "first_site_alarm_ns", "new_down_site"]], on=["seed", "incident"], how="left")
        # alarms by (seed, service): sorted times and owners; by (seed, obs): the row.
        self.times: dict = {}
        self.owner_at: dict = {}
        self.by_obs: dict = {}
        for (seed, svc), g in self.alarms.groupby(["seed", "service"]):
            g = g.sort_values(["at_ns", "obs"])
            self.times[(seed, svc)] = g["at_ns"].to_numpy()
            self.owner_at[(seed, svc)] = g["owner"].to_numpy()
        for r in self.alarms.itertuples():
            self.by_obs[(r.seed, r.obs)] = (r.at_ns, r.service, r.owner)
        # graph at time zero: depends_on lists.
        self.deps0: dict = defaultdict(dict)
        for r in self.graph.itertuples():
            self.deps0[r.seed][r.service] = [int(x) for x in str(r.depends_on).split("|") if x != ""]
        self.edges_added: dict = defaultdict(list)
        for r in self.reg[self.reg["kind"] == "edge_add"].itertuples():
            self.edges_added[r.seed].append((int(r.at_ns), int(r.dependent), int(r.dependency)))
        self._down0: dict = {}
        self._down_t: dict = {}
        self.cascades: dict = defaultdict(set)  # seed -> {(root, partner)}
        self.peers: dict = defaultdict(set)
        for r in self.inc.itertuples():
            if r.family == "cascade" and r.tier in ("hard", "decoy") and pd.notna(r.other):
                self.cascades[r.seed].add((int(r.site), int(r.other)))
            if r.family == "split_brain" and r.tier in ("hard", "decoy") and pd.notna(r.other):
                self.peers[r.seed].add(frozenset((int(r.site), int(r.other))))

    # reachability: dependents_of(seed, x, graph) = services that transitively depend on x
    def _dependents(self, seed: int, x: int, extra: tuple) -> frozenset:
        key = (seed, x, extra)
        if key in self._down_t:
            return self._down_t[key]
        deps = {s: list(v) for s, v in self.deps0[seed].items()}
        for (_, d, u) in extra:
            deps[d] = sorted(set(deps[d]) | {u})
        member = {x}
        changed = True
        while changed:
            changed = False
            for s, ds in deps.items():
                if s not in member and any(d in member for d in ds):
                    member.add(s)
                    changed = True
        out = frozenset(member - {x})
        self._down_t[key] = out
        return out

    def downstream0(self, seed: int, x: int) -> frozenset:
        return self._dependents(seed, x, ())

    def downstream_at(self, seed: int, x: int, t: int) -> frozenset:
        extra = tuple(e for e in self.edges_added[seed] if e[0] <= t)
        return self._dependents(seed, x, extra)

    def services(self, seed: int) -> list[int]:
        return sorted(self.deps0[seed])

    def unconnected(self, seed: int, a: int) -> list[int]:
        """The services the time-zero public graph connects to `a` in neither direction."""
        return [b for b in self.services(seed) if b != a and b not in self.downstream0(seed, a) and a not in self.downstream0(seed, b)]

    def candidates(self, seed: int, a: int, t: int) -> list[int]:
        """The services A2 could have named at an alarm at `a` at `t`: unconnected to `a` in the public
        graph and quiet (no abnormal observation in the previous 2 s: a trial is licensed only for those)."""
        return [b for b in self.unconnected(seed, a) if not self.any_in(seed, b, t - QUIET_NS, t)]

    def relation(self, seed: int, a: int, b: int, t: int) -> str:
        if (a, b) in self.cascades[seed]:
            return "cascade"
        if b in self.downstream_at(seed, a, t) and b not in self.downstream0(seed, a):
            return "added_edge"
        if (b, a) in self.cascades[seed]:
            return "cascade_rev"
        if a in self.downstream_at(seed, b, t) and a not in self.downstream0(seed, b):
            return "added_edge_rev"
        if frozenset((a, b)) in self.peers[seed]:
            return "peer"
        return "none"

    def first_after(self, seed: int, svc: int, t0: int, t1: int):
        """The first abnormal observation at `svc` in (t0, t1]: (at_ns, owner) or None."""
        ts = self.times.get((seed, svc))
        if ts is None:
            return None
        i = bisect.bisect_right(ts, t0)
        if i < len(ts) and ts[i] <= t1:
            return int(ts[i]), int(self.owner_at[(seed, svc)][i])
        return None

    def any_in(self, seed: int, svc: int, t0: int, t1: int) -> bool:
        """Any abnormal observation at `svc` in [t0, t1]."""
        ts = self.times.get((seed, svc))
        if ts is None:
            return False
        i = bisect.bisect_left(ts, t0)
        return i < len(ts) and ts[i] <= t1


TRUE_EDGE = ("cascade", "added_edge")


# ---- scoring predictions --------------------------------------------------------------------


def score_predictions(pred: pd.DataFrame, H: Hidden) -> pd.DataFrame:
    rows = []
    for p in pred.itertuples():
        seed, a, b, ta, w = int(p.seed), int(p.predicting_service), int(p.predicted_service), int(p.alarm_at_ns), int(p.band_ns)
        rel = H.relation(seed, a, b, ta)
        fol = H.first_after(seed, b, ta, ta + w)
        own = H.by_obs.get((seed, int(p.alarm_obs)))
        pred_owner = int(own[2]) if own is not None else -2
        if own is not None:
            assert int(own[1]) == a and int(own[0]) == ta, f"seed {seed}: alarm_obs {p.alarm_obs} is not the predicting alarm"
        row = {"seed": seed, "a": a, "b": b, "band_ns": w, "alarm_at_ns": ta, "made_at_ns": int(p.made_at_ns), "relation": rel,
               "true_edge": rel in TRUE_EDGE, "followed_hidden": fol is not None,
               "lead_from_alarm_ns": (fol[0] - ta) if fol else np.nan, "lead_from_made_ns": (fol[0] - int(p.made_at_ns)) if fol else np.nan,
               "follow_owner": fol[1] if fol else -9, "predicting_owner": pred_owner,
               "followed_public": int(p.followed) if hasattr(p, "followed") else -1}
        # event hit: the predicting alarm's incident has a -> b as a true partner relation and the follow is its own.
        hit = False
        if pred_owner >= 0 and fol is not None and fol[1] == pred_owner:
            hit = rel in TRUE_EDGE and partner_pair_of_incident(H, seed, pred_owner, a, b)
        row["event_hit"] = hit
        rows.append(row)
    return pd.DataFrame(rows)


def partner_pair_of_incident(H: Hidden, seed: int, inc_id: int, a: int, b: int) -> bool:
    r = H.inc[(H.inc["seed"] == seed) & (H.inc["incident"] == inc_id)]
    if r.empty:
        return False
    r = r.iloc[0]
    if int(r["site"]) != a:
        return False
    if r["family"] == "cascade" and pd.notna(r["other"]) and int(r["other"]) == b:
        return True
    if r["edge_altered"] == 1 and isinstance(r["new_down_site"], str) and str(b) in r["new_down_site"].split("|"):
        return True
    return False


def boot_ci(x: np.ndarray, den: np.ndarray | None = None, groups: np.ndarray | None = None, n=10_000):
    """90% percentile interval of sum(x)/sum(den), resampling clusters (streams) when `groups` is given."""
    x = np.asarray(x, float)
    den = np.ones_like(x) if den is None else np.asarray(den, float)
    if len(x) == 0 or den.sum() == 0:
        return float("nan"), float("nan"), float("nan")
    point = x.sum() / den.sum()
    if groups is None:
        groups = np.arange(len(x))
    ug, inv = np.unique(groups, return_inverse=True)
    sx = np.bincount(inv, weights=x, minlength=len(ug))
    sd = np.bincount(inv, weights=den, minlength=len(ug))
    rng = np.random.default_rng(BOOT_SEED)
    idx = rng.integers(0, len(ug), size=(n, len(ug)))
    d = sd[idx].sum(axis=1)
    with np.errstate(invalid="ignore", divide="ignore"):
        r = np.where(d > 0, sx[idx].sum(axis=1) / np.where(d > 0, d, 1), np.nan)
    lo, hi = np.nanquantile(r, [0.05, 0.95])
    return float(point), float(lo), float(hi)


def chance_tables(sc: pd.DataFrame, H: Hidden, perms: int, seed_rng: int = BOOT_SEED) -> tuple[pd.DataFrame, dict]:
    """The chance level of the verdicts under a uniform draw of the predicted service among the services
    A2 could have named at that alarm (`Hidden.candidates`): exactly (expected shares per prediction) and by
    Monte Carlo (the distribution of the totals over `perms` draws of the whole set of predictions)."""
    cand_true, cand_fol, cand_n = [], [], []
    for p in sc.itertuples():
        cands = H.candidates(p.seed, p.a, p.alarm_at_ns)
        cand_n.append(len(cands))
        cand_true.append(sum(H.relation(p.seed, p.a, c, p.alarm_at_ns) in TRUE_EDGE for c in cands))
        cand_fol.append(sum(H.any_in(p.seed, c, p.alarm_at_ns + 1, p.alarm_at_ns + p.band_ns) for c in cands))
    sc = sc.copy()
    sc["cand_n"], sc["cand_true"], sc["cand_followed"] = cand_n, cand_true, cand_fol
    sc["chance_true_edge"] = sc["cand_true"] / sc["cand_n"].clip(lower=1)
    sc["chance_followed"] = sc["cand_followed"] / sc["cand_n"].clip(lower=1)
    rng = np.random.default_rng(seed_rng)
    tot_true = np.zeros(perms)
    tot_fol = np.zeros(perms)
    for p in sc.itertuples():
        cands = H.candidates(p.seed, p.a, p.alarm_at_ns)
        if not cands:
            continue
        draw = rng.integers(0, len(cands), size=perms)
        t_flag = np.array([H.relation(p.seed, p.a, c, p.alarm_at_ns) in TRUE_EDGE for c in cands])
        f_flag = np.array([H.any_in(p.seed, c, p.alarm_at_ns + 1, p.alarm_at_ns + p.band_ns) for c in cands])
        tot_true += t_flag[draw]
        tot_fol += f_flag[draw]
    return sc, {"n": len(sc), "mc_true": tot_true, "mc_fol": tot_fol}


def pct(x, nd=3):
    return "" if x is None or (isinstance(x, float) and not np.isfinite(x)) else f"{x:.{nd}f}"


def precision_rows(sc: pd.DataFrame, mc: dict, label: str, seeds_label: str) -> list[dict]:
    rows = []

    def one(group: str, m: pd.Series, with_mc: bool):
        d = sc[m]
        if d.empty:
            return
        n = len(d)
        te, tlo, thi = boot_ci(d["true_edge"].to_numpy(float), None, d["seed"].to_numpy())
        fp, flo, fhi = boot_ci(d["followed_hidden"].to_numpy(float), None, d["seed"].to_numpy())
        eh = d["event_hit"].sum()
        row = {"arm": label, "seeds": seeds_label, "side": "hidden joined to public", "group": group, "predictions": n,
               "on_true_edge_pair": int(d["true_edge"].sum()), "precision_pair": pct(te), "pair_lo90": pct(tlo), "pair_hi90": pct(thi),
               "chance_pair_expected": "" if group.startswith("pair relation") else pct(d["chance_true_edge"].mean()),
               "event_hits": int(eh), "event_hit_share": pct(eh / n),
               "followed_hidden": int(d["followed_hidden"].sum()), "followed_share": pct(fp), "followed_lo90": pct(flo), "followed_hi90": pct(fhi),
               "followed_public": int((d["followed_public"] == 1).sum()),
               "chance_followed_expected": "" if group.startswith("pair relation") else pct(d["chance_followed"].mean())}
        lf = d.loc[d["followed_hidden"], "lead_from_alarm_ns"] / NS
        lm = d.loc[d["followed_hidden"], "lead_from_made_ns"] / NS
        row.update({"lead_from_alarm_median_s": pct(lf.median()) if len(lf) else "", "lead_from_made_median_s": pct(lm.median()) if len(lm) else ""})
        if with_mc:
            pt = (mc["mc_true"] >= d["true_edge"].sum()).mean() if n == mc["n"] else ""
            row["permutation_p_true_edge_uniform"] = pct(pt) if pt != "" else ""
            row["mc_true_edge_mean"] = pct(mc["mc_true"].mean() / mc["n"], 4) if n == mc["n"] else ""
        rows.append(row)

    one("all predictions", sc["seed"].notna(), True)
    for band in sorted(sc["band_ns"].unique()):
        one(f"band {band / NS:g} s", sc["band_ns"] == band, False)
    for rel in ["cascade", "added_edge", "cascade_rev", "added_edge_rev", "peer", "none"]:
        one(f"pair relation {rel}", sc["relation"] == rel, False)
    seeds = sorted(sc["seed"].unique())
    half = len(seeds) // 2
    one(f"first {half} streams", sc["seed"].isin(seeds[:half]), False)
    one(f"last {len(seeds) - half} streams", sc["seed"].isin(seeds[half:]), False)
    for lo, hi in ((0, 200), (200, 400), (400, 601)):
        one(f"alarm at {lo}-{hi} s", (sc["alarm_at_ns"] >= lo * NS) & (sc["alarm_at_ns"] < hi * NS), False)
    return rows


# ---- coverage of true partner alarms --------------------------------------------------------


def true_partner_events(H: Hidden) -> pd.DataFrame:
    """One row per true partner alarm: cascade incidents (hard and decoy: a decoy's phase-1 partner
    alarms are the same draws), and incidents altered by the added edge."""
    ev = []
    for r in H.inc.itertuples():
        seed, i, a = int(r.seed), int(r.incident), int(r.site)
        ta = r.first_site_alarm_ns
        if pd.isna(ta):
            continue
        ta = int(ta)
        targets: list[int] = []
        kind = None
        if r.family == "cascade" and pd.notna(r.other) and r.tier in ("hard", "decoy"):
            targets, kind = [int(r.other)], f"cascade {r.tier} {r.mode}"
        elif r.edge_altered == 1 and isinstance(r.new_down_site, str) and r.new_down_site:
            targets, kind = [int(x) for x in r.new_down_site.split("|")], f"added_edge {r.tier}"
        if not targets:
            continue
        # the incident's own first alarm at any target
        best = None
        for b in targets:
            ts = H.times.get((seed, b))
            if ts is None:
                continue
            ow = H.owner_at[(seed, b)]
            for t, o in zip(ts, ow):
                if o == i and t >= ta - 1:
                    if best is None or t < best[0]:
                        best = (int(t), b)
                    break
        if best is None:
            continue
        ev.append({"seed": seed, "incident": i, "kind": kind, "a": a, "b": best[1], "targets": targets, "t_a": ta, "t_b": best[0],
                   "lead_true_ns": best[0] - ta, "tier": r.tier})
    return pd.DataFrame(ev)


def coverage(ev: pd.DataFrame, pred: pd.DataFrame, sc: pd.DataFrame, fa: pd.DataFrame | None, edges: pd.DataFrame | None,
             H: Hidden) -> pd.DataFrame:
    counted = {}
    if fa is not None:
        for r in fa.itertuples():
            counted[(int(r.seed), int(r.obs))] = int(r.counted)
    held_pair = set()
    if edges is not None:
        held_pair = {(int(r.seed), int(r.a), int(r.b)) for r in edges.itertuples()}
    preds_by = defaultdict(list)
    for p, s in zip(pred.itertuples(), sc.itertuples()):
        preds_by[(int(p.seed), int(p.predicting_service))].append((p, s))
    rows = []
    for e in ev.itertuples():
        seed, a, tgt = e.seed, e.a, set(e.targets)
        # funnel
        coverable = e.lead_true_ns <= WIDEST_BAND_NS
        # the incident's alarms at the predicting service that the learner counted, before the partner's alarm
        alarms_a = H.alarms[(H.alarms["seed"] == seed) & (H.alarms["service"] == a) & (H.alarms["owner"] == e.incident)
                            & (H.alarms["at_ns"] < e.t_b)]
        counted_here = [int(o) for o in alarms_a["obs"] if counted.get((seed, int(o)), 0) == 1] if fa is not None else None
        trial_open = (len(counted_here) > 0) if fa is not None else None
        status = ""
        if fa is not None:
            rows_fa = [counted[(seed, int(o))] for o in alarms_a["obs"] if (seed, int(o)) in counted]
            status = "counted" if counted_here else ("explained by an upstream burst" if rows_fa else "not a first alarm")
        quiet = None
        if trial_open:
            ts = [int(alarms_a.loc[alarms_a["obs"] == o, "at_ns"].iloc[0]) for o in counted_here]
            quiet = any(not H.any_in(seed, e.b, t - QUIET_NS, t) for t in ts)
        # covered: a prediction on one of the incident's alarms at a, naming a target, whose window holds the target's alarm
        covered, lead_pred = False, np.nan
        for p, s in preds_by.get((seed, a), []):
            if int(p.predicted_service) in tgt and s.predicting_owner == e.incident:
                tb = H.first_after(seed, int(p.predicted_service), int(p.alarm_at_ns), int(p.alarm_at_ns) + int(p.band_ns))
                if tb is not None and tb[1] == e.incident:
                    covered = True
                    lead_pred = (tb[0] - int(p.alarm_at_ns)) / NS if np.isnan(lead_pred) else min(lead_pred, (tb[0] - int(p.alarm_at_ns)) / NS)
        # looser: any prediction (a -> a target) whose window holds the partner's alarm, whoever's alarm it followed
        loose = any(int(p.predicted_service) in tgt and int(p.alarm_at_ns) < e.t_b <= int(p.alarm_at_ns) + int(p.band_ns)
                    for p, _ in preds_by.get((seed, a), []))
        held_ever = any((seed, a, t) in held_pair for t in tgt) if edges is not None else None
        rows.append({"seed": e.seed, "incident": e.incident, "kind": e.kind, "a": a, "b": e.b, "lead_true_s": e.lead_true_ns / NS,
                     "coverable_by_a_band": coverable, "trial_openable_counted_alarm": trial_open, "site_alarm_status": status, "partner_quiet": quiet,
                     "edge_held_ever_in_stream": held_ever, "covered": covered, "covered_loose": loose,
                     "lead_pred_s": lead_pred, "t_a": e.t_a})
    return pd.DataFrame(rows)


def coverage_rows(cv: pd.DataFrame, label: str, seeds_label: str) -> list[dict]:
    rows = []
    groups = [("all true partner alarms", cv["seed"].notna())]
    for k in sorted(cv["kind"].unique()):
        groups.append((k, cv["kind"] == k))
    groups.append(("hard cascades and added-edge (no decoys)", ~cv["kind"].str.contains("decoy")))
    for name, m in groups:
        d = cv[m]
        if d.empty:
            continue
        n = len(d)
        f1 = d[d["coverable_by_a_band"]]
        f2 = f1[f1["trial_openable_counted_alarm"] == True]  # noqa: E712
        f3 = f2[f2["partner_quiet"] == True]  # noqa: E712
        have_fa = d["trial_openable_counted_alarm"].notna().any()
        rows.append({"arm": label, "seeds": seeds_label, "side": "hidden joined to public", "group": name,
                     "true_partner_alarms": n,
                     "lead_within_10s": len(f1),
                     "plus_predicting_alarm_counted": len(f2) if have_fa else "",
                     "lost_site_alarm_explained_by_an_upstream_burst": int((f1["site_alarm_status"] == "explained by an upstream burst").sum()) if have_fa else "",
                     "lost_site_alarm_not_a_first_alarm": int((f1["site_alarm_status"] == "not a first alarm").sum()) if have_fa else "",
                     "plus_partner_quiet_trial_licensed": len(f3) if have_fa else "",
                     "edge_held_at_some_read_of_the_stream": int((d["edge_held_ever_in_stream"] == True).sum()) if d["edge_held_ever_in_stream"].notna().any() else "",  # noqa: E712
                     "covered": int(d["covered"].sum()), "covered_loose": int(d["covered_loose"].sum()),
                     "coverage_of_all": pct(d["covered"].mean()),
                     "coverage_of_lead_within_10s": pct(f1["covered"].mean()) if len(f1) else "",
                     "coverage_of_trial_openable": pct(f2["covered"].mean()) if (have_fa and len(f2)) else "",
                     "coverage_of_trial_licensed": pct(f3["covered"].mean()) if (have_fa and len(f3)) else "",
                     "median_lead_true_s": pct(d["lead_true_s"].median()),
                     "median_lead_of_covered_s": pct(d.loc[d["covered"], "lead_pred_s"].median()) if d["covered"].any() else ""})
    return rows


def edge_rows(edges: pd.DataFrame, H: Hidden, label: str, seeds_label: str) -> list[dict]:
    """Precision of the learned edges themselves (distinct (seed, a, b) held at any read)."""
    d = edges[["seed", "a", "b", "band", "first_held_at_ns"]].copy()
    d["relation"] = [H.relation(int(r.seed), int(r.a), int(r.b), int(r.first_held_at_ns)) for r in d.itertuples()]
    d["true_edge"] = d["relation"].isin(TRUE_EDGE)
    d["chance"] = [np.mean([H.relation(int(r.seed), int(r.a), c, int(r.first_held_at_ns)) in TRUE_EDGE for c in H.unconnected(int(r.seed), int(r.a))] or [0])
                   for r in d.itertuples()]
    rows = []
    for name, m in [("all learned edges (seed, a, b, band)", d["seed"].notna())] + [(f"band index {b}", d["band"] == b) for b in sorted(d["band"].unique())]:
        x = d[m]
        p, lo, hi = boot_ci(x["true_edge"].to_numpy(float), None, x["seed"].to_numpy())
        rows.append({"arm": label, "seeds": seeds_label, "side": "hidden joined to public", "group": name, "edges": len(x),
                     "on_true_edge_pair": int(x["true_edge"].sum()), "precision": pct(p), "lo90": pct(lo), "hi90": pct(hi),
                     "chance_expected": pct(x["chance"].mean()),
                     **{f"relation_{k}": int((x["relation"] == k).sum()) for k in ["cascade", "added_edge", "cascade_rev", "added_edge_rev", "peer", "none"]}})
    return rows


# ---- driver ---------------------------------------------------------------------------------


def run(pred_path, hidden, fa_path=None, edges_path=None, arm="a2", perms=2000):
    H = Hidden(pathlib.Path(hidden))
    pred = pd.read_csv(pred_path)
    seeds_label = f"{int(pred['seed'].min())}-{int(pred['seed'].max())}"
    sc = score_predictions(pred, H)
    sc, mc = chance_tables(sc, H, perms)
    fa = pd.read_csv(fa_path) if fa_path else None
    edges = pd.read_csv(edges_path) if edges_path else None
    ev = true_partner_events(H)
    ev = ev[ev["seed"].isin(H.alarms["seed"].unique())]  # every stream of the smoke, whether or not the arm predicted in it
    cv = coverage(ev, pred, sc, fa, edges, H)
    out = {"predictions_scored": sc, "precision": pd.DataFrame(precision_rows(sc, mc, arm, seeds_label)),
           "coverage": pd.DataFrame(coverage_rows(cv, arm, seeds_label)), "coverage_events": cv, "mc": mc}
    if edges is not None:
        out["edges"] = pd.DataFrame(edge_rows(edges, H, arm, seeds_label))
    out["perm_summary"] = pd.DataFrame([{
        "arm": arm, "seeds": seeds_label, "predictions": mc["n"], "permutations": perms,
        "uniform_true_edge_precision_mean": pct(mc["mc_true"].mean() / mc["n"], 4),
        "uniform_true_edge_precision_p05": pct(np.percentile(mc["mc_true"], 5) / mc["n"], 4),
        "uniform_true_edge_precision_p95": pct(np.percentile(mc["mc_true"], 95) / mc["n"], 4),
        "uniform_followed_mean": pct(mc["mc_fol"].mean() / mc["n"], 4),
        "uniform_followed_p05": pct(np.percentile(mc["mc_fol"], 5) / mc["n"], 4),
        "uniform_followed_p95": pct(np.percentile(mc["mc_fol"], 95) / mc["n"], 4),
        "observed_true_edge": int(sc["true_edge"].sum()), "observed_followed_hidden": int(sc["followed_hidden"].sum()),
        "p_true_edge_ge_observed_uniform": pct((mc["mc_true"] >= sc["true_edge"].sum()).mean()),
        "p_followed_ge_observed_uniform": pct((mc["mc_fol"] >= sc["followed_hidden"].sum()).mean()),
    }])
    return out


def write(out: dict, out_dir: pathlib.Path, prefix: str) -> None:
    for k, df in out.items():
        if k == "mc" or df is None:
            continue
        df = df.copy()
        path = out_dir / f"{prefix}-{k.replace('_', '-')}.csv"
        for col in df.columns:
            if df[col].dtype == float:
                df[col] = df[col].map(lambda x: "" if not np.isfinite(x) else f"{x:.6f}")
        df.to_csv(path, index=False, lineterminator="\n")
        print(f"wrote {path.name} ({len(df)} rows)")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--predictions", required=True, type=pathlib.Path)
    ap.add_argument("--first-alarms", type=pathlib.Path)
    ap.add_argument("--edges", type=pathlib.Path)
    ap.add_argument("--hidden", required=True, type=pathlib.Path)
    ap.add_argument("--arm", default="a2_learn")
    ap.add_argument("--out-dir", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    ap.add_argument("--prefix", default="w3-a2")
    ap.add_argument("--permutations", type=int, default=2000)
    a = ap.parse_args(argv)
    out = run(a.predictions, a.hidden, a.first_alarms, a.edges, a.arm, a.permutations)
    write(out, a.out_dir, f"{a.prefix}-{a.arm}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
