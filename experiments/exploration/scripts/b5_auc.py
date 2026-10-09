"""Feature AUCs at the ask instant (work item B5, item 3).

Usage:
  b5_auc.py FEATURE_LOG.csv [OUT_PREFIX]
      FEATURE_LOG.csv is the output of the feature-log tool (`crates/gordian-run/tests/
      stream_b5_features.rs`) over the tuning streams. Writes `<OUT_PREFIX>-feature-auc.csv` (one row per
      arm, comparison and feature: the AUC with its 90% cluster-bootstrap interval over streams, and
      the class sizes) and `<OUT_PREFIX>-feature-population.csv` (per arm and class: the notices, how
      many were live at the ask instant, how many the rung retired first). OUT_PREFIX defaults to
      `experiments/exploration/b5`.

An AUC is the probability that a randomly drawn anomaly of the first class has a higher value of the
feature than a randomly drawn anomaly of the second, a tie counting one half (`gordian_analysis.auc`).
It reads the feature at the step that made the anomaly ready (notice + 16 s), over the anomalies live
then. The comparisons (R8 of `b5_common.py`): hard (outside the slow-leak family) against plain; hard
including the slow leak against plain; leak against decoy. A feature below 0.5 ranks the classes the
other way round. The first five features are the score's; `score_z` and `peak_z` (the rung's z-scores
of the abnormal count) are the rung's existing figures, listed as not in the score.
"""

import pathlib
import sys

# B5's scripts live here (the unit's territory, E1's convention); the earlier units' modules they build
# on (B2 to B4, C1, M2) live in scripts/, and the analysis package is this checkout's, not an installed one.
_ROOT = pathlib.Path(__file__).resolve().parents[3]
sys.path.insert(0, str(_ROOT / "scripts"))
sys.path.insert(0, str(_ROOT / "analysis"))

import sys

import numpy as np
import pandas as pd

import b5_common as C
from gordian_analysis.auc import cluster_auc

FEATURES = [("contradiction", "contradiction"), ("silence", "silence"), ("evidence", "evidence_n"),
            ("services", "services_n"), ("age", "age_s")]
NOT_IN_THE_SCORE = [("rung_score_z", "score_z"), ("rung_peak_z", "peak_z")]
COMPARISONS = [
    ("hard_vs_plain", ["hard"], ["plain"]),
    ("hard_or_leak_vs_plain", ["hard", "leak"], ["plain"]),
    ("leak_vs_decoy", ["leak"], ["decoy"]),
]
CLASSES = ("background", "plain", "hard", "leak", "decoy")


def load(path):
    df = pd.read_csv(path)
    df["ready"] = df["ready"].astype(str).str.lower().eq("true")
    return df


def population(df):
    """Per arm and class: notices, ready at the ask instant, retired before it, and per stream."""
    rows = []
    for arm, g in df.groupby("arm", sort=False):
        streams = g["seed"].nunique()
        for cls in CLASSES:
            c = g[g["class"] == cls]
            rows.append({"arm": arm, "class": cls, "notices": len(c), "ready": int(c["ready"].sum()),
                         "not_ready": int((~c["ready"]).sum()), "streams": streams,
                         "ready_per_stream": float(c["ready"].sum()) / streams,
                         "notices_per_stream": len(c) / streams})
    return pd.DataFrame(rows)


def auc_rows(df, resamples=None, seed=None):
    """One row per (arm, comparison, feature)."""
    resamples = C.N_RESAMPLES if resamples is None else resamples
    seed = C.BOOT_SEED if seed is None else seed
    out = []
    for arm, g in df.groupby("arm", sort=False):
        g = g[g["ready"]]
        for name, pos_cls, neg_cls in COMPARISONS:
            sub = g[g["class"].isin(pos_cls + neg_cls)]
            positive = sub["class"].isin(pos_cls).to_numpy()
            for feat, col in FEATURES + NOT_IN_THE_SCORE:
                point, lo, hi, n_pos, n_neg = cluster_auc(
                    sub[col].to_numpy(float), positive, sub["seed"].to_numpy(), resamples=resamples,
                    seed=seed)
                out.append({"arm": arm, "comparison": name, "feature": feat,
                            "in_the_score": feat in dict(FEATURES), "auc": point, "lo": lo, "hi": hi,
                            "positives": n_pos, "negatives": n_neg,
                            "excludes_half": bool(np.isfinite(lo) and (lo > 0.5 or hi < 0.5))})
    return pd.DataFrame(out)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    prefix = sys.argv[2] if len(sys.argv) > 2 else str(C.OUT / "b5")
    df = load(sys.argv[1])
    pop = population(df)
    pop.to_csv(f"{prefix}-feature-population.csv", index=False)
    table = auc_rows(df)
    table.to_csv(f"{prefix}-feature-auc.csv", index=False)
    pd.set_option("display.width", 200)
    print(pop.to_string(index=False))
    print(table.pivot_table(index=["arm", "comparison"], columns="feature", values="auc",
                            sort=False).round(3).to_string())


if __name__ == "__main__":
    main()
