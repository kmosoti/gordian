"""W2, the learnable laws: shared settings, loaders and the bootstrap.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). Every
statistic these scripts print names the seed range it was measured on and its side:

- *hidden*: computed from the stream's hidden truth (`crates/gordian-stream/examples/laws`), an
  experimenter-side number about the world;
- *public ids, hidden labels*: a statistic over bytes a policy sees (message ids) grouped by a
  hidden label (the incident's family): it says what a policy could learn at best, not what it
  does learn;
- *run*: computed from a run directory's evaluator files joined to hidden truth by (seed,
  incident index).

Nothing here reaches an arm. Intervals are the program's: 90% percentile intervals of a cluster
bootstrap that resamples whole streams, 10,000 resamples, seed 9950 (R10's constants), with a
fresh generator per interval so that no number depends on the order of the others.
"""

from __future__ import annotations

import csv
import pathlib

import numpy as np
import pandas as pd

ROOT = pathlib.Path(__file__).resolve().parents[3]
OUT = ROOT / "experiments" / "exploration"
HIDDEN = ROOT / "artifacts" / "runs" / "w2" / "hidden"
RUNS = ROOT / "artifacts" / "runs" / "w2"
NS = 1_000_000_000
N_RESAMPLES = 10_000
BOOT_SEED = 9_950

# The three seed ranges of the brief. `dir` is the name under the hidden root.
RANGES = {
    "a-tune": {"label": "10000-10099", "world": "a"},
    "a-heldout": {"label": "40000-40199", "world": "a"},
    "b": {"label": "50000-50099", "world": "b"},
}


def read_range(root: pathlib.Path, name: str) -> dict[str, pd.DataFrame]:
    """The hidden tables of one range, with strings filled and the stream list checked."""
    d = root / name
    inc = pd.read_csv(d / "incidents.csv", keep_default_na=False, na_values=[""])
    for col in ("family", "mode", "known_kind", "kind_a", "kind_b"):
        inc[col] = inc[col].fillna("")
    reg = pd.read_csv(d / "regimes.csv", keep_default_na=False, na_values=[""])
    streams = pd.read_csv(d / "streams.csv")
    vocab = pd.read_csv(d / "vocab.csv")
    seeds = streams["seed"].tolist()
    assert seeds == sorted(set(seeds)), "streams must be listed once each in seed order"
    assert set(inc["seed"]) <= set(seeds)
    inc = inc.sort_values(["seed", "incident"]).reset_index(drop=True)
    inc["fam_mode"] = inc["family"] + "/" + inc["mode"]
    inc["strict"] = inc["fam_mode"] + "/" + inc["kind_a"] + "/" + inc["kind_b"]
    inc["onset_s"] = inc["onset_ns"] / NS
    return {"inc": inc, "reg": reg, "streams": streams, "vocab": vocab, "seeds": np.array(seeds)}


def seed_range(seeds) -> str:
    return f"{int(min(seeds))}-{int(max(seeds))}"


def per_stream(df: pd.DataFrame, mask, seeds) -> np.ndarray:
    """Count of rows of `df` where `mask`, per stream, in the order of `seeds` (zeros included)."""
    s = df.loc[mask].groupby("seed").size()
    return s.reindex(seeds, fill_value=0).to_numpy(dtype=float)


def boot_ratio(num: np.ndarray, den: np.ndarray | None = None) -> tuple[float, float, float]:
    """Point and 90% interval of sum(num) / sum(den) (den = streams when None), resampling streams."""
    num = np.asarray(num, dtype=float)
    den = np.ones_like(num) if den is None else np.asarray(den, dtype=float)
    n = len(num)
    point = num.sum() / den.sum() if den.sum() > 0 else float("nan")
    if n < 2:
        return point, float("nan"), float("nan")
    rng = np.random.default_rng(BOOT_SEED)
    idx = rng.integers(0, n, size=(N_RESAMPLES, n))
    d = den[idx].sum(axis=1)
    with np.errstate(invalid="ignore", divide="ignore"):
        r = np.where(d > 0, num[idx].sum(axis=1) / np.where(d > 0, d, 1), np.nan)
    lo, hi = np.nanquantile(r, [0.05, 0.95])
    return float(point), float(lo), float(hi)


def f(x, nd=4):
    """A float as text for a CSV: fixed decimals, empty for NaN."""
    if x is None or (isinstance(x, float) and not np.isfinite(x)):
        return ""
    if isinstance(x, (int, np.integer)):
        return str(int(x))
    return f"{float(x):.{nd}f}"


def write_csv(path: pathlib.Path, rows: list[dict]) -> None:
    assert rows, path
    fields = list(rows[0])
    for r in rows:
        assert list(r) == fields, (path, list(r), fields)
    with open(path, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=fields, lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {path.name} ({len(rows)} rows)")


def md_table(rows: list[dict], cols: list[str] | None = None) -> str:
    cols = cols or list(rows[0])
    out = ["| " + " | ".join(cols) + " |", "|" + "|".join("---" for _ in cols) + "|"]
    for r in rows:
        out.append("| " + " | ".join(str(r[c]) for c in cols) + " |")
    return "\n".join(out)


def quantiles(x, qs=(0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0)) -> list[float]:
    x = np.asarray(x, dtype=float)
    return [float(np.quantile(x, q)) for q in qs] if len(x) else [float("nan")] * len(qs)
