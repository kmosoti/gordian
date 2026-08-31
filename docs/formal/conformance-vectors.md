# Conformance Test Vectors

Status: **normative research specification**

`docs/implementation/execution-order.md` section 5 publishes the "proof/conformance test-vector
format from #7" as a foundation interface. This document is that format. Without it, #7 has no
serialized subject and `docs/research/verification-strategy.md` step 1 ("define canonical
serialized inputs") has no referent.

Per D1, #7 is scoped to the harness plus **one seeded predicate**: `HardDependenciesAcyclic`.
The seed consumes raw graph nodes and edges and compares the Lean predicate with #4's
deterministic reference cycle-validation/topological-order algorithm. Additional predicates are
added by later Atoms against this unchanged format.

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
    "predicate":       { "enum": ["HardDependenciesAcyclic"] },
    "input":           {
      "type": "object",
      "additionalProperties": false,
      "required": ["nodes", "edges"],
      "properties": {
        "nodes": { "type": "array", "items": { "type": "string" } },
        "edges": {
          "type": "array",
          "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["depender", "prerequisite"],
            "properties": {
              "depender": { "type": "string" },
              "prerequisite": { "type": "string" }
            }
          }
        }
      }
    },
    "evaluation_point":{ "type": "integer", "minimum": 0,
                         "description": "the EventSeq at which an event-sensitive predicate is evaluated; the HardDependenciesAcyclic seed uses 0 and reads no event state" },
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
  "vector_id": "hard-dependencies-acyclic/000017",
  "predicate": "HardDependenciesAcyclic",
  "input": {
    "nodes": ["atom:a", "atom:b", "atom:c"],
    "edges": [
      { "depender": "atom:b", "prerequisite": "atom:a" },
      { "depender": "atom:c", "prerequisite": "atom:b" }
    ]
  },
  "evaluation_point": 0,
  "expected": { "result": true, "reason": null },
  "seed": 17,
  "lean_toolchain": "leanprover/lean4:v4.33.1",
  "rust_toolchain": "1.98.0",
  "source_commit": "0000000000000000000000000000000000000000",
  "canonicalization_scheme": "gordian-canon-v1",
  "generator": "crates/gordian-core/tests/conformance.rs"
}
```

The `nodes` and `edges` arrays are the complete raw predicate input; implementations MUST NOT
silently derive or inject additional graph facts. The #4 reference algorithm supplies a
deterministic topological order for an acyclic input and a deterministic cycle result otherwise.
`reason` is optional diagnostic output and, when present, MUST be rendered identically by both
implementations.

`evaluation_point` remains part of the versioned envelope for predicates that later read recorded
event state; the graph seed fixes it to `0` and does not use it. No vector may contain a wall-clock
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
| G-202 | an executable Lean `Evidence.isCompatible` over `EvidenceRef` / `CandidateRef` for the evidence implementation to test | #15 |
| G-204 | `formal/conformance/`, the `gordian-core` runner/generator at `crates/gordian-core/tests/conformance.rs`, the seeded `HardDependenciesAcyclic` vectors, the `injected_disagreement_is_detected` test, and the CI wiring | #7 |
| G-201 | an executable `isAcceptable` so admission itself becomes a conformance predicate rather than a witness of opaque `Prop`s | #19 |
| G-206 | `dependenciesSatisfied` defined over the real dependency graph and `Frontier.Satisfied` rather than the `Environment.satisfied` oracle field | #13 |

The seed's Lean-side deliverable is an executable `HardDependenciesAcyclic` predicate over raw
nodes and edges in `formal/Gordian/Graph.lean`; the Rust side is #4's deterministic reference
implementation, exercised through `gordian-core`. This Atom does not implement the composite
readiness/dispatch predicate or evidence compatibility; those extensions remain owned by their
respective Atoms.

`predicate` is a single-member enum today (`HardDependenciesAcyclic`) precisely because #7 is
scoped to the harness plus one seeded predicate. Widening it is a deliberate schema edit by the
Atom that lands the second predicate, not a side effect of adding vectors.
