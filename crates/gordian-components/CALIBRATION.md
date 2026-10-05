# Calibration of the components' declared costs

What was measured, on what, with which result, and where the declared cost is wrong. Every
number below was produced by the commands in section 1 on the date and machine given there; the
claims in section 6 about content dependence were also measured. This is the calibration made for
work item A5b, which changed two things at once: the release profile of the workspace (section 7
of the previous version of this file said it would move every constant) and the world's
consistency checker behind the verifier. Section 8 separates the two effects as far as the
measurements allow.

## 1. Setup

| | |
|---|---|
| Date | 2026-10-04 |
| CPU | Intel(R) Xeon(R) Processor @ 2.80GHz, 4 vCPU virtual machine, Linux 6.18.44 |
| Toolchain | rustc 1.98.0 |
| Profile | `bench` (optimized, inherits `release`), with the workspace release settings: `debug = 1`, `lto = "thin"`, `codegen-units = 1` |
| Pinning | core 2, by the cpuset of `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100 --memory 2G` (cgroup v1 here). `taskset` was not used and was not refused: the sandbox this work ran in allowed both |
| Load | nothing else was running on the machine (the load average stayed at about 1.0, the benchmark itself); no build ran during a measurement. The earlier calibration shared the machine with another worker |
| Runs | four full runs of the benchmark, one after the other, each followed by the calibration test; about 3 min 40 s each. The test after run 1 ran against the previous constants and failed, as expected under a new profile (declared 1.4 to 2.0 times measured); only its JSON of pool features and medians was used |

The benchmark and the test binaries were built first (`cargo bench -p gordian-components --no-run`,
`cargo test -p gordian-components --release --test cost_calibration --no-run`) and then the
built binaries were run under the runner, so that no `rustc` ran during a measurement:

```bash
cargo bench -p gordian-components --no-run
cargo test -p gordian-components --release --test cost_calibration --no-run
scripts/cgroup-run.sh --name calib-bench --cpus 2 --cpu-quota 100 --memory 2G -- \
  target/release/deps/components-<hash> --bench
scripts/cgroup-run.sh --name calib-test --cpus 2 --cpu-quota 100 --memory 2G -- \
  target/release/deps/cost_calibration-<hash> --ignored --test-threads=1 declared_cost
python3 crates/gordian-components/calibrate.py target/cost-calibration.json
```

The acceptance test does no timing of its own: it reads `median.point_estimate` from criterion's
`estimates.json` and compares it with the declared cost. Criterion settings: 1 s warm-up, 3 s
measurement, 60 samples. The two timing tests of section 6 are plain loops and were run under
the runner the same way (`--ignored --test-threads=1 --nocapture content_envelope`, and
`class_spread`).

## 2. What the benchmarks run

One benchmark iteration runs one component once on each of 22 windows (two generated episodes of
each of the 11 classes), so the number criterion reports is the cost of one run averaged over
the class mix, times 22. The declared cost compared with it is the sum of `declared_cost` over
the same 22 windows. A window of `n` observations is the episode's stream cut at `n`, or repeated
with shifted instants when the stream is shorter (`benches/workload/mod.rs`).

Sweeps (each a benchmark group, each id a row in section 4):

- `<component>_n`: n = 16, 64, 256, 1024; services as generated (4 to 12).
- `<component>_s`: 4, 8, 12 services, n = 256.
- `memory_r`: 0, 16, 128, 1024 extra records added to the generated 12, n = 64. The extra records
  cannot match any window, so the number of records read changes and the number of entries
  emitted does not.
- `<component>_h`: n = 32, 128, 512, 2048. **Held out of the fit**: the constants were fitted
  to the other groups, and these rows score the model on three sizes between fitted ones and one
  beyond the largest.

## 3. Declared cost models

All in `Resource::Compute`, nanoseconds. `n` is the number of observations in the window, `s`
the number of services, `r` the number of prior records read.

| component | declared cost | constants in code |
|---|---|---|
| heuristic | `365 + 2.14 n` | `A_NS = 365`, `B_PS = 2_140` |
| estimator | `485 + 2.93 n + 101 s` | `A_NS = 485`, `B_PS = 2_930`, `C_PS = 101_000` |
| prior-record lookup | `275 + 2.14 n + 2.71 r` | `A_NS = 275`, `B_PS = 2_140`, `C_PS = 2_710` |
| verifier | `415 + 5.77 n + 156 s` | `A_NS = 415`, `B_PS = 5_770`, `C_PS = 156_000` |

Slopes are stored in picoseconds per unit so that costs of a few nanoseconds per observation keep
their precision in integer arithmetic. The constants are rounded from the means of the fits of
runs 1 to 3 (section 5). Run 4 was not used for them: it is the check.

**Shape.** The measured cost is affine in `n` for all four components over 16 to 2048
observations, and linear in `s` for the estimator and the verifier. For the verifier a term in
`n * s` was tried again in all four runs: with and without it the fit leaves the same worst
rows (the minimum and maximum of predicted / median agree to three decimals), and its
coefficient is not stable (3.9, -0.3, 0.7 and 3.9 ns per observation-service in runs 1 to 4, with
the per-observation and per-service coefficients negative in two of them), so it is collinearity
with `n` and `s` over this pool and not a cost. It is not in the model. The lookup is linear in the records read. Section 6
says what "affine in n" does not cover.

**The verifier now calls an optimized checker.** `physics::consistent_hypotheses` is linear in
`n` on every input shape (gordian-world `DESIGN.md`, section 8). The previous verifier model was
fitted to generated streams only, because the late-anchor shape was quadratic and generated
streams never contain it; the new model is fitted to the same generated streams but, unlike the
old one, is not contradicted by the shape it was never fitted to (section 6).

**Record reads are charged in `Compute` only.** The lookup declares no `Resource::Memory` charge:
core defines that resource as peak or integrated resident memory, which is not what a read of a
record costs, and the `2.71 r` term already carries the reads.

**The lookup's intercept** includes the cost of encoding an entry in the share of windows where a
record matched (18 of 22 in this pool, 17 of 22 at n = 16). A window with no match costs less
(section 6).

## 4. Declared against measured, run 4

Declared cost within 25% of the criterion median at every row is the acceptance criterion; the
test passes on this run. Run 4 supplied no data to the constants. Range of declared / median per
component: heuristic 0.85 to 0.99, estimator 0.89 to 1.02, lookup 0.90 to 1.03, verifier 0.84
to 1.14. The largest deviations are the verifier at `h/32` (+14%) and `h/128` (-16%), and the
heuristic at `h/128` (-15%). The mean of declared / median over all 48 rows is below 1 (0.96):
run 4 is slower than the runs the constants came from (section 5).

| benchmark | windows with an entry | median (us) | declared (us) | declared / median |
|---|---:|---:|---:|---:|
| heuristic_n/16 | 22/22 | 9.15 | 8.78 | 0.959 |
| heuristic_n/64 | 22/22 | 11.40 | 11.02 | 0.967 |
| heuristic_n/256 | 22/22 | 20.59 | 20.06 | 0.975 |
| heuristic_n/1024 | 22/22 | 56.88 | 56.23 | 0.989 |
| estimator_n/16 | 22/22 | 27.61 | 28.04 | 1.016 |
| estimator_n/64 | 22/22 | 32.02 | 31.15 | 0.973 |
| estimator_n/256 | 22/22 | 45.42 | 43.53 | 0.958 |
| estimator_n/1024 | 22/22 | 94.89 | 93.03 | 0.980 |
| memory_n/16 | 17/22 | 7.57 | 7.50 | 0.991 |
| memory_n/64 | 18/22 | 9.92 | 9.75 | 0.983 |
| memory_n/256 | 18/22 | 20.91 | 18.79 | 0.899 |
| memory_n/1024 | 18/22 | 53.34 | 54.96 | 1.030 |
| verifier_n/16 | 22/22 | 34.96 | 36.43 | 1.042 |
| verifier_n/64 | 22/22 | 42.30 | 42.52 | 1.005 |
| verifier_n/256 | 22/22 | 70.06 | 66.90 | 0.955 |
| verifier_n/1024 | 22/22 | 182.48 | 164.38 | 0.901 |
| heuristic_s/4 | 22/22 | 21.55 | 20.06 | 0.931 |
| heuristic_s/8 | 22/22 | 21.50 | 20.06 | 0.933 |
| heuristic_s/12 | 22/22 | 21.25 | 20.06 | 0.944 |
| estimator_s/4 | 22/22 | 36.43 | 36.06 | 0.990 |
| estimator_s/8 | 22/22 | 44.87 | 44.95 | 1.002 |
| estimator_s/12 | 22/22 | 53.58 | 53.83 | 1.005 |
| memory_s/4 | 18/22 | 20.23 | 18.79 | 0.929 |
| memory_s/8 | 18/22 | 19.42 | 18.79 | 0.968 |
| memory_s/12 | 18/22 | 20.28 | 18.79 | 0.926 |
| verifier_s/4 | 22/22 | 59.32 | 55.35 | 0.933 |
| verifier_s/8 | 22/22 | 70.43 | 69.08 | 0.981 |
| verifier_s/12 | 22/22 | 84.03 | 82.81 | 0.986 |
| heuristic_h/32 | 22/22 | 10.64 | 9.53 | 0.895 |
| heuristic_h/128 | 22/22 | 16.62 | 14.04 | 0.845 |
| heuristic_h/512 | 22/22 | 34.24 | 32.12 | 0.938 |
| heuristic_h/2048 | 22/22 | 108.19 | 104.43 | 0.965 |
| estimator_h/32 | 22/22 | 32.82 | 29.08 | 0.886 |
| estimator_h/128 | 22/22 | 38.59 | 35.28 | 0.914 |
| estimator_h/512 | 22/22 | 61.98 | 60.03 | 0.969 |
| estimator_h/2048 | 22/22 | 165.98 | 159.03 | 0.958 |
| memory_h/32 | 17/22 | 8.28 | 8.25 | 0.997 |
| memory_h/128 | 18/22 | 13.55 | 12.76 | 0.942 |
| memory_h/512 | 18/22 | 32.01 | 30.84 | 0.964 |
| memory_h/2048 | 18/22 | 108.51 | 103.16 | 0.951 |
| verifier_h/32 | 22/22 | 33.62 | 38.45 | 1.144 |
| verifier_h/128 | 22/22 | 60.37 | 50.64 | 0.839 |
| verifier_h/512 | 22/22 | 111.28 | 99.39 | 0.893 |
| verifier_h/2048 | 22/22 | 325.26 | 294.35 | 0.905 |
| memory_r/0 | 18/22 | 10.09 | 9.75 | 0.966 |
| memory_r/16 | 18/22 | 11.25 | 10.69 | 0.950 |
| memory_r/128 | 18/22 | 18.00 | 17.38 | 0.966 |
| memory_r/1024 | 18/22 | 70.83 | 70.80 | 0.999 |

## 5. Stability of the fit

`calibrate.py` fits each model by least squares on relative error (each row weighted by one over
its median). Four runs of the benchmark gave these fits; the constants in section 3 are rounded
from runs 1 to 3, not from any one. (`a` in ns, slopes in ps per unit.)

| run | heuristic `a`, `b` | estimator `a`, `b`, `c` | lookup `a`, `b`, `c` | verifier `a`, `b`, `c` |
|---|---|---|---|---|
| 1 | 350, 2079 | 476, 2863, 98795 | 271, 2104, 2700 | 436, 5606, 148277 |
| 2 | 375, 2184 | 529, 2857, 99291 | 276, 2166, 2705 | 255, 5981, 176280 |
| 3 | 371, 2157 | 454, 3060, 105027 | 279, 2143, 2729 | 549, 5712, 145151 |
| 4 (check) | 383, 2223 | 507, 3021, 97994 | 285, 2233, 2717 | 459, 6586, 139292 |
| constants | 365, 2140 | 485, 2930, 101000 | 275, 2140, 2710 | 415, 5770, 156000 |

Against runs 1 to 3 the constants predict every row, held-out rows included, within 0.90 to 1.18
of the measured median (verifier `h/2048` at 1.18 in run 1; every other row of every component
within 0.90 to 1.11). Against run 4 the range is 0.84 to 1.14 (section 4).

**The machine drifted during the session, upward in run order.** The median of the per-row ratio
of each run's medians to run 1's is 1.00, 1.02, 1.04, 1.06 for runs 1 to 4, and individual rows
moved by much more: the verifier at `h/2048` took 249, 268, 324 and 325 us in runs 1 to 4 (+31%)
while the heuristic at `n/1024` took 51.9, 55.9, 54.4 and 56.9 us (+10%). The verifier's long
windows are the most sensitive to whatever changed. With four runs in one order, drift and noise
cannot be told apart, so the verifier slope (5.6 to 6.6 ns per observation, a 17% spread) and
its intercept (255 to 549 ns) are the least certain constants here. Averaging runs 1 to 4 instead
of 1 to 3 would move the verifier slope by 4% and the other slopes by about 1%. A busy or drifting
machine is the likeliest reason for the calibration test to fail on a rerun without anything in
the code having changed. The tolerance of 25% was not changed.

## 6. Where the declared cost is wrong

A declared cost that depends on sizes only cannot track content. Two measurements, both on
core 2 under the runner, both at n = 256, after the constants of section 3 were in the code.

**By episode class** (`class_spread_at_window_size_256`; a plain timing loop against the declared
cost for that class's windows). Declared over measured, per class. Three runs: in the first the
loop measured 15 to 35% slower than in the other two (cause unknown), so the ranges below are
from runs 2 and 3, which agree to within 0.03 at every extreme except the lookup's low end (0.66
and 0.84; the lookup's windows take about a microsecond, so single classes are noisy).

| component | range over the 11 classes | worst classes |
|---|---|---|
| heuristic | 0.85 to 1.18 | NoFault 1.18, QuietUrgent 1.15; FeedbackBait 0.85 |
| estimator | 0.71 to 1.27 | NoFault 1.27, QuietUrgent 1.08; Duplicates 0.71 to 0.74 |
| lookup | 0.66 to 1.75 | NoFault 1.73 to 1.75 (no record matches, so no entry is built), JointlyDecisive 1.50 to 1.51; DelayedConfigChange 0.66 in run 2 |
| verifier | 0.77 to 1.36 | QuietUrgent 1.36, DelayedConfigChange 1.31; NoFault 0.77 |

So per class the heuristic stays inside 25% of the measured cost, the estimator leaves it at the
extremes (0.71 and 1.27), and the lookup (1.75) and the verifier (1.36) leave it on the high
side: for the classes that produce little (NoFault for the lookup, QuietUrgent for the verifier)
the declared cost is well above the measured one. The pooled number in section 4 is right on average over a class mix that weights each class
equally and that no experiment will use.

**By informative density** (`content_envelope_at_window_size_256`; measured over declared at the
same size):

| component | declared (us) | all benign | all informative, anchor first | informative, anchor late |
|---|---:|---:|---:|---:|
| heuristic | 0.91 | 0.74x | 1.02x | 1.01x |
| estimator | 2.14 | 0.67x | 1.55x | 1.52x |
| lookup | 0.85 | 0.44x | 0.70x | 0.72x |
| verifier | 3.30 | 1.53x | 2.70x | 2.52x |

"Benign" is counters below the alarm threshold; "informative" is alarms the rules can
discriminate on. The generated classes are mostly noise, so the pooled calibration sits near the
benign end for the estimator and verifier. The table is the first of four runs of this test. For
the verifier the four runs gave 1.37 to 1.53x (benign), 2.70 to 3.02x (anchor first) and 2.52 to
2.76x (anchor late); for the estimator 0.67 to 0.73x, 1.46 to 1.64x and 1.40 to 1.52x. (Before the change, the same table read 24.31x for
the verifier on the late anchor against 2.32x with the anchor first; those figures were measured
on the previous profile and constants and are given only as the size of the effect.)

**The verifier is linear on the late anchor now.** The last column is the shape that made the
reference checker quadratic. The verifier's cost on it is 2.5 to 2.8x its declared cost over four runs, no longer
above its cost with the anchor first (2.7 to 3.0x); the two overlap within the run-to-run
spread, which is about 10%. What is left is content dependence of the ordinary kind: a window in which every
observation is an alarm costs 2.5 to 3.0 times the pooled declared cost, because the pooled
model is fitted to windows that are mostly benign. An experiment whose windows are denser in
alarms than the generated pool will see declared cost below measured cost for the verifier by
that factor. The measured cost, not the declared one, is the cost in every experiment. (Superseded
by work item A8b: the cost in every experiment is now the *modelled* cost of section 9, a count of
the work done, which follows content where the declared cost follows only size. Measured wall time
stays as the secondary check.)

## 7. Recalibrating

Rerun section 1 after a change to the CPU, the toolchain, the workspace `[profile]`, or the code
of a component or of the checker. Fit with `calibrate.py`, round, edit the constants and the
comment above them in the component's source, and rerun the test. Do not edit the tolerance to
make a test pass. Fit on several runs, check on one more, and say in the file what ran when.

## 8. What the A5b change moved, as far as it can be told

Two changes landed together, so the old constants cannot be compared with the new ones as if one
thing had happened.

**The profile.** The same benchmark source, built with the old settings (no `[profile]` section:
no LTO, 16 codegen units, no debug info) and run in this session on core 2, gave for the
heuristic at `n` = 16, 64, 256, 1024: 11.25, 14.32, 27.52, 79.15 us. With the new settings, in the
same session: 8.23, 10.60, 19.77, 51.90 us (run 1), that is 0.73, 0.74, 0.72 and 0.66 of the old
build's time. The heuristic does not call the checker, so this is the effect of LTO and one
codegen unit on the component code. The checker on its own gains little: the reference
`consistent_hypotheses` on the late anchor at n = 2048 took 4,084 us under the old settings and
3,809 us under the new (-7%), measured by `benches/checker.rs`.

**The session.** The old settings in this session gave the heuristic times 0.74, 0.66, 0.76 and
0.87 of those recorded by the previous calibration (15.26, 21.86, 36.44, 90.98 us), which also
used the old settings. So the machine was 15 to 50% faster in this session than in that one, on
the same code, by an amount that varies with `n`. The cause is not known (the previous run
shared the machine with another worker; the hypervisor's allocation may also differ between
sessions). It is why a constant from one session cannot be trusted in another, and one more
reason why the measured cost is the experiments' cost.

**The checker.** The verifier's slope fell by 23% (7.5 to 5.77 ns per observation) against 42%
for the heuristic, 44% for the lookup and 45% for the estimator. The verifier is the one component
whose hot loop is the checker, which gained little from the profile, so a smaller fall is what
the profile alone would give, and the late-anchor shape no longer affects it. What the
measurements cannot say is how much of the verifier's fall is the session and how much the
profile. The part that can be said is that the optimized checker costs 0 to 20% more than the
reference on the shapes where the reference was already linear (`DESIGN.md`, section 8.4 of
gordian-world), and that the verifier's constants include that.

## 9. Counted operations and their weights (work item A8b)

Sections 1 to 8 are the declared cost: what a policy is charged and the `Bill` enforces. This
section is the cost an experiment reports, the charter's `C`: a count of the work each call did,
weighted. Everything below was produced by the commands in 9.9 on 2026-10-05. The data files are
scratch (4 to 70 MB each) and are not kept; the scripts that read them are
`crates/gordian-run/calibrate_ops.py`, `calibrate_insitu.py` and `apply_weights.py`.

### 9.1 Why a count, and what it is

On this VM wall time carries bursts of stolen CPU time that hit one copy of an episode and not the
other (`docs/review-log.md`, A8): two of four A/A runs excluded zero. A count of the work done does
not move. Each component returns, with its output, the number of times it did each of a few
named things (`Component::run_counted`, `src/ops.rs`); the shared rule counts the same way
(`RULE_UNITS` in `gordian-run/src/policy/decide.rs`, `Policy::take_ops`); the world's checker has
a counted variant that the plain function wraps (`physics::consistent_worlds_counted`, with
`CheckerOps`), and its reference functions are unchanged. A count is a function of the input and
the component's configuration. A policy is never handed one and cannot set one
(`gordian-run/tests/counted_ops.rs`). `results.csv` records, per episode, `ops_component` and
`ops_sched` (sums of counts, a sanity check: the units differ) and `modelled_component_ns` and
`modelled_sched_ns` (counts times weights, summed once per episode). The charter's `C` is their
sum, `modelled_cost_ns` in the analysis package. It covers what a policy controls (which
components ran, and what the rule did with them) and not the harness's own work (generating the
episode, the simulator, the ledger), which is in `measured_harness_ns` only.

### 9.2 The units

A unit is a loop (or a fixed overhead) the code runs a number of times that depends on content.
Each was counted where the code does it, not derived from the input size. The tests next to the
components check each unit on windows where the answer can be worked out, and that a count follows
content (two windows of one size and one graph that make a component work differently are counted
differently).

| component | unit | counts |
|---|---|---|
| heuristic | `calls` | one per call: the summary's allocations, the entry's envelope |
| | `scanned` | observations the one pass over the window looked at |
| | `rules` | rows of the rule table looked at before one decided (up to 15) |
| | `mask_steps` | steps of the dependents-mask construction when the upstream-site rule locates a site (no generated window reaches this rule: the site's own message decides first; it is calibrated on hand-built windows) |
| | `entries`, `ranked` | entries emitted (0 or 1); candidates written into it |
| estimator | `calls` | one per call: its entry always lists the top five |
| | `scanned` | observations looked at |
| | `permit_scans` | informative observations, each scanning five fault kinds' permit tables |
| | `ancestors`, `site_updates` | ancestors examined to find the sites an observation is anchored at; anchored sites updated |
| | `probe_evals` | `probe_result` calls made to judge a probe result against every hypothesis |
| | `hypotheses` | hypotheses scored, `1 + 5 * services` (the ancestor sets scale with it) |
| | `sort_cmps` | comparisons made by the ranking sort, which is cheap when every score is equal and dear when the evidence separates them: the unit that made the estimator's fit work |
| memory | `calls`, `scanned` | one per call; observations looked at |
| | `records` | prior records compared with the window's signature |
| | `entries` | entries emitted (0 or 1) |
| verifier | `candidates`, `damaged` | exactly one of the two per call: the envelope of the entry, a list of candidates, or the shorter damaged-evidence entry when the window lost its anchor |
| | `scanned` | observations touched: the copy into the checker's slice, and the checker's first pass |
| | `worlds`, `evals`, `probe_evals` | the checker's counts: candidate worlds tried; evaluations of a world against an observation; against a probe result |
| | `ranked` | hypotheses written into the entry (the whole consistent set) |
| rule | `calls` | one per `decide` or final call: the step's fixed cost (cost declaration, selection, decision) |
| | `decoded_outputs`, `decoded_ranked` | component outputs decoded; candidates in them. An output that arrives byte for byte equal to the one held for its component is not decoded (work item A6c, 9.10) |
| | `compared_bytes` | payload bytes of an arriving output compared with the held copy (added by A6c) |
| | `worlds` | worlds built from a candidate set, and visited when probes are scored |
| | `probe_evals` | `probe_result` calls made to score candidate probes |

Units that the data could not tell apart were dropped, not kept at weight zero: the checker's
`mask_steps` (counted, tested, not priced by the verifier: a function of the graph alone, not
separable from `worlds`), the verifier's and the estimator's separate per-call and graph terms, the
memory lookup's `ranked` and `record_tags`, and for the rule the scan for bought probes (0.3 ns an
observation), the check of bought probes against a world, the grouping of worlds by a probe's
result and the per-probe set-up (each under 2% of the time; dropping all four moves no fit by more
than 0.0002 in R^2). Not counted at all: the selectors' own work (a few random draws, a period
test), the rule's checks of corrections (it never buys one), and the clock reads (a pair costs 25
to 75 ns here).

### 9.3 Setup

| | |
|---|---|
| Date | 2026-10-05 |
| CPU | the VM reports `Intel(R) Xeon(R) Processor @ 2.10GHz`, 4 vCPU, kernel 6.18.44. Sections 1 to 8 say `@ 2.80GHz` for 2026-10-04: **the host is not the one the declared cost was fitted on**, which is one more reason the declared constants are not reused |
| Toolchain, profile | rustc 1.98.0; the `release`/`bench` profile of the workspace (`debug = 1`, thin LTO, one codegen unit) |
| Isolation | `scripts/cgroup-run.sh --cpus 2 --cpu-quota 100 --memory 2G` (cgroup v1); nothing else was running and no build ran during a measurement. `/proc/stat` over the five hot runs shows 69 to 90 steal ticks of about 34,000 (0.2 to 0.3% of the machine's time), and 7 to 12 of about 3,800 over the three in-situ runs: the host does steal time here, in bursts, which is the reason for taking minima |
| Hot calibration | `examples/calibrate_ops.rs`, five runs of 84 to 85 s, 25 timings per datum, each timing about 300 microseconds of calls; the datum's time is the minimum over all 125 |
| In-situ calibration | `examples/calibrate_insitu.rs`, three runs of 9 to 10 s, five passes each, so 15 plays of every episode; the call's time is the minimum over them |

### 9.4 Do the counters track the work? Fixed windows, timed in a loop

The program times every component, and the rule, on windows built to cover every class and a range
of sizes, each timed as the minimum over repeated loops:

- `fit`: three generated episodes of each of the eleven classes, the stream cut (or repeated) to 0,
  1, 2, 4, 8, 16, 32, 64, 128, 256, 512 and 1,024 observations; two episodes per class with 2, 6 and
  12 probe results appended (taken from the real simulator); two episodes per class over 12 to
  1,036 prior records (the lookup only); every stream admitted into windows of 8, 16, 32 and 64 (so
  that the recency rule evicts the anchor and the verifier answers some with the damaged entry); and
  the hand-built upstream windows. 674 windows per component (762 for the lookup).
- `heldout`: three *other* episodes of each class at sizes the fit never saw (3, 6, 12, 24, 48, 96,
  192, 384, 768, 1,536), with the same probe, record, eviction and upstream variants. 612 windows
  (700 for the lookup).
- `real`: 1,368 states recorded from episodes played by the real harness (four arms, the default
  limits and a 250 microsecond compute budget, seeds 104 to 106), each component timed on the
  window it saw. The rule has no fixed windows, since its input is a candidate set that only
  episodes make: it is fitted on states recorded from seeds 100 to 103 (1,999 calls, every call in
  which it bought a probe kept) and judged on seeds 104 to 106 (1,443 calls).

Weights are a non-negative least squares fit of the minimum time on the counts, weighted by 1/time
so that the fixed costs of small windows count as much as the slopes of large ones, with no
intercept beyond the explicit per-call unit. The hot weights, rounded to three figures, in
nanoseconds per operation:

| component | weights (ns per unit) |
|---|---|
| heuristic | calls 20.7, scanned 1.90, rules 4.49, mask_steps 3.11, entries 196, ranked 35.2 |
| estimator | calls 563, scanned 2.47, permit_scans 5.74, ancestors 0.604, site_updates 2.38, probe_evals 6.39, hypotheses 1.17, sort_cmps 3.23 |
| memory | calls 19.5, scanned 1.62, records 2.84, entries 301 |
| verifier | candidates 262, damaged 34.4, scanned 1.54, worlds 7.44, evals 6.71, probe_evals 21.6, ranked 61.5 |
| rule | calls 70.9, decoded_outputs 223, decoded_ranked 110, worlds 15.4, probe_evals 27.6 (A8b; superseded by 9.10: calls 83.9, decoded_outputs 266, decoded_ranked 115, worlds 14.6, probe_evals 24.2, compared_bytes 0.0325) |

Across the five runs the well-determined weights move by 1 to 6% (heuristic `scanned` 1.93 to 1.96,
`entries` 202 to 205; estimator `sort_cmps` 3.28 to 3.35; memory `records` 2.83 to 2.87; verifier
`ranked` 64 to 68; rule `decoded_ranked` 109 to 115). Weights that are functions of the same size
are not individually determined and move a lot: the verifier's `damaged` (9 to 53 ns) and `worlds`,
the estimator's `ancestors`, `site_updates` and `permit_scans`, the rule's `probe_evals`. Their sum
predicts well; the table claims no more for them than that. Validity, weights as rounded above (R^2
of the weighted counts against the minimum time per call; relative errors are predicted over
measured):

| component | set | calls | R^2 | median abs. rel. | 90th pct | mean rel. | median abs. error | worst class (mean rel.) |
|---|---|---:|---:|---:|---:|---:|---:|---|
| heuristic | fit | 674 | 0.995 | 2.6% | 7.0% | -0.2% | 10 ns | JointlyDecisive +4.3% |
| | heldout | 612 | 0.996 | 2.7% | 7.8% | -0.1% | 12 ns | JointlyDecisive +5.2% |
| | real | 1,368 | 0.993 | 2.7% | 4.6% | +2.5% | 7 ns | JointlyDecisive +4.9% |
| estimator | fit | 674 | 0.994 | 2.4% | 6.1% | -0.1% | 31 ns | StaleMemory +3.3% |
| | heldout | 612 | 0.994 | 2.6% | 5.8% | -0.5% | 41 ns | JointlyDecisive +3.0% |
| | real | 1,368 | 0.990 | 3.2% | 4.9% | +2.9% | 23 ns | DelayedConfigChange +3.9% |
| memory | fit | 762 | 0.988 | 6.5% | 20.4% | -1.2% | 11 ns | JointlyDecisive -7.0% |
| | heldout | 700 | 0.983 | 5.8% | 15.8% | +0.6% | 15 ns | JointlyDecisive -6.2% |
| | real | 1,368 | 0.994 | 10.2% | 21.1% | -0.5% | 4 ns | DelayedConfigChange -20.4% |
| verifier | fit | 674 | 0.981 | 5.0% | 11.1% | -0.4% | 101 ns | DelayedConfigChange +6.8% |
| | heldout | 612 | 0.984 | 4.6% | 10.0% | -0.7% | 98 ns | DelayedConfigChange +5.3% |
| | real | 1,368 | 0.989 | 2.9% | 8.7% | -0.2% | 82 ns | FeedbackBait +2.2% |
| rule (A8b; redone in 9.10) | fit (real states) | 1,999 | 0.996 | 2.8% | 11.8% | -0.2% | 76 ns | DelayedConfigChange +1.2% |
| | real (held-out seeds) | 1,443 | 0.997 | 2.2% | 7.7% | -0.1% | 43 ns | DelayedConfigChange -1.0% |

**Every component and the rule pass R^2 >= 0.9 on every set; the lowest is 0.981.** The bar was not
lowered and no class was dropped. In development the first versions of the counters did not pass
on the real states: the estimator's cost was dominated by the comparisons of its ranking sort,
which a count of observations and hypotheses cannot see, and the verifier's by the size of the entry
it emits; both became units. The worst class is the one with the largest mean relative residual
among the eleven (the hand-built upstream windows are a source, not a class).

**Residual shape.** By window size the mean relative residual stays within 2% for the heuristic and
the estimator on the fit and held-out sets (the largest, +1.9%, at 512 or more observations),
within 5.3% for the verifier (slightly low at 32 to 127 observations, slightly high beyond 512),
and shows the memory lookup's: -11% at 0 to 7 observations and +11% at 8 to 31 on the fit set.
That is an absolute error of about 4 ns on a call of 25 to 45 ns (the lookup returns early when the
window shows no symptom, and the count cannot see that the first few observations are mostly
benign), which is why its median relative error is 10% on real states while its median absolute
error is 4 ns. By source of the window (fit set): generated prefixes -0.5% to +1.2% for the four
components other than the lookup; windows with probe results -0.4% to -4.5% (the verifier is the
low one); evicted windows +1.1% (heuristic), +1.3% (estimator), -1.8% (verifier), +11% (lookup);
the hand-built upstream windows +0.3% (heuristic, which is what they are for), -5.6% (estimator),
+3.9% (verifier). On the real states the heuristic and the estimator are overpredicted by 2.5% and
2.9% in every class (+1.7% to +4.9%), the verifier is within 2.2% in every class, and the rule
within 1.2%.

### 9.5 The first non-identical-arm check failed, and what the weights of C are

The check of plan A8b (`heuristic_only` against `all_components`, interleaved, 20 seeds by 11
classes, through the driver) was made first with the weights of 9.4. It failed in all three runs:
modelled ratio sum(B)/sum(A) 15.66 against a wall-time median-of-episodes ratio of 12.13 (90%
interval 11.61 to 13.06), 12.18 (11.82 to 12.80) and 12.10 (11.83 to 13.00). The counters were not
at fault: replaying the actual calls of both arms in a loop gave 1.01 to 1.04 times the modelled
time for the heuristic, the estimator and the verifier (1.31 for the lookup, 4 ns a call). The
model was a model of a loop. A call inside an episode costs more than that, and by different
factors: measured over modelled with the hot weights was 2.2 for the components' calls (the
heuristic's alone) and 1.9 for the rule's steps in `heuristic_only`, and 1.7 (four components) and
1.4 in `all_components`. The gap per call is roughly constant through an episode (it does not grow
with the ledger) and is not the timer: a pair of clock reads costs 25 to 75 ns here. Touching a
megabyte of memory between calls moves a call by 6 to 20%; touching four to sixteen megabytes (the
order of what an episode's ledger and JSON churn pass through) adds 0.4 to 1.8 microseconds to a
call, nearly the same for a 500 ns call as for a 1.6 microsecond one. (Those probes were ad hoc
and are not kept; they say which way the gap goes, not how big it is.) The cheaper arm pays
proportionally more, so the loop-calibrated ratio overstates the expensive arm.

The weights that ship are therefore the hot weights **scaled to what the harness pays**
(`calibrate_insitu.py`): for each component and for the rule, in-situ time = alpha x (hot weighted
count) + beta, fitted by non-negative least squares to the minimum in-situ time of each call over
thousands of calls. The in-situ calls are the harness's own timer entries (each now carries the
operation counts of the call it timed), replayed over episodes of four arms that select varied
subsets of the components (`random_matched` at p = 0.5 and 0.25, and two `fixed_pipeline`
configurations; seeds 100 to 129 for the fit, seeds 200 to 209 held out), taking for every call the
minimum over 15 plays. **The arms and episodes of the check (`heuristic_only`, `all_components`,
seeds 0 to 19) are in neither set.**

| | alpha | beta |
|---|---:|---:|
| heuristic | 1.606 | 20.0 ns |
| estimator | 1.388 | 23.6 ns |
| memory | 1.845 | 19.6 ns |
| verifier | 1.196 | 484 ns |
| rule | 1.208 | 196 ns (A8b; 9.10: 1.374 and 130 ns) |

The beta goes onto the per-call unit (`calls`; for the verifier both `candidates` and `damaged`,
exactly one of which counts per call). The shipped weights, in nanoseconds per unit, are in the
unit tables of the sources; the heuristic's, for instance: calls 53.3, scanned 3.06, rules 7.22,
mask_steps 5.00, entries 314, ranked 56.5. Judged on the in-situ calls, with the weights as rounded
in the code:

| component | single-call R^2: fit, held out, check arms | median abs. rel. error (check arms) |
|---|---|---:|
| heuristic | 0.71, 0.75, 0.74 | 10.7% |
| estimator | 0.82, 0.80, 0.75 | 7.8% |
| memory | 0.89, 0.86, 0.86 | 17.5% |
| verifier | 0.93, 0.91, 0.91 | 4.9% |
| rule | 0.96, 0.96, 0.96 | 15.0% |

**Single in-situ calls are scattered**: the minimum over 15 plays of a call that takes 300 ns to 8
microseconds still varies with the allocator's and the caches' state, so the R^2 of the model
against single in-situ calls is below 0.9 for the heuristic, the estimator and the lookup. That is
a property of the measurement, not a fault of the counters (9.4 tests those, in loops, and every
one passes), but it is a limit on what can be said call by call. What an experiment sums is
episodes, and there the model is close:

| set | arm | components, model / measured | rule | total | unscaled hot weights, total |
|---|---|---:|---:|---:|---:|
| fit | `pipeline_he` | 1.03 | 0.97 | 0.995 | 0.70 |
| fit | `pipeline_hvm_2` | 1.00 | 1.08 | 1.05 | 0.79 |
| fit | `random_25` | 0.93 | 0.99 | 0.97 | 0.72 |
| fit | `random_50` | 0.95 | 1.01 | 0.99 | 0.75 |
| held out | the same four arms | 0.92 to 1.01 | 0.97 to 1.08 | 0.97 to 1.05 | 0.70 to 0.79 |
| check arms | `all_components` | 0.99 | 1.09 | 1.05 | 0.80 |
| check arms | `heuristic_only` | 1.01 | 1.15 | 1.10 | 0.66 |

On the check's own calls the ratio of the two arms is 12.77 measured, 12.18 modelled, and 15.65
with the hot weights. The model prices `heuristic_only`'s rule steps 15% too high (its steps are
the cheapest in the set, where the fit arms' are dearer), which is most of the 5% by which the
modelled ratio is below the measured one.

### 9.6 The non-identical-arm check, with the weights that ship

Through `scripts/run-driver.sh` (cgroup v1, cores 0 to 2, driver pinned to core 3), three runs with
`--run-seed` 1, 2 and 3, and `gordian-analyze cost-check --a heuristic_only --b all_components
--seed 1` (the median over the 220 episodes of B_i/A_i of `measured_policy_ns`, paired bootstrap,
10,000 resamples):

| run | modelled ratio, sum(B)/sum(A) | wall-time ratio, median of episodes | 90% interval | inside |
|---|---:|---:|---|---|
| 1 | 12.18 | 12.24 | 11.68 to 12.97 | yes |
| 2 | 12.18 | 12.66 | 11.90 to 13.15 | yes |
| 3 | 12.18 | 12.48 | 11.73 to 13.03 | yes |

The modelled ratio is identical in the three runs, as it must be. The ratio of wall-time totals,
for reference, is 11.90, 11.70 and 11.94. The intervals are 10% wide, so the check separates a 29%
error from none and says little about a 5% one. Two things it does not show. On
`measured_total_ns`, the whole episode, the modelled ratio is *not* inside: the whole episode's
wall-time median ratio is 4.9 to 5.0, because the harness's own work, which `C` excludes, is in both
arms and dilutes it. And the weights have been shown to suit arms that run between zero and four
components per step; the heuristic-only rule steps show a 15% miss.

An A/A on the modelled cost is exactly zero (`analysis/tests/test_real_runs.py`, on the real A/A
fixture: the two copies have identical `modelled_cost_ns` in every episode, `S = 0` with a
zero-width interval, while their wall times differ).

### 9.7 What this does and does not establish

- The counters follow the work in the components and the rule on fixed windows over all classes
  and many sizes (R^2 0.98 to 0.997), on held-out episodes and window sizes, and on the states real
  episodes produce, with a worst-class bias of 7% (20% for a lookup call of 4 ns).
- `C` is a deterministic function of what the arm did. It does not move with host interference, it
  is the same in two copies of an arm, and a re-run reproduces it byte for byte.
- `C` agrees with in-situ wall time at the level of arm totals and of the ratio of two arms, to
  within the 5 to 10% seen on the arms tried, once the harness's per-call and per-step costs are in
  the weights. Those costs are properties of this harness on this host: a change to the harness's
  memory behaviour (the ledger, the payloads) changes them, and they are not an intrinsic cost of a
  component. A recalibration is needed after such a change.
- The weights are not intrinsic costs of an operation. Several are not individually determined
  (9.4). They are a model of this harness at this revision, as the declared constants were.
- Not tested: an arm that selects components by content (the selector EXP-001 will freeze), which
  changes the mix of calls; windows larger than the 256 of the default limits inside an episode
  (the fixed-window fits go to 1,536); a stress class that makes calls dearer than any generated
  one.
- That `C` excludes the harness's own work is a choice made here (the plan says "the charter's `C`
  becomes modelled cost" and states the check against "the wall-time ratio's median-of-episodes
  estimate" without saying which wall time). The wall time that matches is `measured_policy_ns`.

### 9.8 Reading a result

`gordian-analyze compare --relative-savings` defaults to `modelled_cost_ns`, labels it the
charter's `C`, and reports the measured wall time of the same pairs underneath, labelled a
secondary check (`analysis/README.md`). `gordian-analyze cost-check` is the check of 9.6.

### 9.9 Recalibrating

After a change to the CPU, the toolchain, the profile, the harness, or the code of a component, of
the rule or of the checker:

```bash
cargo build --locked --release --example calibrate_ops --example calibrate_insitu
for i in 1 2 3 4 5; do
  CALIBRATE_REPS=25 scripts/cgroup-run.sh --name calib-ops-$i --cpus 2 --cpu-quota 100 --memory 2G -- \
    target/release/examples/calibrate_ops > target/calib-ops-$i.jsonl
done
analysis/.venv/bin/python crates/gordian-run/calibrate_ops.py target/calib-ops-?.jsonl --json target/hot.json
for i in 1 2 3; do
  CALIBRATE_PASSES=5 scripts/cgroup-run.sh --name calib-insitu-$i --cpus 2 --cpu-quota 100 --memory 2G -- \
    target/release/examples/calibrate_insitu > target/calib-insitu-$i.jsonl
done
analysis/.venv/bin/python crates/gordian-run/calibrate_insitu.py target/hot.json target/calib-insitu-?.jsonl \
    --json target/weights.json
python3 crates/gordian-run/apply_weights.py target/weights.json --record "<what ran, and when>"
```

`calibrate_ops.py` exits 1 if any target fails R^2 >= 0.9 on any set: fix the counter, do not edit
the bar. `calibrate_insitu.py HOT.json RUNS --from-source` judges the constants that are in the
code. Run every measurement alone; a build must not overlap one. Regenerate the analysis fixtures
(`analysis/README.md`) after changing weights, since they hold `modelled_*_ns`.

### 9.10 The rule after work item A6c (decode each output once)

The shared rule recognises an output that arrives byte for byte equal to the one it holds and does
not decode it (`POLICIES.md`, section 3.1). That changes what the rule's calls do, so the rule's
units were re-measured. Nothing else was: the components' code, units and weights are those of
9.4 and 9.5, and `apply_weights.py --only rule` rewrote only the rule's table. Everything below was
produced by the commands of 9.9 on 2026-10-05, same host and setup as 9.3, after the change.

**A new unit and a new measurement.** `compared_bytes` counts the payload bytes the comparison of an
arriving output with the held one had to look at (all of them when the entries have the held ones'
shape; an upper bound when they differ, because the comparison stops at the first differing byte).
`calibrate_ops` used to time the rule by calling the same step in a loop, which now only ever
reaches the comparison path: after the first call the rule holds the outputs it is handed. It
therefore times each recorded state twice. One is the repeated call (comparison, no decoding). The
other alternates the outputs with a copy of them with one trailing space appended to every payload,
which decodes to exactly the same candidates but is never equal to the other, so that every call
decodes what it is handed, as a call that brings a new output does. Without the second timing the
fit would have seen no decoding at all and the decoding weights would have been undetermined. In
the fit data the rule has 4,282 `real_fit` rows (was 1,999) and 3,124 held-out `real` rows (was
1,443); the new ones are the decoding-path rows (source `episode_decode`).

**Hot weights** (five runs of 25 timings; ns per unit; in brackets the A8b figure):

| unit | weight | across the five runs |
|---|---:|---|
| calls | 83.9 (70.9) | 86.6 to 92.0 |
| decoded_outputs | 266 (223) | 269 to 288 |
| decoded_ranked | 115 (110) | 117 to 124 |
| worlds | 14.6 (15.4) | 15.0 to 15.9 |
| probe_evals | 24.2 (27.6) | 24.7 to 25.8 |
| compared_bytes | 0.0325 | 0.030 to 0.038 |

(The last column is the fit of each run alone; the first is the fit of the minimum over the five
runs, which is what is used.) `compared_bytes` carries 1.0% of the fitted time and
`decoded_ranked` 58%. A hit on 1 KB costs about 32 ns over a call that compares nothing, which
agrees with the weight. Validity, weights as fitted, R^2 of the weighted counts against the minimum
time per call: **rule, `real_fit` 4,282 calls R^2 0.9954 (median |relative error| 5.1%, 90th
percentile 15.9%, mean -0.7%, worst class DelayedConfigChange +4.0%); `real` held-out seeds 3,124
calls R^2 0.9966 (4.5%, 11.2%, -0.7%, worst class Duplicates -2.7%). The bar of A8b, R^2 >= 0.9, is
met on both.** The components, run again in the same five runs on the same windows, also met it
(lowest 0.983, memory heldout), and their fits are not applied.

**In situ** (three runs of five passes, 51,446 rule calls fitted): the rule is 1.374 x hot + 130 ns
per call (A8b: 1.208 x and 196 ns). Shipped weights, ps per unit: calls 246,000, decoded_outputs
365,000, decoded_ranked 158,000, worlds 20,000, probe_evals 33,200, compared_bytes 45. Single calls
(R^2 of the model against the minimum in-situ time of each call): rule 0.905 fit, 0.911 held out,
0.932 on the check arms (A8b: 0.96 on each); median |relative error| 8.3%, 8.4% and 17.9% on the check
arms. Totals per arm, model over measured, for the rule: 0.94 to 1.04 on the four fit and held-out
arms, 1.04 for `all_components` and 1.15 for `heuristic_only` (A8b: 1.09 and 1.15), the same
over-pricing of the cheapest rule steps as before. The judgement of the constants as shipped
(`calibrate_insitu.py --from-source`): `all_components` over `heuristic_only` on the check's own
calls, 10.44 measured, 9.77 modelled.

**The non-identical-arm check again** (`heuristic_only` against `all_components`, interleaved,
20 seeds by 11 classes, through `scripts/run-driver.sh`, `--run-seed` 1, 2 and 3,
`gordian-analyze cost-check ... --seed 1`): modelled ratio 9.77, identical in the three runs; the
wall-time ratio, median of episodes, 9.73 (90% interval 9.28 to 10.38), 9.54 (9.22 to 10.02) and 10.04
(9.53 to 10.41). **Inside in all three** (A8b: 12.18 against 12.24, 12.66, 12.48). The ratio fell
because the rule's steps in `all_components` became much cheaper (the verifier's output is no
longer decoded at every step), and the model followed. The same limits apply as in 9.6: intervals 8
to 11% wide, and a check on two arms.

**What was not refitted.** The declared cost's decoding constants (`POLICIES.md`, section 3): they
are what the bill enforces, were fitted on another host, and a refit would move every binding
budget; the comparison term is declared at the hot weight, 33 ps per byte.
