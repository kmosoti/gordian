# Calibration of the components' declared costs

What was measured, on what, with which result, and where the declared cost is wrong. Every
number below was produced by the commands in section 1 on the date and machine given there; the
claims in section 6 about content dependence were also measured, and the one about worst cases is
labelled as such.

## 1. Setup

| | |
|---|---|
| Date | 2026-10-04 |
| CPU | Intel(R) Xeon(R) Processor @ 2.80GHz, 4 vCPU virtual machine, Linux 6.18.44 |
| Toolchain | rustc 1.98.0 |
| Profile | `bench` (optimized, inherits `release`). The workspace has no `[profile]` section yet |
| Pinning | `taskset -c 2` |
| Load | other work shares the machine (a second worker, evaluator mutation testing). `ps` showed no other cargo or rustc process when the recorded benchmark run started; the load average was about 1.0 during it |

```bash
taskset -c 2 cargo bench -p gordian-components
cargo test -p gordian-components --release --test cost_calibration -- --ignored --test-threads=1 declared_cost
python3 crates/gordian-components/calibrate.py target/cost-calibration.json
```

The acceptance test does no timing of its own: it reads `median.point_estimate` from criterion's
`estimates.json` and compares it with the declared cost, so the `taskset` wrapper matters for
`cargo bench` and not for the test. (The wrapper around `cargo test` was refused by the
sandbox this work ran in; the two timing tests of section 6 were run pinned from the built test
binary instead.) Criterion settings: 1 s warm-up, 3 s measurement, 60 samples.

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
  beyond the largest. (The constants were rounded after looking at the held-out residuals of an
  earlier run, which were all inside 14%; they were not adjusted to them.)

## 3. Declared cost models

All in `Resource::Compute`, nanoseconds. `n` is the number of observations in the window, `s`
the number of services, `r` the number of prior records read.

| component | declared cost | constants in code |
|---|---|---|
| heuristic | `685 + 3.7 n` | `A_NS = 685`, `B_PS = 3_700` |
| estimator | `950 + 5.3 n + 120 s` | `A_NS = 950`, `B_PS = 5_300`, `C_PS = 120_000` |
| prior-record lookup | `500 + 3.83 n + 3.75 r`, plus `Resource::Memory` `64 r` bytes | `A_NS = 500`, `B_PS = 3_830`, `C_PS = 3_750` |
| verifier | `750 + 7.5 n + 240 s` | `A_NS = 750`, `B_PS = 7_500`, `C_PS = 240_000` |

Slopes are stored in picoseconds per unit so that costs of a few nanoseconds per observation keep
their precision in integer arithmetic.

**Shape.** The measured cost is affine in `n` for all four components over 16 to 2048
observations (the 2048 held-out row is inside 12% for every component except the verifier, which
is at +18%), and linear in `s` for the estimator and the verifier. For the verifier a term in
`n * s` was tried; its fitted coefficient changed sign between runs and its removal changed no
residual, so it is not in the model. The lookup is linear in the records read. Nothing here is a
line forced through a curve; but see section 6 for what "affine in n" does not cover.

**The `Memory` charge** of the lookup (`RECORD_READ_BYTES = 64` per record read) is a declared
accounting unit, not a measurement, and `Resource::Memory` in core is documented as peak or
integrated memory. A run budget has to declare a `Memory` limit for the lookup to be affordable,
and whether bytes read is the right thing to put against it is not settled here.

**The lookup's intercept** includes the cost of encoding an entry in the share of windows where a
record matched (18 of 22 in this pool, 17 of 22 at n = 16). A window with no match costs about a
third as much (section 6).

## 4. Declared against measured, final run

Declared cost within 25% of the criterion median at every row is the acceptance criterion; the
test passes. The largest deviations are the verifier at +18% (`h/2048`) and +14% (`n/1024`).

| benchmark | windows with an entry | median (us) | declared (us) | declared / median |
|---|---:|---:|---:|---:|
| heuristic_n/16 | 22/22 | 15.26 | 16.37 | 1.073 |
| heuristic_n/64 | 22/22 | 21.86 | 20.26 | 0.927 |
| heuristic_n/256 | 22/22 | 36.44 | 35.90 | 0.985 |
| heuristic_n/1024 | 22/22 | 90.98 | 98.41 | 1.082 |
| estimator_n/16 | 22/22 | 41.57 | 42.19 | 1.015 |
| estimator_n/64 | 22/22 | 49.94 | 47.80 | 0.957 |
| estimator_n/256 | 22/22 | 70.72 | 70.17 | 0.992 |
| estimator_n/1024 | 22/22 | 162.87 | 159.73 | 0.981 |
| memory_n/16 | 17/22 | 12.06 | 13.33 | 1.105 |
| memory_n/64 | 18/22 | 17.90 | 17.38 | 0.971 |
| memory_n/256 | 18/22 | 33.13 | 33.55 | 1.013 |
| memory_n/1024 | 18/22 | 88.15 | 98.25 | 1.115 |
| verifier_n/16 | 22/22 | 54.61 | 58.02 | 1.062 |
| verifier_n/64 | 22/22 | 66.44 | 65.94 | 0.992 |
| verifier_n/256 | 22/22 | 98.75 | 97.62 | 0.989 |
| verifier_n/1024 | 22/22 | 196.99 | 224.34 | 1.139 |
| heuristic_s/4 | 22/22 | 35.55 | 35.90 | 1.010 |
| heuristic_s/8 | 22/22 | 35.62 | 35.90 | 1.008 |
| heuristic_s/12 | 22/22 | 36.48 | 35.90 | 0.984 |
| estimator_s/4 | 22/22 | 61.22 | 61.29 | 1.001 |
| estimator_s/8 | 22/22 | 70.73 | 71.85 | 1.016 |
| estimator_s/12 | 22/22 | 84.42 | 82.41 | 0.976 |
| memory_s/4 | 18/22 | 33.19 | 33.55 | 1.011 |
| memory_s/8 | 18/22 | 32.84 | 33.55 | 1.022 |
| memory_s/12 | 18/22 | 33.53 | 33.55 | 1.000 |
| verifier_s/4 | 22/22 | 80.30 | 79.86 | 0.994 |
| verifier_s/8 | 22/22 | 97.63 | 100.98 | 1.034 |
| verifier_s/12 | 22/22 | 124.96 | 122.10 | 0.977 |
| heuristic_h/32 | 22/22 | 17.60 | 17.67 | 1.004 |
| heuristic_h/128 | 22/22 | 26.89 | 25.48 | 0.947 |
| heuristic_h/512 | 22/22 | 54.39 | 56.74 | 1.043 |
| heuristic_h/2048 | 22/22 | 173.47 | 181.76 | 1.048 |
| estimator_h/32 | 22/22 | 44.70 | 44.06 | 0.986 |
| estimator_h/128 | 22/22 | 57.09 | 55.26 | 0.968 |
| estimator_h/512 | 22/22 | 98.31 | 100.03 | 1.017 |
| estimator_h/2048 | 22/22 | 275.53 | 279.13 | 1.013 |
| memory_h/32 | 17/22 | 14.34 | 14.67 | 1.023 |
| memory_h/128 | 18/22 | 23.65 | 22.77 | 0.963 |
| memory_h/512 | 18/22 | 53.05 | 55.11 | 1.039 |
| memory_h/2048 | 18/22 | 165.05 | 184.54 | 1.118 |
| verifier_h/32 | 22/22 | 54.16 | 60.66 | 1.120 |
| verifier_h/128 | 22/22 | 77.56 | 76.50 | 0.986 |
| verifier_h/512 | 22/22 | 130.65 | 139.86 | 1.071 |
| verifier_h/2048 | 22/22 | 332.57 | 393.30 | 1.183 |
| memory_r/0 | 18/22 | 18.51 | 17.38 | 0.939 |
| memory_r/16 | 18/22 | 19.90 | 18.70 | 0.940 |
| memory_r/128 | 18/22 | 29.74 | 27.94 | 0.939 |
| memory_r/1024 | 18/22 | 102.31 | 101.86 | 0.996 |

## 5. Stability of the fit

`calibrate.py` fits each model by least squares on relative error (each row weighted by one over
its median). Four runs of the benchmark gave these fits; the constants in section 3 are rounded
from them, not from any one. Runs 1 and 2 predate the held-out groups (and run 1 the padded
record sweep, so it has no lookup fit).

| run | heuristic `a`, `b` | estimator `a`, `b`, `c` | lookup `a`, `b`, `c` | verifier `a`, `b`, `c` |
|---|---|---|---|---|
| 1 | 683, 4062 | 1049, 5301, 115520 | | 754, 8059, 241452 |
| 2 | 682, 3563 | 979, 5308, 114413 | 500, 3867, 3784 | 840, 6681, 233769 |
| 3 | 685, 3620 | 813, 5242, 134968 | 496, 3794, 3719 | 636, 7701, 242065 |
| 4 (recorded) | 687, 3599 | 885, 5383, 129642 | 509, 3645, 3886 | 785, 6841, 238427 |

(`a` in ns, slopes in ps per unit.) Slopes move by up to 20% between runs on this shared machine
(verifier `b`: 6.7 to 8.1 ns per observation), which is why the acceptance tolerance of 25% leaves
little room if the machine is busier than it was. In runs 2 to 4 the fitted model predicted every
row, held-out rows included, within 0.86 to 1.14 of the measured median (the extremes were the
estimator at `h/128`, 0.86, and the lookup at `h/2048`, 1.14, both in run 3). A busy machine is the likeliest reason for the calibration test
to fail on a rerun without anything in the code having changed.

## 6. Where the declared cost is wrong

A declared cost that depends on sizes only cannot track content. Three measurements, all pinned
to core 2, all at n = 256.

**By episode class** (`class_spread_at_window_size_256`; measured per run with a plain timing loop
against the declared cost for that class's windows). Declared over measured, per class:

| component | range over the 11 classes | worst classes |
|---|---|---|
| heuristic | 0.94 to 1.36 | ComponentTimeout 1.36, JointlyDecisive 1.35 |
| estimator | 0.78 to 1.35 | NoFault 1.35, Duplicates 0.78 |
| lookup | 0.93 to 2.02 | JointlyDecisive 2.02, NoFault 1.96 (no record matches, so no entry is built) |
| verifier | 0.75 to 1.39 | DelayedConfigChange 1.39, QuietUrgent 1.39, NoFault 0.75 |

So per class the declared cost is outside 25% of the measured cost for every component, in
either direction. The pooled number in section 4 is right on average over a class mix that
weights each class equally and that no experiment will use.

**By informative density** (`content_envelope_at_window_size_256`; measured over declared at the
same size):

| component | declared (us) | all benign | all informative, anchor first | informative, anchor late |
|---|---:|---:|---:|---:|
| heuristic | 1.63 | 0.47x | 0.69x | 0.69x |
| estimator | 3.39 | 0.55x | 1.17x | 1.16x |
| lookup | 1.52 | 0.27x | 0.46x | 0.47x |
| verifier | 4.83 | 1.50x | 2.32x | 24.31x |

"Benign" is counters below the alarm threshold; "informative" is alarms the rules can
discriminate on. The generated classes are mostly noise, so the pooled calibration sits near the
benign end for the estimator and verifier.

**The verifier is not linear in the worst case.** `consistent_hypotheses` re-scans the
observations before each dependent observation to find the site's anchoring `ErrorRate`
(`anchored` in `physics::consistent_worlds`), which is quadratic when the anchor comes late and
many dependent observations follow. The last column above is that shape (24 times declared at
n = 256); at n = 1025 a scratch measurement, not committed, gave 1.7 ms against 10.6 us declared.
The generator puts the site's `ErrorRate` first, so generated streams do not hit it, but a
recency window that drops the early anchor and keeps dependent alarms moves toward it. This
crate does not change the checker (the world is not this item's to edit); the model is affine
because the measured generated workload is.

## 7. Recalibrating

Rerun section 1 after a change to the CPU, the toolchain, the workspace `[profile]` (the baselines
item adds `lto = "thin"` and `codegen-units = 1`, which will move every constant), or the code of
a component. Fit with `calibrate.py`, round, edit the constants and the comment above them in the
component's source, and rerun the test. Do not edit the tolerance to make a test pass.
