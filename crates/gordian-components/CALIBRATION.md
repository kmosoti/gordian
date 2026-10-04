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
that factor. The measured cost, not the declared one, is the cost in every experiment.

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
