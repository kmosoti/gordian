"""R8: build the worked examples that precede every question in the reasoner prompt.

Usage: r8_make_demos.py QUESTIONS.jsonl OUT.json

QUESTIONS.jsonl is the dumper's output for the example seeds (28000 upward, a range no pilot or
scored question uses). Fourteen incidents are taken by fixed rule (`RULES`, in order), each shown with its decisive evidence
and 12 distractors in the same rendering a question uses, followed by the true answer:

four plain (CredentialExpired, DependencyDown, ConfigDrift, Intermittent; none a duo) and ten
hard (four Compound, three Cascade, three SplitBrain, mixing the two modes).

Each is the first incident of its description, in seed order, whose pool has at least 12
observations and whose context, read by the prompt's own rules (`r8_rules.read`), gives the true
answer, so that an example never teaches what the rules contradict. The answer is the evidence line
(written by `r8_rules.evidence_line` from the context shown) and the answer line. The examples are
shared by every call, so they are part of the cached prefix. The output is committed as
`r8-demos.json` and is frozen with the prompt.
"""

import json
import sys

import r8_common as C
import r8_rules as R

DEMO_M = 12

RULES = (
    ("plain", lambda q: q["family"] == "Known:CredentialExpired" and not q["duo"]),
    ("hard", lambda q: q["family"] == "Cascade" and q["mode"] == "contradict"),
    ("hard", lambda q: q["family"] == "Compound" and q["mode"] == "mimic"),
    ("plain", lambda q: q["family"] == "Known:DependencyDown" and not q["duo"]),
    ("hard", lambda q: q["family"] == "SplitBrain" and q["mode"] == "contradict"),
    ("hard", lambda q: q["family"] == "Cascade" and q["mode"] == "mimic"),
    ("hard", lambda q: q["family"] == "Compound" and q["mode"] == "contradict"),
    ("plain", lambda q: q["family"] == "Known:ConfigDrift" and not q["duo"]),
    ("hard", lambda q: q["family"] == "SplitBrain" and q["mode"] == "mimic"),
    ("hard", lambda q: q["family"] == "Compound" and q["mode"] == "mimic"),
    ("hard", lambda q: q["family"] == "Cascade" and q["mode"] == "contradict"),
    ("plain", lambda q: q["family"] == "Known:Intermittent" and not q["duo"]),
    ("hard", lambda q: q["family"] == "SplitBrain" and q["mode"] == "contradict"),
    ("hard", lambda q: q["family"] == "Compound" and q["mode"] == "contradict"),
)


def answer_line(q):
    kind, site = C.truth_pair(q["truth"])
    return f"ANSWER: {kind} s{site}"


def build(records):
    out = []
    used = set()
    for tier, pred in RULES:
        want = "Plain" if tier == "plain" else "Hard"
        for pick in (
            q
            for q in records
            if q["tier"] == want and pred(q) and q["pool_size"] >= DEMO_M and C.qkey(q) not in used
        ):
            perm = C.permutation(pick)[:DEMO_M]
            ctx = C.order_context(list(pick["decisive"]) + perm)
            if R.read(pick["services"], pick["focus"], ctx) != C.truth_pair(pick["truth"]):
                continue
            used.add(C.qkey(pick))
            out.append(
                {
                    "source": C.qkey(pick),
                    "user": C.user_text(pick["services"], pick["focus"], ctx),
                    "answer": R.evidence_line(pick["services"], pick["focus"], ctx)
                    + "\n"
                    + answer_line(pick),
                }
            )
            break
        else:
            raise SystemExit(f"no example for rule {tier}")
    return out


def main(argv):
    records = C.load_jsonl(argv[1])
    demos = build(records)
    with open(argv[2], "w") as f:
        json.dump(demos, f, indent=1)
        f.write("\n")
    for d in demos:
        print(d["source"], d["answer"].replace("\n", " | "))


if __name__ == "__main__":
    main(sys.argv)
