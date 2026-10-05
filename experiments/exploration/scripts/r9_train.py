"""R9: fit the reader's class weights on the DEVELOPMENT questions (seeds 29000 to 29999 only).

Usage: r9_train.py SET_DIR --out r9_reader_weights.json [--lam 1.0] [--folds 5] [--cache FEATS.npz]

SET_DIR holds hard.jsonl and plain.jsonl of the development set. For every question and every level
(0, 50, 100, 200, 400; the control is left out: it has no decisive evidence by construction, and a
reader cannot know that) the reader's features are computed (`r9_reader.features`) and a softmax over
the four classes (not hard, Compound, Cascade, SplitBrain) is fitted by weighted, L2-regularised
maximum likelihood. The labels are the truth's family (a plain incident is "not hard"). The weights
make the two populations equal in total weight (a reader tuned on hard questions alone would learn to
name a hard kind whatever it sees), each hard family equal within the hard half, and every level equal.

The labels are read here and nowhere in the reader: the weights are the only thing that carries them.
A cross-validation over streams reports how much of the accuracy is fitting.

Exploration (nothing here tests a hypothesis). Refuses any set whose seeds are not in 29000-29999.
"""

import argparse
import json
from multiprocessing import Pool
from pathlib import Path

import numpy as np
from scipy.optimize import minimize

import r9_common as C
import r9_reader as R

LV = list(C.LEVELS)


def _feats_one(q):
    ctxs = C.question_contexts(q)
    rows = []
    for lv in LV:
        f, _ = R.features(q["services"], q["focus"], ctxs[lv])
        rows.append([f[k] for k in R.FEATURES])
    return rows


def build(set_dir, workers=3):
    qs = []
    for tier in ("hard", "plain"):
        qs += list(C.iter_records(Path(set_dir) / f"{tier}.jsonl"))
    for q in qs:
        assert C.DEV_SEEDS[0] <= q["seed"] <= C.DEV_SEEDS[1], "development seeds only"
    with Pool(workers) as p:
        rows = p.map(_feats_one, qs, chunksize=4)
    X, y, stream, lv, fam = [], [], [], [], []
    for q, rs in zip(qs, rows):
        label = 0 if q["tier"] == "Plain" else 1 + R.HARD_CLASSES.index(q["family"])
        for l, r in zip(LV, rs):
            X.append(r)
            y.append(label)
            stream.append(q["seed"])
            lv.append(l)
    return np.array(X, float), np.array(y), np.array(stream), np.array(lv)


def weights_for(y):
    """Equal total weight for plain and hard, equal for each hard family, equal for each level."""
    w = np.zeros(len(y))
    share = {0: 0.5, 1: 1 / 6, 2: 1 / 6, 3: 1 / 6}
    for c, s in share.items():
        m = y == c
        w[m] = s / m.sum()
    return w * len(y)


def fit(X, y, w, lam):
    mean, scale = X.mean(axis=0), X.std(axis=0)
    scale[scale < 1e-9] = 1.0
    Z = (X - mean) / scale
    n, d = Z.shape
    k = len(R.CLASSES)
    Y = np.eye(k)[y]

    def unpack(theta):
        return theta[: k * d].reshape(k, d), theta[k * d:]

    def fun(theta):
        W, b = unpack(theta)
        S = Z @ W.T + b
        S -= S.max(axis=1, keepdims=True)
        P = np.exp(S)
        P /= P.sum(axis=1, keepdims=True)
        ll = -(w * np.log(P[np.arange(n), y] + 1e-12)).sum() / n + lam * (W ** 2).sum() / n
        G = (P - Y) * w[:, None]
        gW = G.T @ Z / n + 2 * lam * W / n
        gb = G.sum(axis=0) / n
        return ll, np.concatenate([gW.ravel(), gb])

    theta0 = np.zeros(k * d + k)
    res = minimize(fun, theta0, jac=True, method="L-BFGS-B", options={"maxiter": 2000})
    W, b = unpack(res.x)
    return mean, scale, W, b


def predict(X, params):
    mean, scale, W, b = params
    S = ((X - mean) / scale) @ W.T + b
    return S.argmax(axis=1)


def report(y_true, y_pred, lv, label):
    print(f"-- {label}")
    for name, cls in (("plain", [0]), ("compound", [1]), ("cascade", [2]), ("splitbrain", [3])):
        row = []
        for l in LV:
            m = np.isin(y_true, cls) & (lv == l)
            row.append((y_pred[m] == y_true[m]).mean())
        print(f"   {name:11s} " + " ".join(f"{v:6.3f}" for v in row))
    hard = np.isin(y_true, [1, 2, 3])
    print("   hard(family-correct) " + " ".join(f"{(y_pred[hard & (lv == l)] == y_true[hard & (lv == l)]).mean():6.3f}" for l in LV))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("set_dir")
    ap.add_argument("--out", required=True)
    ap.add_argument("--lam", type=float, default=1.0)
    ap.add_argument("--folds", type=int, default=5)
    ap.add_argument("--cache", default=None)
    a = ap.parse_args()
    if a.cache and Path(a.cache).exists():
        z = np.load(a.cache)
        X, y, stream, lv = z["X"], z["y"], z["stream"], z["lv"]
    else:
        X, y, stream, lv = build(a.set_dir)
        if a.cache:
            np.savez(a.cache, X=X, y=y, stream=stream, lv=lv)
    w = weights_for(y)
    params = fit(X, y, w, a.lam)
    report(y, predict(X, params), lv, "training fit (all development)")
    # cross-validation over streams
    seeds = np.unique(stream)
    rng = np.random.default_rng(12345)
    fold_of = dict(zip(seeds, rng.permutation(len(seeds)) % a.folds))
    folds = np.array([fold_of[s] for s in stream])
    pred = np.zeros(len(y), dtype=int)
    for k in range(a.folds):
        tr = folds != k
        wk = weights_for(y[tr])
        pk = fit(X[tr], y[tr], wk, a.lam)
        pred[~tr] = predict(X[~tr], pk)
    report(y, pred, lv, f"{a.folds}-fold cross-validation over streams (lam {a.lam})")
    mean, scale, W, b = params
    out = {
        "trained_on": "development questions, seeds 29000-29999",
        "lam": a.lam,
        "features": R.FEATURES,
        "mean": dict(zip(R.FEATURES, map(float, mean))),
        "scale": dict(zip(R.FEATURES, map(float, scale))),
        "classes": {c: {"bias": float(b[i]), "w": dict(zip(R.FEATURES, map(float, W[i])))}
                    for i, c in enumerate(R.CLASSES)},
    }
    Path(a.out).write_text(json.dumps(out, indent=1) + "\n")


if __name__ == "__main__":
    main()
