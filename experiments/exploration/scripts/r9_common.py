"""R9: the questions of the grounded-delta measurement, and nothing else that needs a decision twice.

Exploration (nothing here tests a hypothesis). The plan is `docs/local-test-plan.md`, 5R, R9. R9
reuses R8's design (`r8_common`: the levels, the nested seeded permutation of a pool, the control, the
context order, the answer's correctness) and changes three things, all fixed here before any
evaluation number:

* **The pool** holds the focus incident's own non-decisive observations (the dumper's `--pool own`),
  because the simulated reasoner's `m` counts them and a window builder carries them. The focus
  itself is not in the pool: a question shows it as the first alarm.
* **The questions** are one incident per stream ("each from a distinct stream segment where
  possible"): for a stream with `k` eligible hard incidents (not a slow leak, pool of at least 400
  observations), the one at position `seed mod k`, in incident order. Plain: the same rule among
  the plain incidents, in stream order, until the requested number is reached. The streams are
  visited in seed order. The clusters of the bootstrap are therefore streams, one question each.
  An incident whose pool is smaller than 400 is excluded before any reading, and the count is
  recorded.
* **The reader** is a program (`r9_reader.py`), not a model: there are no calls to record, and a
  question's answer is a pure function of the stream's records.

Development questions are streams 29000 to 29999 (the dumper's records of a prefix of them), the
evaluation questions streams 30000 upward. They are never mixed.

Standard library only (the reader and the selection), so that the reader can be run anywhere.
"""

import json

import r8_common as C8

LEVELS = C8.LEVELS
CONTROL_M = C8.CONTROL_M
MIN_POOL = C8.MIN_POOL

DEV_SEEDS = (29_000, 29_999)
EVAL_SEEDS_FROM = 30_000
HARD_FAMILIES = ("Compound", "Cascade", "SplitBrain")


def iter_records(path):
    """The dumper's records, one at a time (the files are hundreds of megabytes)."""
    with open(path) as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


def select_from_dump(path, n_plain, max_hard=None):
    """The questions of a dump, by the rule of this module's docstring. Returns `(hard, plain,
    exclusions)`; `plain` stops at `n_plain` streams' worth, `hard` takes one per stream over
    every stream of the dump (or the first `max_hard`)."""
    hard, plain = [], []
    ex = {"streams": 0, "hard_small_pool": 0, "plain_small_pool": 0, "hard_eligible": 0,
          "hard_total": 0, "streams_without_hard": 0}
    cur_seed, cur = None, []

    def flush():
        if cur_seed is None:
            return
        ex["streams"] += 1
        hs = [r for r in cur if r["tier"] == "Hard"]
        big = [r for r in hs if r["pool_size"] >= MIN_POOL]
        ex["hard_total"] += len(hs)
        ex["hard_small_pool"] += len(hs) - len(big)
        ex["hard_eligible"] += len(big)
        if not big:
            ex["streams_without_hard"] += 1
        elif max_hard is None or len(hard) < max_hard:
            hard.append(big[cur_seed % len(big)])
        ps = [r for r in cur if r["tier"] == "Plain"]
        pbig = [r for r in ps if r["pool_size"] >= MIN_POOL]
        if len(plain) < n_plain:
            ex["plain_small_pool"] += len(ps) - len(pbig)
            if pbig:
                plain.append(pbig[cur_seed % len(pbig)])

    for r in iter_records(path):
        if r["seed"] != cur_seed:
            flush()
            cur_seed, cur = r["seed"], []
        cur.append(r)
    flush()
    return hard, plain, ex


def write_jsonl(path, records):
    with open(path, "w") as f:
        for r in records:
            f.write(json.dumps(r, separators=(",", ":")) + "\n")


def role_of(q):
    """`{observation id: role string}` for the pool of a question (evaluator-side)."""
    return dict(zip((r["id"] for r in q["pool"]), q["pool_roles"]))


def question_contexts(q):
    """`{level or "control": [records]}` exactly as R8's design builds them."""
    return C8.contexts(q)


def truth_pair(q):
    return C8.truth_pair(q["truth"])


def correct(answer, q):
    """`answer` is `(kind, site)`; correct if it equals the truth's (kind, site)."""
    return C8.is_correct(("ok", answer[0], answer[1]), q["truth"])
