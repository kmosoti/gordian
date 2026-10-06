# B2: mutation checks

Exploration. Commands and results from this machine (cargo-mutants 27.1.0, cores 0-2 through the lab's
cargo wrapper, `--in-place` so that the worktree's own build cache is used and the source is restored
after every mutant; output directories were in the lab's scratch directory).

## The evaluator's notice scorer (rules N13 to N16 added to B1's N1 to N12)

```bash
cargo mutants -p gordian-stream-eval --file crates/gordian-stream-eval/src/notice.rs --in-place
# Found 80 mutants to test
# 80 mutants tested in 7m: 78 caught, 2 unviable
```

| | count |
|---|---|
| Mutants generated | 80 (B1's file had 41; the new fields, the site check and the precision methods add 39) |
| Caught (a test failed) | 78 |
| Missed (all tests passed) | 0 |
| Timed out | 0 |
| Unviable (does not compile) | 2 |

The two unviable are B1's: `score_notices -> Ok(Default::default())` (`NoticeVerdict` has no
`Default`) and the `&&` of the `LabelOfUnknownIncident` let-chain (a let-chain cannot hold `||`); B1's
hand-applied equivalents of them stand (`b1-hand-mutants.csv`, mutants "labels of unknown incidents not
checked").

## Twenty hand-applied mutants of N13 to N16 (`scripts/b2_hand_mutants.py`, `b2-hand-mutants.csv`)

cargo-mutants does not change method names, drop a call or reorder checks. Each hand mutant is one
textual change to `notice.rs`, applied alone, with `tests/notice_fixtures.rs` and
`tests/notice_generated.rs` run after it and the source restored: the incident's site is the last
occupied service; an unsited incident has site 0; a notice is read against the first incident's site;
site-correct and anchor-and-site-correct are not accumulated per incident (the last notice decides); a
missing site is right; site compared with `<=`; both is `||`, site only, anchor only; a notice on
background is site-correct when it has a site; incidents counted in the wrong tier total; notices
counted by the wrong flag; notices on incidents omit decoys; precision in a tier reads another tier's
count; strict precision reads the site-correct count; the precision of a stream with no notice is zero.
**20 of 20 caught.** The first run of the list had **one survivor**, "an incident that occupies nothing
has site 0": the fixture that pins "a missing site on either side is never site-correct" paired the
unsited incident with a notice about service 3, which that mutant also rejects. The fixture's notice is
now about service 0 and the whole list was rerun (the table above is that rerun).

## `ReanchorNoticer` (`arms/noticer_reanchor.rs`)

```bash
cargo mutants -p gordian-run --file crates/gordian-run/src/stream/arms/noticer_reanchor.rs --in-place \
  -- --test stream_reanchor --test stream_noticer
```

```text
# Found 35 mutants to test
# 35 mutants tested in 12m: 4 missed, 29 caught, 2 unviable
```

The first run **missed four**: `Noticer::score` replaced by 0.0, 1.0 and -1.0, and `refresh` replaced by
`()`. The noticer delegates both to the rung's; no test read the score a rule may see
(`AnomalyView::score`) or the peak it has had. `a_burst_with_no_stray_is_anchored_as_the_rung_anchors_
it` now asserts that both equal the rung's, at the step of the notice (a value at or above the
threshold) and later, when the burst has left the score window and the score has fallen below the peak.
The four mutants, rerun with the same tests (`-F 'score|refresh'`): **4 caught**. Final: **35 mutants,
33 caught, 2 unviable, 0 missed.** The two unviable: `notice -> vec![Default::default()]` and
`anomalies -> Vec::leak(vec![Default::default()])` (`Notice` and `Tracked` have no `Default`, so these do not compile; I did not
hand-apply equivalents. Every test of the noticer reads what `notice` and `anomalies` return, so a
constant would fail them, but that is a reading and not a run).

## What this does not show

Mutation testing shows that the tests notice changes to the code that exists. It does not show that
N13 to N16 are the right rules (the judgements are in `RULES.md`), and the fixtures and the code were
written from the same understanding, so a shared misreading would pass both. The generated-stream tests
are the check against the generator for N13's reading of the site.
