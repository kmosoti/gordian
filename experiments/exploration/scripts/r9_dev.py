"""R9: run a reader on a question set and print accuracy by level, family and mode.

Usage: r9_dev.py SET_DIR [--reader MODULE] [--limit N] [--out FILE.json] [--workers K]

SET_DIR holds hard.jsonl and plain.jsonl (`r9_select.py`). MODULE defaults to `r9_reader`; it must
offer `read(services, focus, context) -> (kind, site, trace)`. The reader is shown the services, the
focus observation record and the context's observation records, and nothing else: this harness
strips the questions to exactly those before calling it and compares the answer with the truth
afterwards. It is the development loop (on seeds 29000 to 29999) and the evaluation's first stage
(`r9_evaluate.py` reuses `answer_all`).

Exploration (nothing here tests a hypothesis).
"""

import argparse
import importlib
import json
import sys
from collections import defaultdict
from multiprocessing import Pool
from pathlib import Path

import r9_common as C

LEVELS = list(C.LEVELS) + ["control"]
_READER = None


def _init(name):
    global _READER
    _READER = importlib.import_module(name)


def answer_question(q):
    """The reader's answer at every level and the control of one question: a list of
    `(level, kind, site, correct, trace)`. The reader gets only what a reasoner's context holds."""
    ctxs = C.question_contexts(q)
    out = []
    for lv in LEVELS:
        kind, site, trace = _READER.read(q["services"], q["focus"], ctxs[lv])
        out.append((lv, kind, site, C.correct((kind, site), q), trace))
    return out


def answer_all(questions, reader, workers=3):
    _init(reader)
    if workers <= 1:
        return [answer_question(q) for q in questions]
    with Pool(workers, initializer=_init, initargs=(reader,)) as p:
        return p.map(answer_question, questions, chunksize=4)


def summarize(questions, answers, label):
    by = defaultdict(lambda: defaultdict(list))
    for q, ans in zip(questions, answers):
        keys = ["all", f"family={q['family']}"]
        if q["mode"]:
            keys.append(f"mode={q['mode']}")
            keys.append(f"{q['family']}/{q['mode']}")
        for k in keys:
            for lv, _, _, ok, _ in ans:
                by[k][lv].append(int(ok))
    print(f"\n== {label}: {len(questions)} questions")
    print(f"{'group':28s} {'n':>4s} " + " ".join(f"{str(lv):>7s}" for lv in LEVELS))
    for k in sorted(by, key=lambda s: (s != "all", s)):
        n = len(by[k][0])
        print(f"{k:28s} {n:4d} " + " ".join(f"{sum(by[k][lv]) / n:7.3f}" for lv in LEVELS))
    return {k: {str(lv): sum(v) / len(v) for lv, v in d.items()} for k, d in by.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("set_dir")
    ap.add_argument("--reader", default="r9_reader")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--out", default=None)
    ap.add_argument("--workers", type=int, default=3)
    ap.add_argument("--tiers", default="hard,plain")
    a = ap.parse_args()
    d = Path(a.set_dir)
    res = {}
    for tier in a.tiers.split(","):
        qs = list(C.iter_records(d / f"{tier}.jsonl"))
        if a.limit:
            qs = qs[: a.limit]
        ans = answer_all(qs, a.reader, a.workers)
        res[tier] = summarize(qs, ans, f"{a.reader} {tier}")
    if a.out:
        Path(a.out).write_text(json.dumps(res, indent=2) + "\n")


if __name__ == "__main__":
    sys.exit(main())
