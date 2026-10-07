"""Criteria as code (work item V1): a fixed criterion, written as a specification, evaluated from
the raw evaluator files of kept runs.

The verdict is computed, not read (charter section 11). A specification (`experiments/criteria/`,
schema in its README) names the run roles, the arms and the evaluator files, the measures, the
bootstrap, the tested clauses and the tree that combines them. `evaluate` returns the whole result
as plain Python values; `scripts/criterion.py` composes it with files and arguments and does no
arithmetic of its own.

Everything is read from the raw CSV files of an arm directory with `pandas.read_csv`; nothing here
uses `load.py`, `stream.py` or the lab scripts, so that a defect in those cannot hide in both. The
statistical procedure is the one the lab scripts used (B1 to M3 and L1), restated:

* the unit of replication is the stream; every measure is a ratio of sums over streams, or a mean
  per stream, never a mean of per-stream ratios;
* resamples are multinomial stream counts from `numpy.random.default_rng(seed)`, drawn once per
  window length in chunks, so that every measure and every arm of a window is evaluated on the same
  resamples and a difference between two arms is paired;
* an interval is two percentiles of the resample distribution with undefined resamples dropped, by
  `numpy.quantile` with `method` `lower` / `higher` (the B1 to M3 scripts) or the default linear
  interpolation (L1's script).
"""

from __future__ import annotations

import hashlib
import json
import math
import operator
import platform
from pathlib import Path

import numpy as np
import pandas as pd

SCHEMA = "gordian-criterion/1"
INTERVALS = ("lower_higher", "linear")
OPS = {">=": operator.ge, ">": operator.gt, "<=": operator.le, "<": operator.lt}
WHERE_OPS = ("eq", "ne", "in", "not_in")
CLAUSE_KINDS = ("value", "paired", "curve_slope", "identity")
REPORT_KINDS = ("table", "paired_table", "slope_table")
ON = ("point", "lower", "upper")


class SpecError(ValueError):
    """The specification, or the run directories it is evaluated on, are not what it says."""


# ---------------------------------------------------------------------------------------------
# The specification
# ---------------------------------------------------------------------------------------------


def canonical_json(obj) -> str:
    """The JSON text a specification is hashed by: sorted keys, no insignificant whitespace."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def spec_hash(spec: dict) -> str:
    return hashlib.sha256(canonical_json(spec).encode()).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for block in iter(lambda: fh.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load_spec(path) -> dict:
    with open(path) as fh:
        spec = json.load(fh)
    validate_spec(spec)
    return spec


def _need(cond: bool, msg: str) -> None:
    if not cond:
        raise SpecError(msg)


def _validate_term(term, where: str) -> None:
    _need(isinstance(term, dict), f"{where}: a term is an object")
    if "per_stream" in term:
        _need(set(term) == {"per_stream"}, f"{where}: per_stream takes no other key")
        _need(isinstance(term["per_stream"], (int, float)), f"{where}: per_stream is a number")
        return
    _need(term.get("agg") in ("count", "sum"), f"{where}: agg is count or sum")
    _need(isinstance(term.get("file"), str), f"{where}: file is a string")
    if term["agg"] == "sum":
        cols = term.get("columns")
        _need(isinstance(cols, dict) and cols, f"{where}: sum needs a non-empty columns object")
        _need(all(isinstance(v, (int, float)) for v in cols.values()), f"{where}: coefficients")
    for w in term.get("where", []):
        _need(w.get("op") in WHERE_OPS and "column" in w and "value" in w, f"{where}: where item {w}")
        if w["op"] in ("in", "not_in"):
            _need(isinstance(w["value"], list), f"{where}: in / not_in take a list")


def _validate_window(window, where: str) -> None:
    if window is None:
        return
    _need(isinstance(window, dict) and len(window) == 1, f"{where}: window has one key")
    ((k, v),) = window.items()
    _need(k in ("first", "last", "slice"), f"{where}: window key {k}")
    if k == "slice":
        _need(isinstance(v, list) and len(v) == 2, f"{where}: slice is [a, b]")
    else:
        _need(isinstance(v, int) and v > 0, f"{where}: {k} is a positive integer")


def _validate_tests(tests, where: str) -> None:
    for t in tests or []:
        _need(t.get("on") in ON and t.get("op") in OPS and isinstance(t.get("value"), (int, float)),
              f"{where}: test {t}")


def _tree_ids(tree, out: list[str], names: list[str], where: str) -> None:
    if isinstance(tree, str):
        out.append(tree)
        return
    _need(isinstance(tree, dict), f"{where}: a tree item is a clause id or a node")
    kinds = [k for k in ("all", "any") if k in tree]
    _need(len(kinds) == 1, f"{where}: a node has exactly one of all, any")
    _need(isinstance(tree[kinds[0]], list) and tree[kinds[0]], f"{where}: a node needs items")
    if "name" in tree:
        names.append(tree["name"])
    for item in tree[kinds[0]]:
        _tree_ids(item, out, names, where)


def validate_spec(spec: dict) -> None:
    """Raise `SpecError` for anything the evaluator would otherwise have to guess."""
    _need(spec.get("schema") == SCHEMA, f"schema must be {SCHEMA!r}")
    for key in ("id", "title", "source", "runs", "arms", "bootstrap", "measures", "clauses", "verdict"):
        _need(key in spec, f"missing key {key!r}")
    for role, r in spec["runs"].items():
        s = r.get("streams", {})
        _need(isinstance(s.get("first_seed"), int) and isinstance(s.get("count"), int)
              and s["count"] > 0, f"runs.{role}.streams needs first_seed and count")
    for name, a in spec["arms"].items():
        _need(a.get("run") in spec["runs"] and isinstance(a.get("dir"), str),
              f"arms.{name}: run must be a declared role and dir a string")
    b = spec["bootstrap"]
    _need(b.get("unit") == "stream", "bootstrap.unit: only 'stream' is implemented")
    _need(isinstance(b.get("resamples"), int) and b["resamples"] > 0, "bootstrap.resamples")
    _need(isinstance(b.get("seed"), int) and not isinstance(b.get("seed"), bool), "bootstrap.seed is an integer")
    p = b.get("percentiles")
    _need(isinstance(p, list) and len(p) == 2 and 0 <= p[0] < p[1] <= 100, "bootstrap.percentiles")
    _need(b.get("interval") in INTERVALS, f"bootstrap.interval is one of {INTERVALS}")
    _need(isinstance(b.get("chunk", 500), int) and b.get("chunk", 500) > 0, "bootstrap.chunk")
    for name, m in spec["measures"].items():
        _need(m.get("kind") == "ratio", f"measures.{name}: only kind 'ratio'")
        _validate_term(m.get("num"), f"measures.{name}.num")
        _validate_term(m.get("den"), f"measures.{name}.den")
    ids = set()
    for c in spec["clauses"]:
        cid = c.get("id")
        _need(isinstance(cid, str) and cid not in ids, f"clause id {cid!r} missing or repeated")
        ids.add(cid)
        _need(c.get("kind") in CLAUSE_KINDS, f"clause {cid}: kind")
        _validate_tests(c.get("tests"), f"clause {cid}")
        _validate_window(c.get("window"), f"clause {cid}")
        if c["kind"] == "identity":
            _need(c.get("a") in spec["arms"] and c.get("b") in spec["arms"], f"clause {cid}: arms a, b")
            _need(c.get("files"), f"clause {cid}: files")
            continue
        _need(c.get("arm") in spec["arms"], f"clause {cid}: arm")
        _need(c.get("measure") in spec["measures"], f"clause {cid}: measure")
        if c["kind"] == "paired":
            _need(c.get("minus") in spec["arms"], f"clause {cid}: minus")
        if c["kind"] == "curve_slope":
            _need(c.get("minus") is None or c["minus"] in spec["arms"], f"clause {cid}: minus")
            _need(isinstance(c.get("per"), (int, float)), f"clause {cid}: per")
            _need(list((c.get("window") or {})) == ["first"], f"clause {cid}: curve_slope window is first")
    for r in spec.get("reports", []):
        _need(r.get("kind") in REPORT_KINDS, f"report {r.get('id')}: kind")
        _validate_window(r.get("window"), f"report {r.get('id')}")
        for m in r.get("measures", [r.get("measure")]):
            _need(m in spec["measures"], f"report {r.get('id')}: measure {m!r}")
        for a in r.get("arms", []) + [x for p_ in r.get("pairs", []) for x in p_]:
            _need(a in spec["arms"], f"report {r.get('id')}: arm {a!r}")
    used: list[str] = []
    names: list[str] = []
    _tree_ids(spec["verdict"], used, names, "verdict")
    for name, tree in spec.get("observations", {}).items():
        _tree_ids(tree, used, names, f"observations.{name}")
        names.append(name)
    _need(len(names) == len(set(names)), "node names must be unique")
    by_id = {c["id"]: c for c in spec["clauses"]}
    for cid in used:
        _need(cid in by_id, f"the tree names unknown clause {cid!r}")
        c = by_id[cid]
        _need(c["kind"] == "identity" or c.get("tests"), f"clause {cid} is in a tree and has no tests")


# ---------------------------------------------------------------------------------------------
# Reading the raw files
# ---------------------------------------------------------------------------------------------


class RunSet:
    """The run directories bound to roles, and the evaluator files read from them.

    Every file is read once; its SHA-256 is recorded, so that the verdict names exactly the bytes
    it was computed from."""

    def __init__(self, spec: dict, dirs: dict[str, Path]):
        self.spec = spec
        missing = set(spec["runs"]) - set(dirs)
        extra = set(dirs) - set(spec["runs"])
        _need(not missing, f"no --run given for role(s) {sorted(missing)}")
        _need(not extra, f"--run names role(s) the specification does not declare: {sorted(extra)}")
        self.dirs = {r: Path(d) for r, d in dirs.items()}
        for r, d in self.dirs.items():
            _need(d.is_dir(), f"run directory for role {r!r} is not a directory: {d}")
        self._tables: dict[tuple[str, str], pd.DataFrame] = {}
        self._text: dict[tuple[str, str], pd.DataFrame] = {}
        self.hashes: dict[tuple[str, str], str] = {}
        self._seeds: dict[str, np.ndarray] = {}

    def path(self, arm: str, file: str) -> Path:
        a = self.spec["arms"][arm]
        return self.dirs[a["run"]] / a["dir"] / file

    def _hash(self, arm: str, file: str) -> None:
        a = self.spec["arms"][arm]
        rel = f"{a['dir']}/{file}"
        self.hashes[(a["run"], rel)] = sha256_file(self.path(arm, file))

    def table(self, arm: str, file: str) -> pd.DataFrame:
        key = (arm, file)
        if key not in self._tables:
            p = self.path(arm, file)
            _need(p.is_file(), f"arm {arm!r}: {p} does not exist")
            self._hash(arm, file)
            df = pd.read_csv(p)
            _need("seed" in df.columns, f"{p}: no seed column")
            self._tables[key] = df
        return self._tables[key]

    def text(self, arm: str, file: str) -> pd.DataFrame:
        """The file as strings, for exact comparison."""
        key = (arm, file)
        if key not in self._text:
            p = self.path(arm, file)
            _need(p.is_file(), f"arm {arm!r}: {p} does not exist")
            self._hash(arm, file)
            self._text[key] = pd.read_csv(p, dtype=str, keep_default_na=False)
        return self._text[key]

    def seeds(self, arm: str) -> np.ndarray:
        """The streams of an arm: the seeds of its results.csv, ascending, which must be exactly the
        range the specification declares for the arm's run role."""
        if arm not in self._seeds:
            res = self.table(arm, "results.csv")["seed"].to_numpy()
            _need(len(set(res.tolist())) == len(res), f"arm {arm!r}: results.csv repeats a seed")
            s = np.sort(res).astype(np.int64)
            decl = self.spec["runs"][self.spec["arms"][arm]["run"]]["streams"]
            want = np.arange(decl["first_seed"], decl["first_seed"] + decl["count"], dtype=np.int64)
            _need(np.array_equal(s, want),
                  f"arm {arm!r}: streams are {s[:1].tolist()}..{s[-1:].tolist()} ({len(s)}), the "
                  f"specification declares {want[0]}..{want[-1]} ({len(want)})")
            self._seeds[arm] = s
        return self._seeds[arm]

    def identities(self) -> dict:
        """Per run role: the directory, the manifest's identity fields and the SHA-256 of every file
        read (paths relative to the run directory)."""
        out = {}
        for role, d in sorted(self.dirs.items()):
            entry = {"path": str(d), "resolved": str(d.resolve())}
            mp = d / "manifest.json"
            if mp.is_file():
                m = json.loads(mp.read_text())
                entry["manifest_sha256"] = sha256_file(mp)
                for k in ("run_id", "experiment", "source_revision", "lockfile_sha256", "toolchain"):
                    if k in m:
                        entry[k] = m[k]
            else:
                entry["manifest_sha256"] = None
            entry["files_read"] = {rel: h for (r, rel), h in sorted(self.hashes.items()) if r == role}
            out[role] = entry
        return out


# ---------------------------------------------------------------------------------------------
# Measures: per-stream numerators and denominators
# ---------------------------------------------------------------------------------------------


def _row_mask(df: pd.DataFrame, where: list[dict] | None, label: str) -> np.ndarray:
    mask = np.ones(len(df), dtype=bool)
    for w in where or []:
        _need(w["column"] in df.columns, f"{label}: no column {w['column']!r}")
        col = df[w["column"]]
        op, v = w["op"], w["value"]
        if op == "eq":
            hit = col == v
        elif op == "ne":
            hit = col != v
        elif op == "in":
            hit = col.isin(v)
        else:
            hit = ~col.isin(v)
        mask &= hit.to_numpy(dtype=bool)
    return mask


def term_vector(runs: RunSet, arm: str, term: dict) -> np.ndarray:
    """One number per stream (ascending seed) for a term of a measure."""
    seeds = runs.seeds(arm)
    if "per_stream" in term:
        return np.full(len(seeds), float(term["per_stream"]))
    df = runs.table(arm, term["file"])
    label = f"arm {arm!r}, {term['file']}"
    bad = set(df["seed"].unique().tolist()) - set(seeds.tolist())
    _need(not bad, f"{label}: seeds outside the arm's streams: {sorted(bad)[:5]}")
    rows = df[_row_mask(df, term.get("where"), label)]
    if term["agg"] == "count":
        by = rows.groupby("seed").size().astype(float)
    else:
        total = pd.Series(0.0, index=rows.index)
        for col, coef in term["columns"].items():
            _need(col in rows.columns, f"{label}: no column {col!r}")
            try:
                total = total + coef * rows[col].astype(float)
            except (TypeError, ValueError) as e:
                raise SpecError(f"{label}: column {col!r} is not numeric or boolean: {e}") from e
        by = total.groupby(rows["seed"]).sum()
    if df["seed"].is_unique:
        # one row per stream: a per-stream file, which must cover every stream
        _need(len(df) == len(seeds), f"{label}: one row per seed, but {len(df)} rows for {len(seeds)} streams")
    return by.reindex(seeds, fill_value=0.0).to_numpy(float)


class Arms:
    """Per-arm, per-measure numerator and denominator vectors, cached."""

    def __init__(self, runs: RunSet):
        self.runs = runs
        self.measures = runs.spec["measures"]
        self._cache: dict[tuple[str, str], tuple[np.ndarray, np.ndarray, float]] = {}

    def nd(self, arm: str, measure: str) -> tuple[np.ndarray, np.ndarray, float]:
        key = (arm, measure)
        if key not in self._cache:
            m = self.measures[measure]
            self._cache[key] = (term_vector(self.runs, arm, m["num"]),
                                term_vector(self.runs, arm, m["den"]), float(m.get("scale", 1.0)))
        return self._cache[key]

    def seeds(self, arm: str) -> np.ndarray:
        return self.runs.seeds(arm)


def window_bounds(n: int, window: dict | None) -> tuple[int, int]:
    """[a, b) over the n streams in seed order."""
    if window is None:
        return 0, n
    ((kind, v),) = window.items()
    if kind == "first":
        a, b = 0, v
    elif kind == "last":
        a, b = n - v, n
    else:
        a, b = v
    _need(0 <= a < b <= n, f"window {window} does not fit {n} streams")
    return a, b


# ---------------------------------------------------------------------------------------------
# The bootstrap
# ---------------------------------------------------------------------------------------------


def draw_counts(seed: int, n: int, resamples: int, chunk: int) -> np.ndarray:
    """Multinomial stream counts, (resamples, n): row r says how many times each of the n streams
    is in resample r. A fresh generator per call, the draws in chunks, as the lab scripts drew them."""
    rng = np.random.default_rng(seed)
    parts, done = [], 0
    while done < resamples:
        k = min(chunk, resamples - done)
        parts.append(rng.multinomial(n, np.full(n, 1.0 / n), size=k).astype(float))
        done += k
    return np.concatenate(parts)


def interval(x: np.ndarray, percentiles, method: str) -> tuple[float, float]:
    """The interval of a resample distribution, undefined resamples dropped. NaN, NaN if none remain."""
    x = np.asarray(x, dtype=float)
    x = x[~np.isnan(x)]
    if not len(x):
        return float("nan"), float("nan")
    lo, hi = percentiles[0] / 100.0, percentiles[1] / 100.0
    if method == "lower_higher":
        return float(np.quantile(x, lo, method="lower")), float(np.quantile(x, hi, method="higher"))
    return float(np.quantile(x, lo)), float(np.quantile(x, hi))


def ratio_draws(num: np.ndarray, den: np.ndarray, scale: float, w: np.ndarray) -> np.ndarray:
    """The measure on every resample: sum(w * num) / sum(w * den) * scale, NaN where the denominator
    is not positive."""
    n_, d_ = w @ num, w @ den
    with np.errstate(divide="ignore", invalid="ignore"):
        return np.where(d_ > 0, n_ / np.where(d_ > 0, d_, 1.0), np.nan) * scale


def ratio_point(num: np.ndarray, den: np.ndarray, scale: float) -> float:
    d = float(den.sum())
    return float(num.sum()) / d * scale if d > 0 else float("nan")


def curve_slopes(num: np.ndarray, den: np.ndarray, w: np.ndarray) -> np.ndarray:
    """Least-squares slope, against the stream number 1..k, of the cumulative ratio
    cumsum(w * num) / cumsum(w * den), one slope per row of `w` (rows are resamples; one row of ones
    is the point estimate). Streams before the first denominator are left out; NaN with fewer than
    two points."""
    k = num.shape[0]
    x = np.arange(1, k + 1, dtype=float)
    cn = np.cumsum(w * num, axis=1)
    cd = np.cumsum(w * den, axis=1)
    ok = cd > 0
    eff = np.where(ok, cn / np.where(ok, cd, 1.0), 0.0)
    cnt = ok.sum(axis=1)
    sx = (ok * x).sum(axis=1)
    sxx = (ok * x * x).sum(axis=1)
    sy = eff.sum(axis=1)
    sxy = (eff * x).sum(axis=1)
    with np.errstate(divide="ignore", invalid="ignore"):
        var = sxx - sx * sx / cnt
        cov = sxy - sx * sy / cnt
        return np.where((cnt > 1) & (var > 0), cov / var, np.nan)


class Bootstrap:
    """The resamples of the specification, one set per window length."""

    def __init__(self, cfg: dict):
        self.cfg = cfg
        self._w: dict[int, np.ndarray] = {}

    def counts(self, n: int) -> np.ndarray:
        if n not in self._w:
            self._w[n] = draw_counts(self.cfg["seed"], n, self.cfg["resamples"], self.cfg.get("chunk", 500))
        return self._w[n]

    def interval(self, x: np.ndarray) -> tuple[float, float]:
        return interval(x, self.cfg["percentiles"], self.cfg["interval"])


# ---------------------------------------------------------------------------------------------
# Clauses
# ---------------------------------------------------------------------------------------------


def _nan_none(x):
    return None if isinstance(x, float) and math.isnan(x) else x


def _estimate(point: float, lo: float, hi: float) -> dict:
    return {"point": _nan_none(float(point)), "lower": _nan_none(float(lo)), "upper": _nan_none(float(hi))}


class Evaluator:
    def __init__(self, spec: dict, runs: RunSet):
        self.spec = spec
        self.runs = runs
        self.arms = Arms(runs)
        self.boot = Bootstrap(spec["bootstrap"])

    # -- values -------------------------------------------------------------------------------

    def _window(self, arm: str, window) -> tuple[int, int, int]:
        n = len(self.arms.seeds(arm))
        a, b = window_bounds(n, window)
        return a, b, n

    def value(self, arm: str, measure: str, window=None) -> dict:
        num, den, scale = self.arms.nd(arm, measure)
        a, b, _ = self._window(arm, window)
        num, den = num[a:b], den[a:b]
        point = ratio_point(num, den, scale)
        lo, hi = self.boot.interval(ratio_draws(num, den, scale, self.boot.counts(b - a)))
        return {**_estimate(point, lo, hi), "streams": b - a}

    def paired(self, arm: str, minus: str, measure: str, window=None) -> dict:
        sa, sb = self.arms.seeds(arm), self.arms.seeds(minus)
        _need(np.array_equal(sa, sb), f"paired arms {arm!r} and {minus!r} are not on the same streams")
        na, da, sca = self.arms.nd(arm, measure)
        nb, db, scb = self.arms.nd(minus, measure)
        a, b, _ = self._window(arm, window)
        w = self.boot.counts(b - a)
        point = ratio_point(na[a:b], da[a:b], sca) - ratio_point(nb[a:b], db[a:b], scb)
        d = ratio_draws(na[a:b], da[a:b], sca, w) - ratio_draws(nb[a:b], db[a:b], scb, w)
        lo, hi = self.boot.interval(d)
        return {**_estimate(point, lo, hi), "streams": b - a}

    def curve_slope(self, arm: str, measure: str, k: int, per: float, minus: str | None = None) -> dict:
        n = len(self.arms.seeds(arm))
        a, b = window_bounds(n, {"first": k})
        num, den, scale = self.arms.nd(arm, measure)
        num, den = num[a:b] * scale, den[a:b]
        w = self.boot.counts(b - a)
        point = curve_slopes(num, den, np.ones((1, b - a)))[0]
        draws = curve_slopes(num, den, w)
        if minus is not None:
            _need(np.array_equal(self.arms.seeds(arm), self.arms.seeds(minus)),
                  f"arms {arm!r} and {minus!r} are not on the same streams")
            n2, d2, s2 = self.arms.nd(minus, measure)
            n2, d2 = n2[a:b] * s2, d2[a:b]
            point -= curve_slopes(n2, d2, np.ones((1, b - a)))[0]
            draws = draws - curve_slopes(n2, d2, w)
        lo, hi = self.boot.interval(draws)
        return {**_estimate(point * per, lo * per, hi * per), "streams": b - a}

    def identity(self, c: dict) -> dict:
        ignore = set(c.get("ignore_columns", []))
        per_file = {}
        for f in c["files"]:
            x, y = self.runs.text(c["a"], f), self.runs.text(c["b"], f)
            xc = [col for col in x.columns if col not in ignore]
            yc = [col for col in y.columns if col not in ignore]
            if xc != yc:
                per_file[f] = {"equal": False, "why": "columns differ", "rows": [len(x), len(y)]}
                continue
            same = len(x) == len(y) and bool(x[xc].reset_index(drop=True).equals(y[yc].reset_index(drop=True)))
            per_file[f] = {"equal": same, "rows": [len(x), len(y)], "columns_compared": len(xc)}
            if not same and len(x) == len(y):
                per_file[f]["rows_differing"] = int((x[xc].reset_index(drop=True)
                                                     != y[yc].reset_index(drop=True)).any(axis=1).sum())
        return {"files": per_file, "equal": all(v["equal"] for v in per_file.values())}

    # -- clauses ------------------------------------------------------------------------------

    def clause(self, c: dict) -> dict:
        out = {"id": c["id"], "kind": c["kind"], "description": c.get("description", "")}
        if c["kind"] == "identity":
            res = self.identity(c)
            out.update({"a": c["a"], "b": c["b"], **res, "pass": res["equal"], "tests": []})
            return out
        out.update({"arm": c["arm"], "measure": c["measure"]})
        if c.get("window") is not None:
            out["window"] = c["window"]
        if c["kind"] == "value":
            est = self.value(c["arm"], c["measure"], c.get("window"))
        elif c["kind"] == "paired":
            out["minus"] = c["minus"]
            est = self.paired(c["arm"], c["minus"], c["measure"], c.get("window"))
        else:
            out["minus"] = c.get("minus")
            out["per"] = c["per"]
            est = self.curve_slope(c["arm"], c["measure"], c["window"]["first"], c["per"], c.get("minus"))
        out.update(est)
        results = []
        for t in c.get("tests", []):
            obs = est[t["on"]]
            ok = obs is not None and bool(OPS[t["op"]](obs, t["value"]))
            results.append({**t, "observed": obs, "pass": ok})
        out["tests"] = results
        out["pass"] = all(r["pass"] for r in results) if results else None
        return out

    # -- reports ------------------------------------------------------------------------------

    def report(self, r: dict) -> dict:
        cells = []
        window = r.get("window")
        if r["kind"] == "table":
            for arm in r["arms"]:
                for m in r["measures"]:
                    cells.append({"row": arm, "measure": m, **self.value(arm, m, window)})
        elif r["kind"] == "paired_table":
            for a_, b_ in r["pairs"]:
                for m in r["measures"]:
                    cells.append({"row": f"{a_} - {b_}", "measure": m, **self.paired(a_, b_, m, window)})
        else:
            for arm in r["arms"]:
                for k in r["firsts"]:
                    cells.append({"row": arm, "measure": f"{r['measure']} slope, first {k}",
                                  **self.curve_slope(arm, r["measure"], k, r["per"])})
        return {"id": r["id"], "kind": r["kind"], "description": r.get("description", ""),
                "window": window, "cells": cells}


# ---------------------------------------------------------------------------------------------
# The verdict tree
# ---------------------------------------------------------------------------------------------


def eval_tree(tree, outcomes: dict[str, bool], nodes: dict[str, bool]) -> bool:
    """Evaluate a tree of clause ids and all / any nodes; record every named node."""
    if isinstance(tree, str):
        return bool(outcomes[tree])
    kind = "all" if "all" in tree else "any"
    vals = [eval_tree(item, outcomes, nodes) for item in tree[kind]]
    v = all(vals) if kind == "all" else any(vals)
    if "name" in tree:
        nodes[tree["name"]] = v
    return v


def evaluate(spec: dict, run_dirs: dict[str, Path | str]) -> dict:
    """Evaluate a specification on run directories bound to its roles. Returns the verdict as plain
    Python values (JSON-serialisable)."""
    validate_spec(spec)
    runs = RunSet(spec, {k: Path(v) for k, v in run_dirs.items()})
    ev = Evaluator(spec, runs)
    clauses = [ev.clause(c) for c in spec["clauses"]]
    outcomes = {c["id"]: c["pass"] for c in clauses}
    nodes: dict[str, bool] = {}
    holds = eval_tree(spec["verdict"], outcomes, nodes)
    observations = {}
    for name, tree in spec.get("observations", {}).items():
        observations[name] = eval_tree(tree, outcomes, nodes)
    in_verdict: list[str] = []
    _tree_ids(spec["verdict"], in_verdict, [], "verdict")
    for c in clauses:
        c["in_verdict"] = c["id"] in in_verdict
    reports = [ev.report(r) for r in spec.get("reports", [])]
    return {
        "spec": {"id": spec["id"], "title": spec["title"], "sha256": spec_hash(spec),
                 "schema": spec["schema"], "source": spec["source"]},
        "bootstrap": spec["bootstrap"],
        "runs": runs.identities(),
        "environment": {"python": platform.python_version(), "numpy": np.__version__,
                        "pandas": pd.__version__},
        "verdict": {"name": spec["verdict"].get("name", ""), "holds": holds, "nodes": nodes,
                    "observations": observations},
        "clauses": clauses,
        "reports": reports,
    }


# ---------------------------------------------------------------------------------------------
# The Markdown table
# ---------------------------------------------------------------------------------------------


def _fmt(x, nd=4) -> str:
    return "n/a" if x is None else f"{x:.{nd}f}"


def _est(c: dict, nd=4) -> str:
    return f"{_fmt(c['point'], nd)} [{_fmt(c['lower'], nd)}, {_fmt(c['upper'], nd)}]"


def render_markdown(result: dict) -> str:
    """The verdict as a Markdown document: header, clauses, nodes, reports."""
    s, v = result["spec"], result["verdict"]
    b = result["bootstrap"]
    lines = [f"# {s['title']}", "",
             f"Verdict: **{'HOLDS' if v['holds'] else 'DOES NOT HOLD'}** ({v['name']})", "",
             f"Specification `{s['id']}`, sha256 `{s['sha256']}`.  ",
             f"Bootstrap: {b['resamples']} resamples of whole streams, seed {b['seed']}, "
             f"percentiles {b['percentiles']}, interval `{b['interval']}`.", ""]
    for role, r in result["runs"].items():
        lines.append(f"Run `{role}`: `{r['path']}` (run_id `{r.get('run_id')}`, source revision "
                     f"`{r.get('source_revision')}`, {len(r['files_read'])} files read).  ")
    lines += ["", "## Clauses", "", "| clause | measure | window | value [5th, 95th percentile] | tests | outcome |",
              "|---|---|---|---|---|---|"]
    for c in result["clauses"]:
        tag = "" if c["in_verdict"] else " (beside)"
        if c["kind"] == "identity":
            lines.append(f"| `{c['id']}`{tag} | identity of `{c['a']}` and `{c['b']}` | | "
                         f"{'equal' if c['equal'] else 'DIFFERENT'} | | {'pass' if c['pass'] else 'FAIL'} |")
            continue
        meas = c["measure"] + (f" ({c['arm']} - {c['minus']})" if c.get("minus") else f" ({c['arm']})")
        win = json.dumps(c["window"]) if c.get("window") else "all"
        tests = "; ".join(f"{t['on']} {t['op']} {t['value']}: {_fmt(t['observed'])} "
                          f"{'ok' if t['pass'] else 'FAIL'}" for t in c["tests"]) or "reported"
        out = "-" if c["pass"] is None else ("pass" if c["pass"] else "FAIL")
        lines.append(f"| `{c['id']}`{tag} | {meas} | {win} | {_est(c)} | {tests} | {out} |")
    lines += ["", "## Nodes", "", "| node | value |", "|---|---|"]
    for k, val in v["nodes"].items():
        lines.append(f"| {k} | {val} |")
    if v["observations"]:
        lines += ["", "Observations (not in the verdict):", ""]
        for k, val in v["observations"].items():
            lines.append(f"- {k}: {val}")
    for r in result["reports"]:
        lines += ["", f"## Report `{r['id']}`", "", r["description"], ""]
        measures = list(dict.fromkeys(c["measure"] for c in r["cells"]))
        rows = list(dict.fromkeys(c["row"] for c in r["cells"]))
        lines.append("| | " + " | ".join(measures) + " |")
        lines.append("|---|" + "---|" * len(measures))
        cell = {(c["row"], c["measure"]): c for c in r["cells"]}
        for row in rows:
            lines.append(f"| {row} | " + " | ".join(
                _est(cell[(row, m)], 3) if (row, m) in cell else "" for m in measures) + " |")
    return "\n".join(lines) + "\n"
