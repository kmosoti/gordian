"""R9: choose the questions of a set (development or evaluation) from the dumper's output.

Usage: r9_select.py DUMP.jsonl OUT_DIR N_PLAIN

Writes OUT_DIR/hard.jsonl, OUT_DIR/plain.jsonl (the selected question records, with the pool and its
evaluator-side roles) and OUT_DIR/selection.json (the counts: streams, eligible hard incidents,
exclusions for a pool under 400 observations, questions chosen). The rule is
`r9_common.select_from_dump`: one hard incident per stream at position `seed mod k` among the
stream's eligible ones, plain likewise until N_PLAIN streams have one.

The dump is `target/release/examples/questions --tier both --pool own` (regimes off, as R8).

Exploration (nothing here tests a hypothesis).
"""

import json
import sys
from pathlib import Path

import r9_common as C


def main():
    dump, out, n_plain = sys.argv[1], Path(sys.argv[2]), int(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    hard, plain, ex = C.select_from_dump(dump, n_plain)
    C.write_jsonl(out / "hard.jsonl", hard)
    C.write_jsonl(out / "plain.jsonl", plain)
    ex.update({"hard_questions": len(hard), "plain_questions": len(plain), "dump": str(dump),
               "hard_seeds": [min(q["seed"] for q in hard), max(q["seed"] for q in hard)],
               "families": {f: sum(1 for q in hard if q["family"] == f) for f in C.HARD_FAMILIES},
               "modes": {m: sum(1 for q in hard if q["mode"] == m) for m in ("mimic", "contradict")}})
    (out / "selection.json").write_text(json.dumps(ex, indent=2) + "\n")
    print(json.dumps(ex, indent=2))


if __name__ == "__main__":
    main()
