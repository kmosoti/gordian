# M3: sub-tick support, mutation tests, strict precision

Exploration (Lab 1, `docs/lab-queue.md` "## M3"). Nothing here tests a preregistered
hypothesis. **Result: M3 does not hold.** Result 1 fails at 500 ms (+0.019, paired lower bound
−0.003) and at 2 s (−0.003, lower bound −0.028) against the re-anchor. Both frozen media meet the
background bound and the strict-precision bound with a wide margin. Sub-tick pruning does what it
was built to do: against its own control (the same graph with the pruning off) it gains +0.059
anchor-correct at 500 ms and +0.169 at 2 s, with lower bounds well above zero. What it gained, the
tuning rule then spent on precision.

## The criterion, as it stood when the held-out run started

Fixed by the chief before any M3 code or run (2026-10-06) and not re-fixed (review log, B3,
"Decided" 1: "M3's criterion is not re-fixed"; checked on main at `85bdb22` immediately before
the held-out run). M2's two results against `ReanchorNoticer` (anchor-correct 0.952, leak noticed
0.460), with M2's margins (result 1: point difference ≥ 0.03 and paired lower bound > 0.01;
result 2: ≥ 0.20 and > 0.10) and background bound (≤ 6.82 notices on background per stream,
point estimate), plus strict precision ≥ 0.67 (point estimate). "Holds" means result 1 holds at
500 ms and at 2 s. Result 2 and 100 ms are reported for continuity. The readings of what the
brief left open are in `scripts/m3_common.py`, written before the first tuning run and
unchanged (`git diff 15dd717 HEAD -- scripts/m3_common.py` is empty).

## Results (200 held-out streams, seeds 20000–20199; b = 5, ρ = 0.7; selection oracle at 16 s; the rung's context)

### Against the comparator (`m3-criterion.csv`; the rows marked "criterion" decide)

| Tick | Row | Result | Medium | Re-anchor | Paired difference [90%] | Point ≥ margin | Lower bound | Background ≤ 6.82 | Strict precision ≥ 0.67 | Holds |
|---|---|---|---|---|---|---|---|---|---|---|
| 100 ms | frozen M3 medium | result1 | 0.9812 | 0.9516 | +0.0296 [+0.0105, +0.0496] | no | yes (> 0.01) | 2.04 | 0.772 | no |
| 100 ms | frozen M3 medium | result2 | 0.7698 | 0.4604 | +0.3094 [+0.2258, +0.3934] | yes | yes (> 0.1) | 2.04 | 0.772 | yes |
| 100 ms | control: sub-tick off | result1 | 0.9758 | 0.9516 | +0.0242 [+0.0029, +0.0453] | no | no (> 0.01) | 2.08 | 0.782 | no |
| 100 ms | control: sub-tick off | result2 | 0.7698 | 0.4604 | +0.3094 [+0.2239, +0.3950] | yes | yes (> 0.1) | 2.08 | 0.782 | yes |
| 100 ms | M2's frozen medium | result1 | 0.9919 | 0.9516 | +0.0403 [+0.0217, +0.0602] | yes | yes (> 0.01) | 5.44 | 0.501 | no |
| 100 ms | M2's frozen medium | result2 | 0.9928 | 0.4604 | +0.5324 [+0.4615, +0.6029] | yes | yes (> 0.1) | 5.44 | 0.501 | no |
| 500 ms | frozen M3 medium | result1 | 0.9704 | 0.9516 | +0.0188 [-0.0026, +0.0397] | no | no (> 0.01) | 0.95 | 0.807 | no (criterion) |
| 500 ms | frozen M3 medium | result2 | 0.8921 | 0.4604 | +0.4317 [+0.3478, +0.5154] | yes | yes (> 0.1) | 0.95 | 0.807 | yes |
| 500 ms | control: sub-tick off | result1 | 0.9113 | 0.9516 | -0.0403 [-0.0705, -0.0108] | no | no (> 0.01) | 1.99 | 0.783 | no |
| 500 ms | control: sub-tick off | result2 | 0.8921 | 0.4604 | +0.4317 [+0.3503, +0.5125] | yes | yes (> 0.1) | 1.99 | 0.783 | yes |
| 500 ms | M2's frozen medium | result1 | 0.9812 | 0.9516 | +0.0296 [+0.0106, +0.0495] | no | yes (> 0.01) | 5.33 | 0.537 | no |
| 500 ms | M2's frozen medium | result2 | 0.9856 | 0.4604 | +0.5252 [+0.4552, +0.5954] | yes | yes (> 0.1) | 5.33 | 0.537 | no |
| 2000 ms | frozen M3 medium | result1 | 0.9489 | 0.9516 | -0.0027 [-0.0284, +0.0227] | no | no (> 0.01) | 1.01 | 0.843 | no (criterion) |
| 2000 ms | frozen M3 medium | result2 | 0.7122 | 0.4604 | +0.2518 [+0.1538, +0.3516] | yes | yes (> 0.1) | 1.01 | 0.843 | yes |
| 2000 ms | control: sub-tick off | result1 | 0.7796 | 0.9516 | -0.1720 [-0.2116, -0.1318] | no | no (> 0.01) | 5.27 | 0.603 | no |
| 2000 ms | control: sub-tick off | result2 | 0.7410 | 0.4604 | +0.2806 [+0.1871, +0.3758] | yes | yes (> 0.1) | 5.27 | 0.603 | no |
| 2000 ms | M2's frozen medium | result1 | 0.9086 | 0.9516 | -0.0430 [-0.0752, -0.0109] | no | no (> 0.01) | 4.29 | 0.163 | no |
| 2000 ms | M2's frozen medium | result2 | 1.0000 | 0.4604 | +0.5396 [+0.4672, +0.6098] | yes | yes (> 0.1) | 4.29 | 0.163 | no |

The comparator reproduces B2 exactly (0.952, 0.460, 6.82, 0.672). At 100 ms the frozen medium
misses result 1's margin by 0.0004: 11 incidents more than the re-anchor where the margin needs
11.2, the same knife edge M2's 500 ms row stood on.

### Every row (`m3-heldout-table.csv`; 90% cluster-bootstrap intervals over streams)

Anchor-correct is over the 372 hard non-leak incidents; leak measures over the 139 leaks. Cost
per stream is the arm's whole bill under the selection oracle (reasoner calls dominate);
substrate is the medium's and the noticer's counted operations at the declared prices.

| Row | Anchor-correct | Leak noticed | Leak anchor-correct | Background / stream | Strict precision | Notices / incident | Decoy notices / stream | Calls / stream | Substrate s / stream | Cost s / stream |
|---|---|---|---|---|---|---|---|---|---|---|
| `rung_z3` | 0.890 [0.863, 0.916] | 0.460 | 0.000 [0.000, 0.000] | 4.00 [3.76, 4.24] | 0.748 [0.737, 0.759] | 0.89 | 1.88 | 2.10 | 0.0003 | 0.664 |
| `rung_z2` | 0.914 [0.890, 0.937] | 0.460 | 0.000 [0.000, 0.000] | 9.03 [8.63, 9.45] | 0.613 [0.600, 0.624] | 0.97 | 2.27 | 2.16 | 0.0005 | 0.646 |
| `reanchor` | 0.952 [0.932, 0.970] | 0.460 | 0.000 [0.000, 0.000] | 6.82 [6.47, 7.18] | 0.672 [0.660, 0.683] | 1.04 | 2.40 | 2.23 | 0.0005 | 0.670 |
| `ramp_over_re2` | 0.952 [0.932, 0.970] | 0.986 | 0.971 [0.945, 0.993] | 6.37 [6.02, 6.73] | 0.685 [0.674, 0.696] | 1.11 | 2.96 | 2.63 | 0.0005 | 0.791 |
| `ramp_split_over_re2` | 0.973 [0.959, 0.986] | 0.986 | 0.971 [0.945, 0.993] | 6.24 [5.88, 6.61] | 0.693 [0.682, 0.704] | 1.12 | 3.00 | 2.65 | 0.0005 | 0.795 |
| `med_t100` | 0.981 [0.967, 0.993] | 0.770 | 0.475 [0.406, 0.544] | 2.04 [1.86, 2.21] | 0.772 [0.762, 0.782] | 1.10 | 3.15 | 3.56 | 0.0004 | 0.964 |
| `med_t100_subtick_off` | 0.976 [0.961, 0.989] | 0.770 | 0.460 [0.389, 0.533] | 2.08 [1.91, 2.26] | 0.782 [0.772, 0.792] | 1.04 | 2.98 | 3.41 | 0.0004 | 0.934 |
| `m2_t100` | 0.992 [0.984, 0.998] | 0.993 | 0.554 [0.481, 0.623] | 5.44 [5.14, 5.74] | 0.501 [0.487, 0.516] | 1.65 | 4.80 | 6.33 | 0.0008 | 1.848 |
| `med_t100_every_off` | 0.981 [0.967, 0.993] | 0.777 | 0.475 [0.406, 0.544] | 1.77 [1.61, 1.94] | 0.790 [0.779, 0.800] | 1.05 | 3.02 | 3.44 | 0.0004 | 0.942 |
| `med_t100_cut_off` | 0.981 [0.967, 0.993] | 0.755 | 0.468 [0.397, 0.539] | 2.40 [2.22, 2.59] | 0.755 [0.744, 0.765] | 1.10 | 3.12 | 3.53 | 0.0004 | 0.955 |
| `med_t500` | 0.970 [0.954, 0.985] | 0.892 | 0.482 [0.410, 0.551] | 0.95 [0.84, 1.06] | 0.807 [0.796, 0.817] | 1.13 | 3.21 | 2.74 | 0.0004 | 0.807 |
| `med_t500_subtick_off` | 0.911 [0.885, 0.936] | 0.892 | 0.475 [0.403, 0.543] | 1.99 [1.83, 2.15] | 0.783 [0.773, 0.794] | 0.96 | 2.81 | 2.56 | 0.0003 | 0.756 |
| `m2_t500` | 0.981 [0.967, 0.992] | 0.986 | 0.489 [0.416, 0.560] | 5.33 [5.03, 5.64] | 0.537 [0.520, 0.553] | 1.45 | 4.12 | 6.09 | 0.0006 | 1.786 |
| `med_t500_merge_off` | 0.976 [0.961, 0.989] | 0.892 | 0.482 [0.410, 0.551] | 0.96 [0.86, 1.07] | 0.729 [0.714, 0.744] | 1.21 | 3.42 | 2.84 | 0.0005 | 0.826 |
| `med_t500_every_off` | 0.968 [0.951, 0.983] | 0.906 | 0.482 [0.410, 0.551] | 0.62 [0.54, 0.71] | 0.832 [0.823, 0.842] | 1.02 | 2.96 | 2.67 | 0.0003 | 0.789 |
| `med_t500_cut_off` | 0.946 [0.926, 0.965] | 0.878 | 0.475 [0.403, 0.543] | 1.87 [1.72, 2.02] | 0.783 [0.772, 0.794] | 1.10 | 3.08 | 2.70 | 0.0004 | 0.799 |
| `med_t2000` | 0.949 [0.929, 0.968] | 0.712 | 0.173 [0.123, 0.222] | 1.01 [0.91, 1.11] | 0.843 [0.835, 0.850] | 1.01 | 2.87 | 3.21 | 0.0004 | 0.907 |
| `med_t2000_subtick_off` | 0.780 [0.746, 0.814] | 0.741 | 0.187 [0.138, 0.238] | 5.27 [4.99, 5.55] | 0.603 [0.591, 0.614] | 0.80 | 2.31 | 2.59 | 0.0004 | 0.759 |
| `m2_t2000` | 0.909 [0.882, 0.934] | 1.000 | 0.273 [0.212, 0.336] | 4.29 [4.00, 4.57] | 0.163 [0.160, 0.166] | 4.56 | 6.04 | 11.68 | 0.0041 | 3.795 |
| `med_t2000_merge_off` | 0.957 [0.938, 0.974] | 0.719 | 0.173 [0.123, 0.222] | 1.02 [0.91, 1.13] | 0.696 [0.682, 0.710] | 1.23 | 3.44 | 3.77 | 0.0005 | 1.026 |
| `med_t2000_every_off` | 0.946 [0.927, 0.965] | 0.734 | 0.187 [0.138, 0.238] | 0.55 [0.47, 0.62] | 0.794 [0.784, 0.803] | 0.95 | 2.75 | 2.94 | 0.0004 | 0.856 |
| `med_t2000_cut_off` | 0.871 [0.843, 0.899] | 0.719 | 0.173 [0.123, 0.222] | 3.96 [3.72, 4.20] | 0.730 [0.720, 0.740] | 0.94 | 2.56 | 3.00 | 0.0004 | 0.868 |

`*_merge_off`, `*_every_off` and `*_cut_off` are labelled sensitivity rows written into the
held-out arms at the freeze; nothing is chosen from them. (100 ms has no merge, so no merge-off
row.) `*_cut_off` keeps arrivals at event resolution and removes only the sub-tick lookback;
`*_every_off` keeps the lookback and removes arrivals at event resolution.

### The control: frozen medium minus the same graph with sub-tick pruning off (`m3-control.csv`)

| Tick | Measure | Frozen minus sub-tick-off control [90%] |
|---|---|---|
| 100 ms | hard_anchor_correct_share | +0.0054 [+0.0000, +0.0122] |
| 100 ms | strict_precision | -0.0099 [-0.0146, -0.0051] |
| 100 ms | notices_on_background_per_stream | -0.0450 [-0.1550, +0.0650] |
| 100 ms | leak_noticed_share | +0.0000 [-0.0233, +0.0226] |
| 500 ms | hard_anchor_correct_share | +0.0591 [+0.0389, +0.0805] |
| 500 ms | strict_precision | +0.0236 [+0.0175, +0.0296] |
| 500 ms | notices_on_background_per_stream | -1.0400 [-1.1950, -0.8850] |
| 500 ms | leak_noticed_share | +0.0000 [-0.0229, +0.0234] |
| 2000 ms | hard_anchor_correct_share | +0.1694 [+0.1331, +0.2047] |
| 2000 ms | strict_precision | +0.2399 [+0.2283, +0.2514] |
| 2000 ms | notices_on_background_per_stream | -4.2600 [-4.5500, -3.9750] |
| 2000 ms | leak_noticed_share | -0.0288 [-0.0526, -0.0075] |

### Beside the criterion: B3's two composed rows and the re-anchor, paired (`m3-b3-paired.csv`)

On the chief's instruction (added before the held-out run, never in the criterion): every medium
row minus each of `sel_ramp_split_over_re2_privileged`, `sel_ramp_over_re2_privileged` and the
re-anchor. The CSV has every medium row, controls and sensitivity rows included; the frozen M3
media and M2's are below.

| Medium row | Minus | Anchor-correct | Leak noticed | Leak anchor-correct | Background / stream | Strict precision |
|---|---|---|---|---|---|---|
| `med_t100` | `ramp_split_over_re2` | +0.008 [-0.009, +0.025] | -0.216 [-0.277, -0.157] | -0.496 [-0.571, -0.421] | -4.205 [-4.620, -3.795] | +0.079 [+0.065, +0.093] |
| `med_t100` | `ramp_over_re2` | +0.030 [+0.011, +0.050] | -0.216 [-0.277, -0.157] | -0.496 [-0.571, -0.421] | -4.335 [-4.745, -3.930] | +0.087 [+0.074, +0.101] |
| `med_t100` | `reanchor` | +0.030 [+0.011, +0.050] | +0.309 [+0.226, +0.393] | +0.475 [+0.406, +0.544] | -4.785 [-5.200, -4.380] | +0.100 [+0.088, +0.113] |
| `med_t500` | `ramp_split_over_re2` | -0.003 [-0.022, +0.016] | -0.094 [-0.141, -0.048] | -0.489 [-0.562, -0.419] | -5.290 [-5.675, -4.915] | +0.114 [+0.100, +0.127] |
| `med_t500` | `ramp_over_re2` | +0.019 [-0.003, +0.040] | -0.094 [-0.141, -0.048] | -0.489 [-0.562, -0.419] | -5.420 [-5.800, -5.050] | +0.122 [+0.108, +0.135] |
| `med_t500` | `reanchor` | +0.019 [-0.003, +0.040] | +0.432 [+0.348, +0.515] | +0.482 [+0.410, +0.551] | -5.870 [-6.250, -5.495] | +0.135 [+0.122, +0.148] |
| `med_t2000` | `ramp_split_over_re2` | -0.024 [-0.048, +0.000] | -0.273 [-0.339, -0.209] | -0.799 [-0.853, -0.742] | -5.230 [-5.610, -4.860] | +0.150 [+0.137, +0.163] |
| `med_t2000` | `ramp_over_re2` | -0.003 [-0.028, +0.023] | -0.273 [-0.339, -0.209] | -0.799 [-0.853, -0.742] | -5.360 [-5.735, -4.995] | +0.158 [+0.145, +0.170] |
| `med_t2000` | `reanchor` | -0.003 [-0.028, +0.023] | +0.252 [+0.154, +0.352] | +0.173 [+0.123, +0.222] | -5.810 [-6.185, -5.440] | +0.171 [+0.158, +0.184] |
| `m2_t100` | `ramp_split_over_re2` | +0.019 [+0.005, +0.033] | +0.007 [-0.013, +0.029] | -0.417 [-0.493, -0.345] | -0.800 [-1.290, -0.310] | -0.192 [-0.208, -0.175] |
| `m2_t100` | `ramp_over_re2` | +0.040 [+0.022, +0.060] | +0.007 [-0.013, +0.029] | -0.417 [-0.493, -0.345] | -0.930 [-1.415, -0.450] | -0.184 [-0.200, -0.167] |
| `m2_t100` | `reanchor` | +0.040 [+0.022, +0.060] | +0.532 [+0.462, +0.603] | +0.554 [+0.481, +0.623] | -1.380 [-1.865, -0.900] | -0.171 [-0.185, -0.156] |
| `m2_t500` | `ramp_split_over_re2` | +0.008 [-0.009, +0.025] | +0.000 [-0.024, +0.023] | -0.482 [-0.556, -0.411] | -0.910 [-1.390, -0.425] | -0.156 [-0.174, -0.138] |
| `m2_t500` | `ramp_over_re2` | +0.030 [+0.011, +0.049] | +0.000 [-0.024, +0.023] | -0.482 [-0.556, -0.411] | -1.040 [-1.510, -0.565] | -0.148 [-0.166, -0.130] |
| `m2_t500` | `reanchor` | +0.030 [+0.011, +0.049] | +0.525 [+0.455, +0.595] | +0.489 [+0.416, +0.560] | -1.490 [-1.960, -1.020] | -0.135 [-0.151, -0.119] |
| `m2_t2000` | `ramp_split_over_re2` | -0.065 [-0.093, -0.036] | +0.014 [+0.000, +0.033] | -0.698 [-0.765, -0.629] | -1.955 [-2.465, -1.445] | -0.530 [-0.542, -0.519] |
| `m2_t2000` | `ramp_over_re2` | -0.043 [-0.075, -0.011] | +0.014 [+0.000, +0.033] | -0.698 [-0.765, -0.629] | -2.085 [-2.585, -1.580] | -0.522 [-0.534, -0.511] |
| `m2_t2000` | `reanchor` | -0.043 [-0.075, -0.011] | +0.540 [+0.467, +0.610] | +0.273 [+0.212, +0.336] | -2.535 [-3.045, -2.030] | -0.509 [-0.521, -0.497] |

**Leak anchor-correct, every medium row** (also in the table above): M3 frozen 0.475 / 0.482 /
0.173 at 100 ms / 500 ms / 2 s; controls 0.460 / 0.475 / 0.187; M2's media 0.554 / 0.489 /
0.273; sensitivity rows 0.468–0.482 at 100 ms and 500 ms, 0.173–0.187 at 2 s. B3's two rows:
0.971. Every public noticer without the ramp: 0.000.

### Post hoc (added after the held-out run had been analysed; `scripts/m3_posthoc.py`)

M3's frozen media minus M2's at the same tick length, paired (`m3-vs-m2-paired.csv`):

| Tick | Measure | M3 | M2 | M3 minus M2 [90%] |
|---|---|---|---|---|
| 100 ms | hard_anchor_correct_share | 0.981 | 0.992 | -0.011 [-0.020, -0.003] |
| 100 ms | leak_noticed_share | 0.770 | 0.993 | -0.223 [-0.282, -0.166] |
| 100 ms | leak_anchor_correct_share | 0.475 | 0.554 | -0.079 [-0.117, -0.043] |
| 100 ms | notices_on_background_per_stream | 2.035 | 5.440 | -3.405 [-3.650, -3.165] |
| 100 ms | strict_precision | 0.772 | 0.501 | +0.271 [+0.260, +0.281] |
| 100 ms | calls_per_stream | 3.560 | 6.325 | -2.765 [-3.430, -2.125] |
| 100 ms | cost_s_per_stream | 0.964 | 1.848 | -0.884 [-1.079, -0.695] |
| 500 ms | hard_anchor_correct_share | 0.970 | 0.981 | -0.011 [-0.022, +0.000] |
| 500 ms | leak_noticed_share | 0.892 | 0.986 | -0.094 [-0.135, -0.055] |
| 500 ms | leak_anchor_correct_share | 0.482 | 0.489 | -0.007 [-0.020, +0.000] |
| 500 ms | notices_on_background_per_stream | 0.950 | 5.330 | -4.380 [-4.660, -4.110] |
| 500 ms | strict_precision | 0.807 | 0.537 | +0.270 [+0.259, +0.282] |
| 500 ms | calls_per_stream | 2.740 | 6.090 | -3.350 [-3.995, -2.735] |
| 500 ms | cost_s_per_stream | 0.807 | 1.786 | -0.979 [-1.170, -0.799] |
| 2000 ms | hard_anchor_correct_share | 0.949 | 0.909 | +0.040 [+0.011, +0.070] |
| 2000 ms | leak_noticed_share | 0.712 | 1.000 | -0.288 [-0.353, -0.225] |
| 2000 ms | leak_anchor_correct_share | 0.173 | 0.273 | -0.101 [-0.147, -0.058] |
| 2000 ms | notices_on_background_per_stream | 1.010 | 4.285 | -3.275 [-3.570, -2.985] |
| 2000 ms | strict_precision | 0.843 | 0.163 | +0.680 [+0.673, +0.687] |
| 2000 ms | calls_per_stream | 3.205 | 11.680 | -8.475 [-9.230, -7.740] |
| 2000 ms | cost_s_per_stream | 0.907 | 3.795 | -2.888 [-3.145, -2.638] |

Hard non-leak incidents that are not anchor-correct, by kind (`m3-misses.csv`). A notice anchored
on an observation outside the incident (a stray just before it) is attributed elsewhere by the
evaluator, so such an incident appears as "no notice attributed"; an attributed notice is never
anchored early (asserted).

| Row | Not anchor-correct (of 372) | No notice attributed | Anchored late (median s) |
|---|---|---|---|
| `m2_t100` | 3 | 1 | 2 (6.8) |
| `m2_t2000` | 34 | 0 | 34 (1.9) |
| `m2_t500` | 7 | 5 | 2 (6.8) |
| `med_t100_cut_off` | 7 | 5 | 2 (6.6) |
| `med_t100_every_off` | 7 | 5 | 2 (6.6) |
| `med_t100` | 7 | 5 | 2 (6.6) |
| `med_t100_subtick_off` | 9 | 6 | 3 (7.1) |
| `med_t2000_cut_off` | 48 | 28 | 20 (10.1) |
| `med_t2000_every_off` | 20 | 7 | 13 (2.0) |
| `med_t2000_merge_off` | 16 | 8 | 8 (4.4) |
| `med_t2000` | 19 | 10 | 9 (2.7) |
| `med_t2000_subtick_off` | 82 | 58 | 24 (8.0) |
| `med_t500_cut_off` | 20 | 16 | 4 (12.5) |
| `med_t500_every_off` | 12 | 9 | 3 (7.1) |
| `med_t500_merge_off` | 9 | 7 | 2 (6.6) |
| `med_t500` | 11 | 9 | 2 (6.6) |
| `med_t500_subtick_off` | 33 | 24 | 9 (13.1) |
| `ramp_over_re2` | 18 | 7 | 11 (7.3) |
| `ramp_split_over_re2` | 10 | 5 | 5 (15.2) |
| `reanchor` | 18 | 8 | 10 (6.5) |

Latency of the first notice from the incident's first observation (`m3-latency.csv`):

| Row | Hard non-leak: noticed, median s, p90 s | Leak: noticed, median s, p90 s |
|---|---|---|
| `m2_t100` | 371, 0.28, 0.49 | 138, 9.80, 18.66 |
| `m2_t2000` | 372, 1.32, 11.69 | 139, 12.85, 13.82 |
| `m2_t500` | 367, 0.32, 0.65 | 137, 10.51, 23.44 |
| `med_t100` | 367, 0.26, 0.47 | 107, 9.71, 18.06 |
| `med_t2000` | 362, 1.10, 1.90 | 99, 12.93, 13.80 |
| `med_t500` | 363, 0.27, 0.49 | 124, 10.04, 21.97 |
| `ramp_split_over_re2` | 367, 0.51, 4.85 | 137, 5.46, 6.78 |
| `reanchor` | 364, 0.51, 5.00 | 64, 14.22, 17.16 |

## What was built

- **Sub-tick support in `gordian-medium`** (commit `67dc0af`; `DESIGN.md` departures 53–57): a
  support lookback in microseconds (parameter 7 of `Integrator` and `Coincidence`, 0 off), applied
  when a cell fires from the run's instant in event time, so the anchoring rule's floor is no
  longer the tick; and arrivals at event resolution for the ordered coincidence (parameter 6, the
  predecessor PI's addition), so a same-kind stray does not stand for a burst and a later event
  does not hide one. Both off by default. M1's, M1b's and M2's pinned digests reproduce (commit
  `7a70871` pinned them before any M3 change).
- **The graph** (`crates/gordian-run/src/stream/arms/medium/graph.rs`; commits `12d8427`,
  `8ebc77b`, `5ea2ea2`), all off by default: `burst_subtick_ns` and `burst_every_event` (the two
  switches above, on the burst and three-kind coincidences); `merge_window_ns`, a cluster merge
  so services bursting together around one incident repeat one anchor; `confirm_window_ns`, the
  compound-incident confirmation as an ordered coincidence in event time instead of a latch;
  `ramp_refractory_ns`; and `ramp_inhibit`, which silences the ramp path while an anomaly is open
  at its service. The last four are precision devices the predecessor added after tune-a and
  tune-b, each by the tuning rule.
- **Frozen media** (`m3-selected.json`, commit `15dd717`), by the rule fixed in `m3_common.py`
  (tuning streams 10000–10099 only; background ≤ 5.6 and strict precision ≥ 0.70, then M2's score
  `min(z1, z2)`):

| Tick | Configuration | Tuning: anchor-correct / leak noticed / background / strict precision | z1 | Held-out anchor-correct |
|---|---|---|---|---|
| 100 ms | tune-c `all50L_m0_i1_r0_h15` | 0.975 / 0.818 / 1.92 / 0.754 | 0.84 | 0.981 |
| 500 ms | tune-d `dep50L_m10_i1` | 0.980 / 0.932 / 0.81 / 0.791 | 1.005 | 0.970 |
| 2 s | tune-c `dep50L_m50_i1_r0_h15` | 0.965 / 0.795 / 0.87 / 0.846 | 0.50 | 0.949 |

  Every selected medium uses the sub-tick lookback, arrivals at event resolution, the
  confirmation in event time, and the ramp inhibit (`i1`).

## Mutation tests (`m3-mutants.csv`, one row per mutant; `crates/gordian-medium/DESIGN.md` "Mutation checks", M3)

cargo-mutants 27.1.0, in place, under the cgroup runner. Two fresh runs on the final tree after
every test was added:

| Scope | Tests | Mutants | Caught | Unviable | Timeout | Missed |
|---|---|---|---|---|---|---|
| `gordian-run` medium arm: `adapters.rs`, `graph.rs`, `noticing.rs`, `mod.rs` | `--test stream_medium` | 294 | 267 | 17 | 3 | 7 |
| `gordian-medium/src`, M3's diff (`--in-diff`) | the crate's tests | 94 | 85 | 8 | 0 | 1 |

Earlier runs, which drove the added tests: five over the medium arm (the first stopped by its
time limit at 212 of 294 with 45 survivors; then 35, 20, 8 and 7, iterating) and one over the
M3 diff (15 survivors, killed by the seven tests of commit `7da37aa` but one).

Every survivor's disposition:

- **Timeouts (3), detected:** `TickClock::complete_before`'s `/` made `%` or `*`, and
  `run_ticks`'s `&&` made `||`; the tick loop then runs over about 10^8 ticks or without end, and
  the test binary does not finish in 120 s (baseline about 1 s).
- **Equivalent (8):** `message_tag`'s two `|` made `^` (the operands' bits are disjoint: ids
  below 2^16 against `0x4000_0000`; `folded` masked to `0x7FFF_FFFF` against `0x8000_0000`);
  `MediumParams::validate`'s `t > u32::MAX` made `>=` (`u32::MAX` is not a whole number of
  microseconds, so the next clause refuses it with the same message); the second
  `lost_anchors += 1` in `effect` made `-=` or `*=` (unreachable: an anchor is the `seq` of an
  event `encode` made, which is the id of an observation with a service, and only probes and
  corrections lack one); `reoffer`'s `h.id > first_anchor` made `>=` and `anchor < h.id` made
  `<=` (at equality the anomaly anchored there owns `h.id`, which the `!owns` guard excludes);
  `ordered_every_event`'s `slot < ORDERED_SLOTS` made `<=` (`slot` is the incoming-synapse index,
  and `MediumSpec::resolved`, through which every medium is built, refuses an ordered
  coincidence with more inputs than `ORDERED_SLOTS`).

The `gordian-run` runs used only `stream_medium`'s tests, so the tally is conservative: a mutant
another test file catches counts as missed there.

## Provenance

- **Freeze.** The graph was frozen at `15dd717` (selection, held-out arms, a behaviour digest of
  the frozen media), before the mutation tests. Every later commit is tests, analysis or records:
  `git diff 15dd717 HEAD` over `crates/gordian-medium/src`, `crates/gordian-run/src/stream/arms/medium`,
  `scripts/m3_common.py` and `m3-selected.json` is empty. The tally closed with the named commit
  `7665731` ("Freeze M3 for the held-out run").
- **Merges of main.** `dc22d69` (B3) before the mutation work; `5707862` (L1 and B4, main at
  `85bdb22`), no conflicts, `arms/noticer.rs` merged with every variant kept (`Medium`,
  `Learned`, `Composed` with B4's follow-up). After the second merge both pinned behaviour
  digests failed: B4 added `cause` to `NoticeLogEntry`, whose Debug text the digests hash. Commit
  `12ba61f` hashes the text with exactly B4's two inert spellings removed (`cause: None`,
  `cause: Some(Quiet)`) and refuses a follow-up cause; both digests then equal the constants
  pinned before the merge, unchanged.
- **Binary.** Release build at `12ba61f` (the held-out run's `source_revision`), SHA-256
  `b992f091ecd2db2cd45f94bc37f86e8aab68d2f530480133c6a253e04184770c`; it is the same as the build
  at the merge commit `5707862` (source unchanged between them).
- **Byte identity.** R6's held-out manifest (b = 5, ρ = 0.7) with only `source_revision` changed
  and the run id kept, run with that binary into `artifacts/runs/m3-xcheck-r6-heldout-b5-rho0.7`:
  `results.csv` and `incidents.csv` equal `r6-results-sha256.csv` for **62 of 62** arms
  (`m3-regression.csv`; recomputed with a separate shell loop, 62 of 62).
- **Reproduction** (`m3-reproduce.csv`, `scripts/m3_reproduce.py`). The eight rows earlier
  held-out runs also played on these 200 streams write the same five per-arm files modulo the
  run id: the rungs, the re-anchor and M2's three frozen media against M2's held-out run, B3's two
  composed rows against B3's: **40 of 40 files**. So the re-anchor, M2's rows and B3's rows here
  are the same numbers the review log records, and the merged code did not move them.
- **Hashes of the held-out run** as written: `m3-results-sha256.csv` (22 arms, five files each).
- **Host.** The manifest records `Intel(R) Xeon(R) Processor @ 2.10GHz`; M2's held-out ran on a
  2.80 GHz host. No measure here is a wall time; the reproduction above shows the outputs do not
  depend on it.

## Runs (all through `scripts/run-driver.sh` via `scripts/m3_run.py`, release build; `artifacts/runs/_logs/m3-runs.log`)

| Run | Seeds | Exit | Wall | Waits (30 s polls for another lab's process) |
|---|---|---|---|---|
| `m3-tune-a-b5-rho0.7` (predecessor) | 10000–10099 | 0 | 157 s | 0 |
| `m3-tune-b-b5-rho0.7` (predecessor) | 10000–10099 | 0 | 804 s | 0 |
| `m3-tune-c-b5-rho0.7` (predecessor) | 10000–10099 | 0 | 1,068 s | 0 |
| `m3-tune-d-b5-rho0.7` (predecessor) | 10000–10099 | 0 | 159 s | 1 |
| `m3-xcheck-r6-heldout-b5-rho0.7` (R6's run id) | R6's | 0 | 155 s | 0 |
| `m3-heldout-b5-rho0.7` | 20000–20199 | 0 | 126 s | 7 |

No refusal, no failed or excluded run, no step-capped segment. The tuning run directories were
archived as notice files (`artifacts/runs/m3-tune-*-notices.tar.xz`); tune-a's archive holds only
its manifest (its per-arm files were not kept by the predecessor; its table,
`m3-tuning-tune-a.csv`, is committed).

## Verified by running versus assumed

Verified by running, in this session: the gordian-medium tests with the restart's uncommitted
tests; the two fresh mutation runs and every count above; the freeze digests before and after the
merge (failing, then passing with the normalization, against unchanged constants); the release
build and its hash at both commits; the byte-identity gate (two independent checks); the
reproduction of eight rows against M2's and B3's run directories; the held-out run; every table
here, generated from the run directory by `m3_analyze.py`, `m3_posthoc.py` and
`m3_reproduce.py`; the B3 rows' spelling against B3's held-out manifest (identical); the gates
listed at the end of the report message.

Assumed, not rerun: the predecessor's tuning runs and tables (their logs and archives exist and
the selection follows from `m3-tuning-points.csv` by `m3_select.py`, which I did not rerun); the
equivalence arguments rest on reading the code paths named, not on an exhaustive search; that
`--test stream_medium` was the predecessor's deliberate scope for the arm's mutation runs.

## Hidden record

Not read, by me or (by their commits and notes) the predecessors. The graph names no family,
vocabulary or hidden timing; `scripts/check-no-oracle.sh` passes.

## PI's analysis

**What the result means.** M2 located the long-tick loss in one mechanism: the emitter's support
was cut at the tick edge, so a stray sharing a burst's tick was cited and became the anchor. M3
built the cut below the tick, and the control shows it is the mechanism: with everything else
equal, switching sub-tick pruning off costs 0.059 of anchor-correct at 500 ms and 0.169 at 2 s,
and the 2 s medium goes from 0.780 (pruning off) to 0.949, past M2's 0.909. Of the two switches,
the cut does most of the work at 2 s (cut off: 0.871; arrivals at event resolution off: 0.946).
So the brief's premise survived: the tick was the floor, and it is no longer.

**Why it does not hold anyway.** The criterion asks for 0.03 over the re-anchor, which is 12
incidents of 372 at a re-anchor that already gets 354. M3 also had to bring strict precision from
M2's 0.50–0.54 to at least 0.67, and the tuning rule demanded 0.70. It did (0.77–0.84, with
background at 1–2 per stream against 5.3–5.4 for M2), but the precision devices that got it
there (the cluster merge, the confirmation in event time, the ramp inhibit) cost anchoring: at
500 ms M3's medium anchors 0.970 against M2's 0.981 (−0.011 [−0.022, 0.000], post hoc). The
merge-off sensitivity row is the trade in one switch: 0.976 at 0.729 precision against 0.970 at
0.807. On the tuning streams anchoring saturated: ten feasible 500 ms configurations reached 195 of
199 incidents (z1 = 1.005, +0.030), the tuning set's ceiling, so the selection among them was made
by the tie rule (fewer background notices, then higher precision), not by anchoring. On held-out
streams the chosen one kept two thirds of the margin (+0.019), which is what a margin met exactly
by the best of 130 feasible configurations on 100 streams should be expected to do. At 2 s, 19
configurations within the background budget also reached 195 of 199 on the tuning streams, but
all with strict precision 0.35–0.38 (the ramp inhibit off: its repeated notices of plain
incidents); the best feasible one reached z1 = 0.50. So at 2 s the precision bound, not the
mechanism, decided: anchoring at the margin was available only at about half the required
precision. (Post hoc, from `m3-tuning-points.csv`.)

**What it does not show.** It does not show that sub-tick pruning cannot reach the margin at
500 ms: the margin may be reachable with a different precision device, at a different point on
the trade, or not at all on this world. It does not show the medium is worse than the re-anchor
at long ticks: at 500 ms it is +0.019 [−0.003, +0.040] on anchoring with 5.9 fewer background
notices and +0.135 strict precision, at 0.81 s per stream against 0.67. Against B3's
ramp + split it is at parity on anchoring (−0.003 [−0.022, +0.016] at 500 ms), far ahead on
background (−5.3) and precision (+0.11), and far behind on the leak (leak noticed −0.094, leak
anchor-correct −0.489).

**The leak.** M3's media notice fewer leaks than M2's (0.77 / 0.89 / 0.71 against 0.99–1.00)
and anchor them less often (0.475 / 0.482 / 0.173). The cause is visible in the tuning tables
(tune-c and tune-d, 57 configurations each way per tick): with the ramp inhibit off, leak noticing
on the tuning streams averages 0.99–1.00; with it on, 0.87–0.89 (lowest 0.80). Every selected
medium has it on. It buys little precision at 100 ms and 500 ms (0.73 → 0.77, 0.74 → 0.78 on
average) and most of it at 2 s (0.30 → 0.69). The tuning score `min(z1, z2)` was bound by anchoring at every tick length
(z2 was 1.7–2.4), so it was indifferent to giving leak noticing up for precision. Result 2 still
holds at every tick length against the re-anchor; against B3's ramp the medium now loses on
every leak measure. That is the tuning rule doing what it was written to do, and the rule did not
look at the leak beyond its margin.

**Cost.** The precision gain halves the bill: 0.96 / 0.81 / 0.91 s per stream against M2's
1.85 / 1.79 / 3.79, from 3.6 / 2.7 / 3.2 reasoner calls per stream against 6.3 / 6.1 / 11.7. The
medium's own operations stay under a millisecond per stream. At 500 ms the medium costs about the
same as B3's ramp + split (0.807 against 0.795 s).

**What I would test next.** (1) The trade as a curve, not a point: anchor-correct against strict
precision for the frozen 500 ms graph with each precision device switched separately, on the
tuning streams, to see whether any point clears 0.982 at ≥ 0.67. (2) The same graph with the ramp
inhibit restricted to services whose open anomaly is not ramp-noticed, which should recover the
leak without the plain-incident repeats the inhibit was added for. (3) Given B3, the comparison
that matters for EXP-101 is now cost at matched noticing (B4's and B5's frame), where M3's
precision gain is the medium's first measured advantage over the public rows on something other
than hard anchoring.

## What the chief should examine most carefully

1. **The digest normalization after the merge** (commit `12ba61f`): a test edited after the
   freeze. The pinned constants are unchanged and only B4's two inert spellings are removed; a
   follow-up cause fails the test. The reproduction of M2's rows against M2's own run directory
   (40 of 40 files) is independent evidence that the merge moved nothing.
2. **The order of freeze and mutation work.** The predecessor froze the graph before the mutation
   tests; the brief's order puts the tally first. No code changed after the freeze, so the frozen
   media are what was tested and what ran; the tests could have found a defect that the freeze
   would then have kept.
3. **The equivalence arguments** for the eight missed mutants, especially the unreachable
   `lost_anchors` branch and the two `reoffer` boundaries, which rest on invariants
   (`seq = held.id`, `owns(anchor)`) that a later change could break silently.
4. **The tuning rule's indifference to the leak**: the selected media gave up 0.1–0.3 of leak
   noticing for precision, and nothing in the rule weighed it. This was fixed before any run and
   is reported, not repaired.
5. **The mutation scope**: `stream_medium`'s tests only for the arm; conservative for the count,
   but a reviewer should know the other test files were not part of it.

## Kept run directories (until the chief has verified them)

- Held-out: `/home/user/gordian/.claude/worktrees/agent-a3abb7e7191ecfefc/artifacts/runs/m3-heldout-b5-rho0.7`
- Byte identity: `/home/user/gordian/.claude/worktrees/agent-a3abb7e7191ecfefc/artifacts/runs/m3-xcheck-r6-heldout-b5-rho0.7`
- Manifests: `.../artifacts/runs/_manifests/`; logs: `.../artifacts/runs/_logs/`
