# Crate Map

Status: **normative research specification**

`Cargo.toml` has one workspace member today. `knowledge/graph/80-implementation.jsonld` names
eleven further planned crates, one per row below other than `gordian-kg`;
`docs/architecture.md` contained no occurrence of the word "crate", and no issue body named a
target crate path. [`../../AGENTS.md`](../../AGENTS.md) nevertheless requires
cross-crate dependencies to "follow declared architecture direction". This table is that
declaration (G-517).

Every crate below except `gordian-kg` is **planned**. A row is a commitment about where an Atom's
code goes and what it may depend on, not a claim that the crate exists.

The `May depend on` column is the **complete, transitively closed** set of Gordian crates a crate
may name in its `[dependencies]`. It is not an ordering hint and there is no second rule: a
dependency is permitted if and only if it appears in that crate's row.

| Crate | Path | May depend on (complete list) | Owning Atoms |
| --- | --- | --- | --- |
| `gordian-core` | `crates/gordian-core` | (none) | #9, #10, #11, #12, #13, #55, #56, #58 |
| `gordian-kg` | `crates/gordian-kg` | (none) | #8, #71, #72, #73, #74 |
| `gordian-evidence` | `crates/gordian-evidence` | `gordian-core` | #15, #16, #17 |
| `gordian-artifacts` | `crates/gordian-artifacts` | `gordian-core` | #14 |
| `gordian-scheduler` | `crates/gordian-scheduler` | `gordian-core` | #20, #21, #22, #24 |
| `gordian-source` | `crates/gordian-source` | `gordian-core` | #29 (trait), #34 |
| `gordian-experiments` | `crates/gordian-experiments` | `gordian-core` | #37, #77, #75 |
| `gordian-jj` | `crates/gordian-jj` | `gordian-core`, `gordian-source` | #29, #30, #31, #32, #33 |
| `gordian-git` | `crates/gordian-git` | `gordian-core`, `gordian-source` | #76 |
| `gordian-postgres` | `crates/gordian-postgres` | `gordian-core`, `gordian-evidence` | #25, #26, #27 |
| `gordian-coordination` | `crates/gordian-coordination` | `gordian-core`, `gordian-evidence`, `gordian-scheduler`, `gordian-source` | #18, #19, #23, #41 |
| `gordian-runtime` | `crates/gordian-runtime` | `gordian-core`, `gordian-evidence`, `gordian-artifacts`, `gordian-scheduler`, `gordian-source`, `gordian-jj`, `gordian-git`, `gordian-postgres`, `gordian-coordination`, `gordian-experiments` | #35, #36, #38, #44, #45, #46 |

Rules:

- `gordian-core` depends on no other Gordian crate. Canonical records, events, and the projection
  live there, so every other crate can be read as a consumer of one vocabulary.
- Adapters (`gordian-jj`, `gordian-git`) depend on `gordian-source` and never on each other;
  nothing above `gordian-source` may name a backend. This is what lets #34 vary the substrate with
  everything else held constant.
- `gordian-kg` stays independent: the research corpus tool must be buildable without the runtime.
- `gordian-runtime` is the only crate permitted to depend on an adapter crate directly, and does
  so only to select one at startup.

`scripts/check-crate-map.sh` asserts that (a) every `crates/` url in `knowledge/graph/*.jsonld`
appears as a row, (b) every `Cargo.toml` workspace member appears as a row, (c) every
`[dependencies]` entry naming a `gordian-*` crate appears in that crate's `May depend on` column,
and (d) the permitted-dependency relation is acyclic and each row is transitively closed. The
**Specification consistency** job of `.github/workflows/verify.yml` runs it through its
`for s in scripts/check-*.sh` loop, so the first violating `use` is a build failure rather than an
architectural drift discovered later.

One conjunct of G-517 is outside this document: **each implementation Atom body must name its
target crate path**, which is an issue-body edit carried by the "Adding or splitting an Atom"
checklist of [`issue-index.md`](issue-index.md#adding-or-splitting-an-atom) and automated by
`check-drift` (**G-527, assigned to #70**).

## Atoms outside the crate map

Atoms that write no Rust crate code are deliberately absent from the table: the Jujutsu
qualification suite (#1), CI and toolchain stabilisation (#2), workload and benchmark harnesses
(#3, #4, #5), verification-technique and conformance work (#6, #7), sandbox and secret brokerage
qualification (#62, #63), distributed robustness (#40, #42, #43), the explorer UI (#47), release
and operations (#64-#67), the experiment and retention reports (#39, #48, #49, #50-#54, #59-#61,
#68, #69), reconciliation and repair planning (#57), and the temporary GitHub bootstrap (#70).
When one of those Atoms does introduce a crate, it adds a row here in the same change; a crate
without a row fails `scripts/check-crate-map.sh`.

#34 is in the table under `gordian-source` because the comparison harness drives both adapters
through that trait; the experiment report it produces is not crate code.
