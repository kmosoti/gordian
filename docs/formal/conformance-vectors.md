# Conformance Test Vectors

Status: **normative research specification**

`docs/implementation/execution-order.md` section 5 publishes the "proof/conformance test-vector
format from #7" as a foundation interface. This document is that format. Without it, #7 has no
serialized subject and `docs/research/verification-strategy.md` step 1 ("define canonical
serialized inputs") has no referent.

Per D1, #7 is scoped to the harness plus **one seeded predicate**: `Dispatchable`. Additional
predicates are added by later Atoms against this unchanged format.

## 1. Layout

```text
formal/conformance/
  <predicate>/
    <vector_id>.json        one vector per file, sorted by vector_id
  index.json                { predicate -> [vector_id, ...] }, generated, checked in
```

## 2. Vector schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://gordian.dev/schema/conformance-vector-v1.json",
  "title": "ConformanceVector",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "vector_format",
    "vector_id",
    "predicate",
    "input",
    "evaluation_point",
    "expected",
    "seed",
    "lean_toolchain",
    "rust_toolchain",
    "source_commit",
    "canonicalization_scheme"
  ],
  "properties": {
    "vector_format":   { "const": "gordian-conformance-v1" },
    "vector_id":       { "type": "string", "pattern": "^[a-z0-9-]+/[0-9]{6}$" },
    "predicate":       { "enum": ["Dispatchable"] },
    "input":           { "type": "object" },
    "evaluation_point":{ "type": "integer", "minimum": 0,
                         "description": "the EventSeq the predicate is evaluated at; lease and capability liveness are decided against it, so a clock-induced Lean/Rust divergence is representable as a vector" },
    "expected":        { "type": "object",
                         "required": ["result"],
                         "properties": {
                           "result": { "type": "boolean" },
                           "reason": { "type": ["string", "null"] } } },
    "seed":            { "type": "integer", "minimum": 0 },
    "lean_toolchain":  { "type": "string" },
    "rust_toolchain":  { "type": "string" },
    "source_commit":   { "type": "string" },
    "canonicalization_scheme": { "const": "gordian-canon-v1" },
    "generator":       { "type": "string" },
    "note":            { "type": "string" }
  }
}
```

`predicate` is an enum, not a free string, so a typo is a schema error rather than a silently
skipped vector. Adding a predicate is a deliberate schema edit.

## 3. Canonicalization and comparison rule

A vector passes when

```text
JCS(run_result) == JCS(vector.expected)
```

where `JCS` is RFC 8785 JSON Canonicalization Scheme over the object, and `run_result` is the
predicate's output rendered through the same serializer on both sides. Comparison is on canonical
bytes, never on a pretty-printed string, and never field-by-field with a "close enough" rule.

A vector whose `lean_toolchain` or `rust_toolchain` differs from the running toolchain is **not
skipped**: it is a hard failure, because a conformance claim is a claim about a pinned pair of
implementations. Regenerating vectors after a toolchain bump is an explicit, reviewable diff.

## 4. Worked example — the seeded predicate for #7

```json
{
  "vector_format": "gordian-conformance-v1",
  "vector_id": "dispatchable/000017",
  "predicate": "Dispatchable",
  "input": {
    "valid_spec": true,
    "blocked": false,
    "preconditions_hold": true,
    "compatible_executor_available": true,
    "required_resources_available": true,
    "grant": { "issued_at_event": 4100, "expires_at_event": 4400, "revoked_at_event": null },
    "leases": [
      { "subject": "semantic_resource:rust-crate://core/model",
        "mode": "write_exclusive",
        "held_by_other": true,
        "issued_at_event": 4180,
        "expires_at_event": 4300,
        "revoked_at_event": null }
    ]
  },
  "evaluation_point": 4211,
  "expected": { "result": false, "reason": "lease_compatible" },
  "seed": 17,
  "lean_toolchain": "leanprover/lean4:v4.33.1",
  "rust_toolchain": "1.98.0",
  "source_commit": "0000000000000000000000000000000000000000",
  "canonicalization_scheme": "gordian-canon-v1",
  "generator": "crates/gordian-core/tests/conformance/generate.rs"
}
```

`reason` names the first false conjunct in the order fixed by
`docs/spec/mission-graph.md` `## Logical state predicates`. Both implementations MUST evaluate
conjuncts in that order, so `reason` is deterministic and a divergence in *which* conjunct failed
is caught even when the boolean agrees.

`evaluation_point` is a required field because `AuthorizationValid` and `LeaseCompatible` decide
liveness by comparing `EventSeq` values, and a vector that carried only seven booleans could not
express — and therefore could not catch — a Lean/Rust divergence caused by a lease or grant that
one side considered live and the other did not. Vectors carry the raw `issued_at_event` /
`expires_at_event` / `revoked_at_event` values rather than a pre-computed `lease_compatible`
boolean, so the liveness arithmetic itself is under test. No vector may contain a wall-clock
timestamp.

## 5. Harness contract

```text
cargo test -p gordian-core --test conformance      runs every vector against the Rust predicate
lake exe conformance formal/conformance            runs every vector against the Lean predicate
scripts/check-conformance-index.sh                 index.json matches the files on disk
```

All three run in CI. A vector present on disk and absent from `index.json` fails; so does the
reverse. A run with zero vectors fails: an empty conformance suite MUST NOT report success.

## 6. What this document is not

This document defines the **format**. It does not build the harness, and nothing in this
repository runs a vector yet. The three commands in section 5 name deliverables, not existing
binaries. The work is assigned as follows, and no other Atom may claim it closed:

| Gap | Deliverable | Owner |
| --- | --- | --- |
| G-202 | an executable Lean `isCompatible` over `EvidenceRef` / `CandidateRef` for the Rust side to differentially test | #7 |
| G-204 | `formal/conformance/`, `crates/gordian-conformance/`, the seeded generator, the `injected_disagreement_is_detected` test, and the CI wiring | #7 |
| G-201 | an executable `isAcceptable` so admission itself becomes a conformance predicate rather than a witness of opaque `Prop`s | #19 |
| G-206 | `dependenciesSatisfied` defined over the real dependency graph and `Frontier.Satisfied` rather than the `Environment.satisfied` oracle field | #13 |

The Lean side of the seeded predicate already exists: `enabled` and `dispatchable` in
`formal/Gordian/Scheduler.lean` are `Bool`-valued functions over concrete records (G-203), and
`Satisfied` / `Blocked` / `Active` in `formal/Gordian/Frontier.lean` are defined (G-205). What is
missing is the Rust counterpart, the vectors, and the runner that compares them.

`predicate` is a single-member enum today (`Dispatchable`) precisely because #7 is scoped to the
harness plus one seeded predicate. Widening it is a deliberate schema edit by the Atom that lands
the second predicate, not a side effect of adding vectors.
