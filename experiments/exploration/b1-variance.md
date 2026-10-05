# B1 variance: ten arms, four compute budgets, 500 episodes per class

Status: development run, `exploration` branch. Nothing here tests a hypothesis and nothing here may
later be cited as confirmation. The table is `b1-variance.csv`, one row per (arm, budget level,
class); this file records the runs behind it, what was checked, and every deviation from the plan.

## 1. What was run

B1 as decided in `docs/review-log.md` (A6 headroom probe, A6b, A8, A8b), which differs from the
plan's text: default limits never bind, so the compute budget is swept.

- **Arms** (ten, interleaved in one multi-arm manifest per budget level; A8 plays every arm on
  each episode, back to back, in an order drawn per episode from the run seed):
  `heuristic_only`; `all_components`; `random_matched` p = 0.25 and p = 0.5; `fixed_pipeline`
  verifier only; `fixed_pipeline` estimator only; `fixed_pipeline` heuristic with `every` = 2 and
  `every` = 4; `oracle_evidence_privileged`; `oracle_immediate_privileged`. Arm names in the
  files: `heuristic_only`, `all_components`, `random_p025`, `random_p050`, `fixed_verifier_only`,
  `fixed_estimator_only`, `fixed_heuristic_every2`, `fixed_heuristic_every4`,
  `oracle_evidence_privileged`, `oracle_immediate_privileged`. `random_matched`'s `p` is as
  specified (0.25, 0.5), **not** tuned to match any arm's compute.
- **Compute budget levels** (`limits.compute`, declared ns): 20,000,000 (the default, never binds);
  250,000; 100,000; 60,000. Everything else in `limits` is the default (12 probes, 250 ms of
  `Time`, 50 ms step, window 256, step cap 1000), and `decide` is the default (3 s patience).
- **Episodes**: seeds 1000 to 1499, all 11 classes, 500 per class: 5,500 episodes per arm per
  level, 55,000 arm-episodes per level, 220,000 in all. Default `episode_params` (noise rate 3).
- **How**: `scripts/run-driver.sh --manifest ...` for every run (cgroup v1 isolation, cores 0-2, CPU
  quota 300%, 2 GiB, `timeout` backstop 3,600 s, the driver pinned to core 3), a clean tree, and no
  `cargo` or `rustc` running (the driver refuses otherwise). Binary: `cargo build --locked
  --release -p gordian-run` at commit `4e771f2` (the manifest change that records the CPU model
  and frequency); every manifest's `source_revision` is that commit. The only commits after it
  add files under `experiments/exploration/`; no code changed, so the binary is the one every run
  used. I was the only worker on the machine.
- **Host**: Intel(R) Xeon(R) Processor @ 2.10GHz, 2100 MHz as read from `/proc/cpuinfo`; this is the recorded `cpu_model` and `cpu_mhz` of every manifest.
- Run seeds (arm-order draw): B1 runs 1, S1 2, S2 3, S3 4. `results.csv` does not depend on it
  (checked below).
- **Scripts**: `experiments/exploration/scripts/` holds what generated the manifests
  (`make_manifests.py`, `mkmanifest.py`), the order the runs were made in (`run_all.sh`), and the
  analysis behind the four documents (`b1csv.py`, `meta.py`, `b2.py`, `b3.py`, `b4.py`). Re-running
  the analysis scripts on the run directories reproduced `b1-variance.csv` and
  `b1-supplementary.csv` byte for byte and the B2, B3 and B4 numbers exactly (the bootstraps are
  seeded). `b4.py` also reads a table of the generator's truth per seed. It was first produced by a
  scratch program outside the repository (the repository's guard forbids naming the privileged
  accessor elsewhere). It is now regenerated from the repository by `cargo run -p gordian-eval
  --example truth_table`, which `b4.py` calls itself; the regenerated table is byte-identical to
  the scratch one (sha256 `482c934f...6300b`, 5,500 rows plus header). The table is not committed. Run directories
  (`artifacts/runs/`) are ignored by version control and not committed; the hashes below identify
  them.

## 2. The timing pilot and the extrapolation

As the plan asks, one level was run on a subset first. Pilot: 100 seeds per class (seeds 1000 to
1099, 1,100 episodes per arm, 11,000 arm-episodes) at 20,000,000, the level where arms do the
most work. It took **5.74 s of wall time** (5.67 s of CPU time, peak memory
13.9 MiB). Linear extrapolation to 500 seeds: **28.7 s per non-binding level**, about 2 minutes
for four levels (binding levels do less work, so this is an upper bound). The runs took 26.0, 21.2, 14.4, 10.9
s for 20,000,000, 250,000, 100,000 and 60,000 (72.6 s in all), so the estimate was 10%
high at the non-binding level, as expected. Total wall time of every run described here
(pilot, B1, supplementary): 233 s.

**Pilot as a replay check.** Every deterministic column of the pilot's `results.csv` (all except
`run_id`) is identical, for all ten arms, to the rows of the 20,000,000 run for the same
`(seed, class)` pairs: the 1,100 episodes per arm match exactly.

## 3. The runs

| label | run id | run seed | compute limit (ns) | manifest.json sha256 | wall (s) | CPU (s) | peak memory (MiB) | internal / external ratio | exit |
|---|---|---|---|---|---|---|---|---|---|
| pilot | `b1-pilot-c20000000-s1000-1099` | 1 | 20,000,000 | `853658437e6690502e1a057f80e411f230395c8862ed2f1ef3780f27917891c4` | 5.7 | 5.7 | 13.9 | 0.942 | 0 |
| B1 20,000,000 | `b1-c20000000-s1000-1499` | 1 | 20,000,000 | `bb70350cc8423455f4fd361a7cec387f450c6e96e188d12ee73814f9fdf87c81` | 26.0 | 25.8 | 43.5 | 0.944 | 0 |
| B1 250,000 | `b1-c250000-s1000-1499` | 1 | 250,000 | `5197a2ea32c4d004b8492fff96a7d88d6b1352b0936181fa06bfbd62b7083a69` | 21.2 | 21.0 | 38.2 | 0.948 | 0 |
| B1 100,000 | `b1-c100000-s1000-1499` | 1 | 100,000 | `3b752402b8151a58792946c57964009a4f33115461ffebf1a831c2561f85777e` | 14.4 | 14.2 | 31.5 | 0.934 | 0 |
| B1 60,000 | `b1-c60000-s1000-1499` | 1 | 60,000 | `aa17c7961d1e0ec3a18df11daf1c36dfc738d16f55e627b8611a37c6d7a0ba93` | 10.9 | 10.8 | 28.5 | 0.929 | 0 |
| S1 20,000,000 | `b1s-c20000000-s1000-1499` | 2 | 20,000,000 | `d9d6c9ee3a19048ca1e897cc52f8d60ee76b5d205a1b0494a948af0ae7fde200` | 4.7 | 4.7 | 14.8 | 0.919 | 0 |
| S1 250,000 | `b1s-c250000-s1000-1499` | 2 | 250,000 | `7b6d8bb49cc6a629b2f13400b61f9da3c5362239d66370f592f5d118fad8945f` | 4.9 | 4.8 | 14.7 | 0.915 | 0 |
| S1 100,000 | `b1s-c100000-s1000-1499` | 2 | 100,000 | `a72ac64662d3918ddd2c57b4d6d5d168c66b70eef72733dbd99c5b019cb62231` | 5.0 | 5.0 | 14.8 | 0.925 | 0 |
| S1 60,000 | `b1s-c60000-s1000-1499` | 2 | 60,000 | `19ac6cb80338eda65de2fb653854d6482ccd9ed2c60f72ccc4c20cddf0ab24fe` | 4.9 | 4.8 | 14.5 | 0.936 | 0 |
| S2 15,000 | `b1s2-c15000-s1000-1499` | 3 | 15,000 | `6ddf48b671a40dcf346afe2b8ac34efb3b00444a5d974de9d321e6fcb287d8ab` | 6.7 | 6.6 | 24.3 | 0.926 | 0 |
| S2 20,000 | `b1s2-c20000-s1000-1499` | 3 | 20,000 | `4de1ef999d49f9761747fd13bc2604bb2d71c9b6ea75f2b47e0fe2ed2bd1dbfb` | 7.4 | 7.3 | 24.1 | 0.924 | 0 |
| S2 30,000 | `b1s2-c30000-s1000-1499` | 3 | 30,000 | `62ffa6445c130d3bc7fb5e4aced992630d2c526bea49356925c1ccb2bc2d042a` | 8.2 | 8.1 | 25.8 | 0.929 | 0 |
| S2 40,000 | `b1s2-c40000-s1000-1499` | 3 | 40,000 | `3f425bc8ca8af192b5ec5a67142e56c2640f1cbb7411a8e729a13e2ea7998e96` | 9.1 | 9.0 | 26.9 | 0.927 | 0 |
| S2 50,000 | `b1s2-c50000-s1000-1499` | 3 | 50,000 | `a55a6c18403d929d821b399625add8471833e0048b63bcdb12b82e64a0c748ca` | 9.9 | 9.8 | 27.6 | 0.927 | 0 |
| S3 20,000,000 | `b3n50-c20000000-s1000-1499` | 4 | 20,000,000 | `e6663642a5c9b782d1b1ee37d82af2fedf3c6b616d825de92a6e23c652602c71` | 32.8 | 32.5 | 68.3 | 0.929 | 0 |
| S3 250,000 | `b3n50-c250000-s1000-1499` | 4 | 250,000 | `b41116f617245e6ed52a9acf7bafa0a4fdceba68b6c52a324bf567812dd9fbf9` | 27.2 | 26.9 | 61.7 | 0.933 | 0 |
| S3 100,000 | `b3n50-c100000-s1000-1499` | 4 | 100,000 | `7d7d87e080868e4b610b73694ac6fa7389ea7339bb747073656103c2ad510b93` | 18.9 | 18.6 | 54.0 | 0.920 | 0 |
| S3 60,000 | `b3n50-c60000-s1000-1499` | 4 | 60,000 | `d8e9cd0e4405afcd7af40305f2b3ae361c9cd2f574be178567ea48811f65953b` | 15.4 | 15.2 | 50.9 | 0.910 | 0 |

Wall time, CPU time and peak memory are the runner's (`usage.json`). The internal / external
ratio is the sum of the arms' measured time and the drift workload over the runner's CPU time
(0.910 to 0.948); no tolerance is declared in the manifests (A4's first value is a starting
point), so it is reported, not judged.

**Drift diagnostics** (`gordian-analyze drift`; the fixed verifier workload, timed before the
first episode, every 50 `(seed, class)` units, and after the last):

| run | blocks | CV of `ns` | last / first `ns` | CV of `min_ns` | last / first `min_ns` |
|---|---|---|---|---|---|
| pilot | 23 | 0.100 | 0.806 | 0.027 | 0.928 |
| B1 20,000,000 | 111 | 0.179 | 0.959 | 0.087 | 0.964 |
| B1 250,000 | 111 | 0.145 | 1.073 | 0.055 | 0.990 |
| B1 100,000 | 111 | 0.200 | 0.641 | 0.078 | 0.972 |
| B1 60,000 | 111 | 0.122 | 0.895 | 0.044 | 0.910 |
| S1 20,000,000 | 111 | 0.053 | 0.879 | 0.021 | 0.935 |
| S1 250,000 | 111 | 0.152 | 0.950 | 0.067 | 0.956 |
| S1 100,000 | 111 | 0.431 | 0.883 | 0.033 | 0.948 |
| S1 60,000 | 111 | 0.241 | 0.894 | 0.059 | 0.987 |
| S2 15,000 | 111 | 0.107 | 0.916 | 0.014 | 0.929 |
| S2 20,000 | 111 | 0.161 | 1.023 | 0.059 | 0.958 |
| S2 30,000 | 111 | 0.076 | 0.926 | 0.020 | 0.964 |
| S2 40,000 | 111 | 0.137 | 0.942 | 0.047 | 0.968 |
| S2 50,000 | 111 | 0.203 | 0.603 | 0.068 | 0.755 |
| S3 20,000,000 | 111 | 0.151 | 0.955 | 0.055 | 0.965 |
| S3 250,000 | 111 | 0.162 | 0.989 | 0.021 | 0.987 |
| S3 100,000 | 111 | 0.305 | 0.749 | 0.018 | 0.960 |
| S3 60,000 | 111 | 0.158 | 0.714 | 0.051 | 0.961 |

Reading it: the CV of the block time `ns` is 0.053 to 0.431 across all runs (0.122 to 0.200 for B1), and the CV of
the per-block minimum `min_ns` is 0.014 to 0.087, so the host was disturbed in bursts rather
than drifting steadily, as A8 found. The last-over-first ratios of `ns` are 0.60 to 1.07. None of this
touches the modelled cost, which is the charter's `C` (A8b) and is deterministic; it limits
only the secondary wall-time columns (`measured_policy_ns`). The position-effect test (`gordian-analyze
position`) was not run: no result here uses wall time as a cost.

## 4. What the table shows (pooled over the 11 classes; the CSV has every class)

Pooled success weights the classes equally (500 each). An arm that does nothing scores 0.0909
from `NoFault` alone (`NoFault` is scored for free; review log, A5, A6b), so success on the 10
faulted classes is given beside it. Modelled cost is `modelled_cost_ns`, the sum of the component and
scheduling columns of `results.csv`. "Wrong declaration" is not success, not abstained, not
undecided.


**Compute limit 20,000,000 ns**

| arm | success | success, 10 faulted classes | critical miss | abstained | wrong declaration | mean probes | mean modelled cost (ns) | sd of modelled cost (ns) |
|---|---|---|---|---|---|---|---|---|
| `heuristic_only` | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.71 | 42,637 | 23,675 |
| `all_components` | 0.9649 | 0.9614 | 0.0000 | 0.0002 | 0.0349 | 0.71 | 521,984 | 255,701 |
| `random_matched` p=0.25 | 0.9618 | 0.9580 | 0.0007 | 0.0002 | 0.0380 | 0.65 | 174,495 | 87,713 |
| `random_matched` p=0.5 | 0.9649 | 0.9614 | 0.0000 | 0.0002 | 0.0349 | 0.68 | 288,125 | 143,580 |
| `fixed_pipeline` verifier only | 0.9525 | 0.9478 | 0.0000 | 0.0125 | 0.0349 | 0.70 | 410,106 | 214,225 |
| `fixed_pipeline` estimator only | 0.9538 | 0.9492 | 0.0000 | 0.0113 | 0.0349 | 0.70 | 98,187 | 47,224 |
| `fixed_pipeline` heuristic, every 2 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.69 | 29,094 | 16,388 |
| `fixed_pipeline` heuristic, every 4 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.67 | 22,406 | 12,617 |
| `oracle_evidence` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 2.43 | 0 | 0 |
| `oracle_immediate` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 0.00 | 0 | 0 |

**Compute limit 250,000 ns**

| arm | success | success, 10 faulted classes | critical miss | abstained | wrong declaration | mean probes | mean modelled cost (ns) | sd of modelled cost (ns) |
|---|---|---|---|---|---|---|---|---|
| `heuristic_only` | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.71 | 42,637 | 23,675 |
| `all_components` | 0.2905 | 0.2196 | 0.1573 | 0.0002 | 0.7093 | 0.09 | 285,922 | 81,769 |
| `random_matched` p=0.25 | 0.9500 | 0.9450 | 0.0022 | 0.0002 | 0.0498 | 0.65 | 173,175 | 85,005 |
| `random_matched` p=0.5 | 0.7222 | 0.6944 | 0.0547 | 0.0002 | 0.2776 | 0.47 | 248,282 | 96,199 |
| `fixed_pipeline` verifier only | 0.5385 | 0.4924 | 0.0965 | 0.0125 | 0.4489 | 0.32 | 308,831 | 109,666 |
| `fixed_pipeline` estimator only | 0.9480 | 0.9428 | 0.0000 | 0.0113 | 0.0407 | 0.70 | 98,039 | 47,268 |
| `fixed_pipeline` heuristic, every 2 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.69 | 29,094 | 16,388 |
| `fixed_pipeline` heuristic, every 4 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.67 | 22,406 | 12,617 |
| `oracle_evidence` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 2.43 | 0 | 0 |
| `oracle_immediate` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 0.00 | 0 | 0 |

**Compute limit 100,000 ns**

| arm | success | success, 10 faulted classes | critical miss | abstained | wrong declaration | mean probes | mean modelled cost (ns) | sd of modelled cost (ns) |
|---|---|---|---|---|---|---|---|---|
| `heuristic_only` | 0.9482 | 0.9430 | 0.0000 | 0.0111 | 0.0407 | 0.70 | 42,571 | 23,628 |
| `all_components` | 0.1818 | 0.1000 | 0.1818 | 0.0002 | 0.8180 | 0.00 | 121,279 | 25,823 |
| `random_matched` p=0.25 | 0.4598 | 0.4058 | 0.1165 | 0.0002 | 0.5400 | 0.21 | 120,703 | 38,394 |
| `random_matched` p=0.5 | 0.2140 | 0.1354 | 0.1747 | 0.0002 | 0.7858 | 0.02 | 123,066 | 31,919 |
| `fixed_pipeline` verifier only | 0.1851 | 0.1036 | 0.1811 | 0.0125 | 0.8024 | 0.00 | 142,361 | 36,639 |
| `fixed_pipeline` estimator only | 0.6722 | 0.6394 | 0.0644 | 0.0113 | 0.3165 | 0.43 | 79,299 | 27,095 |
| `fixed_pipeline` heuristic, every 2 | 0.9531 | 0.9484 | 0.0000 | 0.0111 | 0.0358 | 0.69 | 29,095 | 16,388 |
| `fixed_pipeline` heuristic, every 4 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.67 | 22,406 | 12,617 |
| `oracle_evidence` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 2.43 | 0 | 0 |
| `oracle_immediate` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 0.00 | 0 | 0 |

**Compute limit 60,000 ns**

| arm | success | success, 10 faulted classes | critical miss | abstained | wrong declaration | mean probes | mean modelled cost (ns) | sd of modelled cost (ns) |
|---|---|---|---|---|---|---|---|---|
| `heuristic_only` | 0.9438 | 0.9382 | 0.0000 | 0.0113 | 0.0449 | 0.70 | 39,755 | 18,458 |
| `all_components` | 0.1624 | 0.0786 | 0.1818 | 0.0002 | 0.8375 | 0.00 | 76,554 | 13,193 |
| `random_matched` p=0.25 | 0.2276 | 0.1504 | 0.1724 | 0.0002 | 0.7722 | 0.03 | 78,825 | 20,834 |
| `random_matched` p=0.5 | 0.1811 | 0.0992 | 0.1818 | 0.0002 | 0.8187 | 0.00 | 76,791 | 17,128 |
| `fixed_pipeline` verifier only | 0.1787 | 0.0966 | 0.1818 | 0.0125 | 0.8087 | 0.00 | 88,272 | 18,856 |
| `fixed_pipeline` estimator only | 0.2665 | 0.1932 | 0.1609 | 0.0113 | 0.7222 | 0.07 | 53,371 | 14,773 |
| `fixed_pipeline` heuristic, every 2 | 0.9496 | 0.9446 | 0.0000 | 0.0111 | 0.0393 | 0.69 | 28,880 | 15,804 |
| `fixed_pipeline` heuristic, every 4 | 0.9540 | 0.9494 | 0.0000 | 0.0111 | 0.0349 | 0.67 | 22,406 | 12,617 |
| `oracle_evidence` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 2.43 | 0 | 0 |
| `oracle_immediate` (privileged) | 1.0000 | 1.0000 | 0.0000 | 0.0000 | 0.0000 | 0.00 | 0 | 0 |

- **Undecided is 0 in all 220,000 rows**, and every stop reason is `terminal` or
  `final_declaration`. (A6b's final call turns budget exhaustion into a declaration.)
- **Variance.** Of the 440 (arm, budget, class) cells, 137 have a success rate strictly between 0
  and 1 and so a non-zero sd; the rest are 0 or 1 throughout. Within a class the per-episode
  modelled cost has a coefficient of variation with median 0.31 (10th to 90th percentile 0.22 to
  0.42) over the eight public arms at 20,000,000, and 0.07 at 60,000 where the compute limit caps
  the spend; the per-episode wall time (`measured_policy_ns`) has a median CV of 0.37 and a
  maximum of 2.6. The modelled cost is a deterministic function of the episode and the arm,
  so every bit of its variance is between episodes (class and world); the wall time adds host
  noise on top.
- **Privileged arms** have modelled cost and bill 0 by construction; `oracle_evidence`'s planner
  has a measured wall time (about 48 thousand ns per episode) that no cost column prices.
- **At 20,000,000 nothing binds**: success is 0.953 to 0.965 for every public arm, the highest cost is `all_components` at 522 thousand ns (the largest declared
  bill in any episode is 1.55 million, 8% of the limit). Below it the multi-component arms
  collapse (`all_components` 0.291, 0.182, 0.162 at 250,000, 100,000, 60,000), while the heuristic
  family does not (0.944 to 0.954).
- **Arms with identical success.** `heuristic_only`, `fixed_heuristic_every2` and `every4` have
  identical pooled success 0.954 at 20,000,000 and 250,000, and `all_components` and `random
  p=0.5` have identical pooled success 0.9649 at 20,000,000. For the pairs
  `heuristic_only`/`every2` and `all_components`/`random p=0.5` the success is identical episode
  by episode at 20,000,000 (and the first pair at 250,000), which `b2-power.md` uses.

## 5. Checks that were made

- **Protocol replay across runs.** `heuristic_only`'s `results.csv` (all columns except `run_id`)
  is byte-identical between the B1 run and the supplementary S1 run at each of the four levels,
  although the runs have different run seeds (1 and 2), different arm sets (ten arms against
  four) and different arm orders: the arm's result does not depend on them.
- **Hard limits** were enabled in every arm of every run. At every binding level the largest
  declared compute billed in any episode of any arm equals the limit and never exceeds it
  (details in `b3-stress.md`).
- **Stop reasons and undecided**: above.
- **No run failed, was interrupted, was killed by the backstop, or was excluded.** Exit status 0
  and zero OOM kills in all 18 runs; no row of any `results.csv` was dropped, and the CSVs
  here are computed from every row.

## 6. Supplementary runs (not part of the specified B1 grid)

Cheap runs, made after the B1 grid because the B1 data asked questions (each is a full run, listed
in section 3 and in `b1-supplementary.csv`, which has the B1 schema plus `family` and
`noise_rate`):

- **S1** (levels 20,000,000, 250,000, 100,000, 60,000; seeds 1000 to 1499; run seed 2; four arms):
  `heuristic_only` again, `random_matched` with `p = 0` (nothing ever selected: the cost of the
  shared decision rule alone), and `fixed_pipeline` heuristic with `every` = 8 and 16. Purpose:
  the cost floor, and whether a longer period keeps success, for B4.
- **S2** (compute limits 15,000, 20,000, 30,000, 40,000, 50,000; same seeds; run seed 3; nine
  arms: `heuristic_only`, heuristic `every` = 2, 4, 8, 16, estimator only, `all_components`,
  `random p=0.25`, `oracle_evidence`): where the cheap pipelines themselves start to bind.
  Purpose: B4's recommendation.
- **S3** (the four B1 levels at generator noise rate 50, the generator's maximum; same ten arms;
  run seed 4): a noise dose probe for B3.

These are development runs of the same kind as B1; they were chosen after seeing B1, so they are
exploration, with no pre-specified question and no multiplicity control.

## 7. Deviations from the plan and the brief

1. **Budget sweep instead of default limits** (review log, A6): the brief's, not mine.
2. **Interleaved multi-arm manifests** (A8) rather than one run per policy, so one run directory
   holds ten arm directories. `analysis/` reads each arm directory unchanged.
3. **`random_matched` is not compute-matched** (plan A6 sketch: "sized to match a target compute
   bill"; the build is independent selection with probability `p`, and B1 as briefed fixes `p`).
   Its cost at 20,000,000 is 0.55 of `all_components`' for p=0.5 and 0.33 for p=0.25.
4. **`fixed_pipeline` defaults are not a separate arm**: `fixed_pipeline` with all four components
   every step is `all_components` by construction (A6; a test asserts it), so
   `all_components` stands for it.
5. **`b1-variance.csv` has a `run_id` column** (the same `run_id.arm` as `results.csv`) that the
   brief's column list does not name. Rows are one per (arm, budget, class); ordering is budget
   descending, arm as above, class in `EpisodeClass::ALL` order.
6. **The brief's "mean probes used"** is the mean of `probes_used` (`results.csv`); stop-reason
   counts are the five reasons (`terminal`, `final_declaration`, `budget_exhausted`, `horizon`,
   `step_cap`), the last three of which are 0 everywhere.
7. **Supplementary runs S1 to S3** (section 6).

## 8. What I am least sure of

- Wall time is not trustworthy on this VM (burst steal, section 3). Nothing here depends on it,
  but `measured_policy_ns` columns in the CSV are noisy and the host's recorded frequency is a
  nominal figure.
- The table's per-arm numbers are for these 500 seeds per class of one generator at default
  parameters. Two arms having identical success on every episode (several do at 20,000,000)
  makes some paired variances exactly 0; B2 discusses what that does to power.
- The ten arms are not a tuned set. `every` = 2 and 4 are as the brief specified; 8 and 16
  appeared only after B1 (S1).

## 9. Hashes

`manifest.json` is the run directory's own canonical manifest; each arm directory also holds a
one-arm `manifest.json`. `results.csv` hashes are per arm directory.

### Pilot


`b1-pilot-c20000000-s1000-1099` (manifest.json `853658437e6690502e1a057f80e411f230395c8862ed2f1ef3780f27917891c4`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `fa2cc034d317ca6cc4cee57caa4a36e26e9ff0d679c931a4342622d84d46b218` |
| `all_components` | `730350c613c673cbe315a58d5e0c6f5bc438b8da799e2f97717fd6c032497ddf` |
| `random_p025` | `5fb879c77bc0ea7e93dfd571b4dfbd8443b86b70a2f4de1559b6fdb3ee5cbdcb` |
| `random_p050` | `a2f4eb26ded483ee8bee12ffebf6649760c6c1ad1d707de2488411a025f1d0ed` |
| `fixed_verifier_only` | `c7faa299c4d611fe9c6ca5c19fe50cb65fe5b1498cb0c5552657ce9617ad2402` |
| `fixed_estimator_only` | `7848b214c8794b3da7f04bfdde0ea7af23fca3ae90ef016d8b493912a3388040` |
| `fixed_heuristic_every2` | `39d81cf4670add26192897eb21b28f716f6df227c1118e3b398e389052fbee41` |
| `fixed_heuristic_every4` | `3a9ccb47f91ae6d3c3dc843f1617f44f46ebdf6d87fc41020e3b11e2f550d87e` |
| `oracle_evidence_privileged` | `417b46bf0e25707bb1e674513396bc9836f8a7344ba6fd7a22bec8ce01b243ed` |
| `oracle_immediate_privileged` | `e6a92cf8f9e76c0c065cd8e0a92661dcb597390d91f5354f9070a52413d4e94d` |

### B1


`b1-c20000000-s1000-1499` (manifest.json `bb70350cc8423455f4fd361a7cec387f450c6e96e188d12ee73814f9fdf87c81`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `f11429f583c75541075d8a5c9311b8ec43ef410dcc26a3114b381182959a5cd4` |
| `all_components` | `defda3eafc0a7b97f507ba5eed6aad78ade95d34d34aa183876c8754c0a1d061` |
| `random_p025` | `5bcc907abf3c403b70c1eb270d4e5238e8c4400fb52b9f1c6df14243151f30a8` |
| `random_p050` | `1a4f431ba888057d45388ad115d489f67fb9e707237e49ee0cc1cc71a415cf5c` |
| `fixed_verifier_only` | `1f4e70fd8aea44a1f4aa322e54972940f45820a64f5d24992c68a8213cff7ade` |
| `fixed_estimator_only` | `daa748386a09e343653d79b0d0bcc3d0a9c434336e041666a3c23b32b34589bb` |
| `fixed_heuristic_every2` | `800c65db7efa4cae4a7930ec84f0bb06d6ef75f9122d4458b9a999d004399909` |
| `fixed_heuristic_every4` | `5be55c2190ad4067b51387b18579289e9fcf6e353b9c76b75bbdc35499e85112` |
| `oracle_evidence_privileged` | `cfa9fcc08602aeec7e3c0c0c25931f35856cb5cecc730907b9d35f6670d5a162` |
| `oracle_immediate_privileged` | `f9eacdb14ad14a6af172439e2d74d24b45c966c85e8047b03e58a73d0386fe0b` |

`b1-c250000-s1000-1499` (manifest.json `5197a2ea32c4d004b8492fff96a7d88d6b1352b0936181fa06bfbd62b7083a69`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `fdc73ad9429f9d7b895543f767491616c42414e4e81be3189fd2c17db67a7277` |
| `all_components` | `1b8f1828e8f871bc29ba54650a7e55dda0f0cd0b84985af6f492435c7f433803` |
| `random_p025` | `5598f67451e07bff1afdd54e3bd8d41b880b71cca271c52ff959b043ac65bd56` |
| `random_p050` | `6f6b1f1ee432cbf9ebfcb2a662a101f2bbf5483137ad3fa9b682dbef53a45a13` |
| `fixed_verifier_only` | `746c60ac712923131df7a7418bcea211aa18bf4f691455a73e37a254e6835380` |
| `fixed_estimator_only` | `37d348b731951fa318a2cb5082bdde3896b3908caeaea4a98331d37023263a27` |
| `fixed_heuristic_every2` | `3bab0e8860b8367c0c57296d650dd524b57003612bfb0e591b392fab7de150a2` |
| `fixed_heuristic_every4` | `4d5a59064ddd5e55e1aff7c25949e67b6f126c11283b7d2a9256257fde728577` |
| `oracle_evidence_privileged` | `0f9a0409c070a88e49df78defc161c2a4ced6fb6dca2f8fd54ab79add451d924` |
| `oracle_immediate_privileged` | `85663ba25f22cda7359bc9367f0aa6ddbe78e12e6e64aaf3f796b898c5e10eb8` |

`b1-c100000-s1000-1499` (manifest.json `3b752402b8151a58792946c57964009a4f33115461ffebf1a831c2561f85777e`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `3d84287a9f4e44330c4b4aaf56cbb47814fde58ccde7bccd5c41a586d20b8638` |
| `all_components` | `6589c500f88f5a4f7d399b209a50de99d353ca0108c09b0ccb35c53bf5ca78cc` |
| `random_p025` | `1fde19882022b98f35adc7168638d5e2399b417777b3c54aea67718178d97967` |
| `random_p050` | `634348846fd42ea8677e833ef7b59ddb9084ce41da81eb411c7b13428f705487` |
| `fixed_verifier_only` | `327b4a8c4aefa38d9489b8be7228cee1da38257241bc4130f0e0f47a0974a2d1` |
| `fixed_estimator_only` | `e9b9114b8bcf074962f8241d89de8f8f0bbce0b4b4cad67af2d8cc76f1e7d8c4` |
| `fixed_heuristic_every2` | `fa1326ed218a069052d2825d2b55a8604c7d1796c15aa29ce28ae38e2e227cb6` |
| `fixed_heuristic_every4` | `b93a04545c3009f94f1f2e6d1f70313df1a922641d4459ed3f54a364c8c7a97a` |
| `oracle_evidence_privileged` | `8c566dc99df4f0dd9eb50304179109e9acd3b2382c2c3db8fd06f7950b942800` |
| `oracle_immediate_privileged` | `fb2dc377281e1d46d191fd295ae26de9b19354deca92850f5b856ed846fca38b` |

`b1-c60000-s1000-1499` (manifest.json `aa17c7961d1e0ec3a18df11daf1c36dfc738d16f55e627b8611a37c6d7a0ba93`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `095d4f4d122b232f78c1e3da9f12f36ebc1aa2df3e0533f93b2327f2490ee4ec` |
| `all_components` | `3a177fe1bcab89c245380832f4344f92c1674face83c9af8dce782aae0b0005a` |
| `random_p025` | `4f4e4f1f6a7d1aa9a145dd99ebe310d580f0b957866206a9c215f8b7cb29013c` |
| `random_p050` | `3cdb34f9108deda67882df313b4548d925edb35a2abacaa7ccdaf610f4953cab` |
| `fixed_verifier_only` | `043841d0778a140769fa7a37ee1e7a831b595db17cf8cde9ba30df9db2a952db` |
| `fixed_estimator_only` | `27ecf2782d0ac65fc4e3c1f3674b9e8851089188d67d807076ad52df980c4fd0` |
| `fixed_heuristic_every2` | `5d1baf34c2ad4a0ea5fdff2a64d7db5acfbd8f0c743fde0677c9b59f4ef55b0b` |
| `fixed_heuristic_every4` | `976cf419dab11ace57a9f92f79b51610dd112b8b7182d137d64a521f2f692651` |
| `oracle_evidence_privileged` | `da9a9c8b32680f388cdd14da589214b18197f2434513639bc94f68f1c50b41e7` |
| `oracle_immediate_privileged` | `0cdbec1749ee09199a6d15e660fa7c69f808855789b59f5a2de79bd917d859a7` |

### Supplementary (S1, S2, S3)


`b1s-c20000000-s1000-1499` (manifest.json `d9d6c9ee3a19048ca1e897cc52f8d60ee76b5d205a1b0494a948af0ae7fde200`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `830189151f5ea5225fcc181822919d739a7cdf147f8f7b9a90e12d550e1e6e5b` |
| `random_p000` | `fabc018ff8f3c5dfcd8eeb23112fc069dab56147cd426bec9895cdde6eb3969b` |
| `fixed_heuristic_every8` | `a942d3719b0c63952038bcb1796340f4ff86e905a78c02422f773fca9e373db2` |
| `fixed_heuristic_every16` | `c2c7c9c4241cfff9d9f732addcca0007132287ffa2c98acdb5ac088f5fe77cc9` |

`b1s-c250000-s1000-1499` (manifest.json `7b6d8bb49cc6a629b2f13400b61f9da3c5362239d66370f592f5d118fad8945f`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `9f1f09adf866329ab39d3ccf0bfe326f8159ed7df5b792ae7bf3e98db4e003ab` |
| `random_p000` | `51862679024dad4b59d867878c781bc6f07ba5e2dc23a224118eaab3283613b0` |
| `fixed_heuristic_every8` | `c2ced4039f4c5a33c10075037a6b75bdbbac120dbfe1ec6b3350acd8a12942e9` |
| `fixed_heuristic_every16` | `e8c0fab8e09c1594fa7db957b1432d9c5378b79fdece308b454e2e89cc30b4ec` |

`b1s-c100000-s1000-1499` (manifest.json `a72ac64662d3918ddd2c57b4d6d5d168c66b70eef72733dbd99c5b019cb62231`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `b267848841a8d8bd91cfa4de040c9bb847212d98bd7f27df6da77d78446b041d` |
| `random_p000` | `33482ad952e36ef32dc83efcb2d22b70282f774ac4910b3fd405d6910849d592` |
| `fixed_heuristic_every8` | `434b057a3b1038e6d5f2dc3cbfa76a591ae9cb6c3f776b7aa3f035922588e439` |
| `fixed_heuristic_every16` | `79610dfc19ccc91f1af4eb2df1e961c9cd6bb44b295c0e39948e16900a65aec3` |

`b1s-c60000-s1000-1499` (manifest.json `19ac6cb80338eda65de2fb653854d6482ccd9ed2c60f72ccc4c20cddf0ab24fe`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `189979f58c06e0b63e0ca0f1e3b63b267740a2c51886e584481c3695786af23c` |
| `random_p000` | `5968f8e25f055f54c40fc93e3275aab461fff5301a45be5f543c3fb532706a52` |
| `fixed_heuristic_every8` | `ac993f76d7305273ccc92458e8ca6811c464e24ae30bd510aa69ad6e18a6f21e` |
| `fixed_heuristic_every16` | `2fc03f949d05fe629bac291a94c05dd282a2dd752e43fc46b2864fae9a915a9b` |

`b1s2-c15000-s1000-1499` (manifest.json `6ddf48b671a40dcf346afe2b8ac34efb3b00444a5d974de9d321e6fcb287d8ab`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `477d35980b862c0dcfe9bf6b5198d37e2d499d95abb4398c2f65c49d94ecfa78` |
| `fixed_heuristic_every2` | `42f5a8f2da3f911f7d3af16d5e4e4edf7a2b5ed7023df6ab12efe95207a37d05` |
| `fixed_heuristic_every4` | `8e25ec23553c7b912e29cf406a90a6d39daf2c86602b0230882bf05eb3efd415` |
| `fixed_heuristic_every8` | `0ac667cf7379730edb9b778b39bd58f397b5b13b9f922aec19a74ecde7bf7648` |
| `fixed_heuristic_every16` | `1b6333174c8c3cb2a2763c96ce48f3bfd21882962b8a630824747e454f2c1d53` |
| `fixed_estimator_only` | `40ad2b6167d1d3cac9a97448ce1bf13faf1a21341746607d4c0d385bf62c216a` |
| `all_components` | `5a871ff7618105c8ae9c8b5ef99e1c9350d702325ef98f9eaac96b31db572a99` |
| `random_p025` | `33b63fac0890d9d2d7eac2d7d8e3bfb44617b9fe141eb48f4317444918b642de` |
| `oracle_evidence_privileged` | `09c9cf3cd5bc2dd9d1a3db357619d229825e261899a1a939a5b5648f70dd0f69` |

`b1s2-c20000-s1000-1499` (manifest.json `4de1ef999d49f9761747fd13bc2604bb2d71c9b6ea75f2b47e0fe2ed2bd1dbfb`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `657de2efeb208fba4afbbd8f6caef521dfb5178fb86de1c4d41fcc84a31e2d1f` |
| `fixed_heuristic_every2` | `01f25d8b63f6bd32fd4d0e702f0079c7940a8c70795f806601788d36ee4bd7f9` |
| `fixed_heuristic_every4` | `95ff49119db2c238b680881a97093e8886b7ef5e3fa0776f79c77169d05f94df` |
| `fixed_heuristic_every8` | `d563674423a5873a0ab847deba5c6c40c6f071df5b34ba15d0648b1291d37d6c` |
| `fixed_heuristic_every16` | `5aca0ae6865ba480b1ae074acb002c360804230c1f1f99aa649c80058be801a4` |
| `fixed_estimator_only` | `787e3c58ddc8c2c897d5bb8bda0b8897cf61c7276a6b505640be045353e34d87` |
| `all_components` | `748d7c345b8db872c3a2457ade4ae2c85e8e475c60b5bf291ac6c1e5c0c45ae5` |
| `random_p025` | `d1dd78cffa693634cb9fd1c83e7b6093a4b932a9f0cc46d88e3266785ee5b88b` |
| `oracle_evidence_privileged` | `c4339f74ef45f1bea0fb0f0e1a3d3726748a9988362f2629bd76d68451bb2ca0` |

`b1s2-c30000-s1000-1499` (manifest.json `62ffa6445c130d3bc7fb5e4aced992630d2c526bea49356925c1ccb2bc2d042a`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `38ca2331cf600515548f9b4d4208764992261d5517356ab513319cf3bbd8401c` |
| `fixed_heuristic_every2` | `cb883f15676a4d71379773b3b6057a97ba98b70f9b845dbae6a6fc372ae19466` |
| `fixed_heuristic_every4` | `3dd7dfc532ec0e8d35af8465d6ad39b4dc139b87a1b2ab2058621a504bb10b26` |
| `fixed_heuristic_every8` | `99c2bf41c0809e63864fa97231cec95e97f0b8a29248cf0ee9263dd42b7f467e` |
| `fixed_heuristic_every16` | `d087273b659b0df8890b4d7d5ca7f42bc429b2a03660e8ec61a1334c32560ee3` |
| `fixed_estimator_only` | `7b18c0261b53ec495431fe444f02a80d04605249225cd923ada305bab3463c1d` |
| `all_components` | `9827079463a84a0d2b91c40419b01fd0dae269a7141faba4f916ae45856a4507` |
| `random_p025` | `070f2b575aafdce0ec512dd7c4e652ce8ba5c7aa657dd87d1a3cf8393e73526b` |
| `oracle_evidence_privileged` | `561d15bb04e6bca0acaffa83d81ae37f5032e43c38c5b77289bc1f1e3c628418` |

`b1s2-c40000-s1000-1499` (manifest.json `3f425bc8ca8af192b5ec5a67142e56c2640f1cbb7411a8e729a13e2ea7998e96`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `42f776beae0c290196c2ba76206be1f7667d28da397fa3c91415921a62a7ba17` |
| `fixed_heuristic_every2` | `43791f3d7ee79db7728d6cc64e99daba7d600aeb39c91f06fe7a694bc1bf0eea` |
| `fixed_heuristic_every4` | `b70881b33a9480a26034411f8e50820892156d26044af0235d2c2236d9de3fd3` |
| `fixed_heuristic_every8` | `faa85e412dd6db807321dcba3b5012b42169773a6b6131acff40a0a151d3a7ef` |
| `fixed_heuristic_every16` | `152a8b110518b828c9a973fa74a14126204eefb9d1b84d3fb8d328e2720e52b1` |
| `fixed_estimator_only` | `b14e078dab00806eadf51fd62ed718cff6cae2bd6e5ca7fffea8e0da92025318` |
| `all_components` | `10bf83392c30be3cfe256cd1117ea1cbd595214910d8230e96ceb10e01f3d9ab` |
| `random_p025` | `f6ca5dbb81aa669df34d1fda407807bfd13cc05505b4da57c01f87e15bd9aeef` |
| `oracle_evidence_privileged` | `384e74ef6afbd4de88be4021320904199f9afdaecdd0d1aae6990645a8583168` |

`b1s2-c50000-s1000-1499` (manifest.json `a55a6c18403d929d821b399625add8471833e0048b63bcdb12b82e64a0c748ca`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `2e51283208689bc1e0882b1709f6357ed3cfb3d68bcddad1c9e462aa6725c5e3` |
| `fixed_heuristic_every2` | `fd2aa3e64dc4a85d2b53e171dafb5222966c38bd8d902b35b4202831af58fd30` |
| `fixed_heuristic_every4` | `38c8e91d270d1a1d9f191d5acee2b9d0b916a8a9e25ed2e10fd2c5182a446747` |
| `fixed_heuristic_every8` | `34975940f0961c21c5f1fc7d5244e94e371b7c3b9911adfca94c0249e2323fbb` |
| `fixed_heuristic_every16` | `67fe38b634d992f788fe7ef0544456dadd2c404d1de4d219edc2e38559cecb6d` |
| `fixed_estimator_only` | `5b791e2ee1694171f9b6d4f695aa544b31c8d5db8fd8d2158b4bea4eae00e4ed` |
| `all_components` | `a1d371abb6372c9e0c7b1e4e6d467255aa6bcfe31e318c311a87c363a56237d2` |
| `random_p025` | `1278be58d7aa52b4d13355d8ff32e7ed5364b056fbe3517a32c93b8461049243` |
| `oracle_evidence_privileged` | `2757b15841dc31d4efcce896999e9e51bb870dd2d39c5252d58d9488e0611494` |

`b3n50-c20000000-s1000-1499` (manifest.json `e6663642a5c9b782d1b1ee37d82af2fedf3c6b616d825de92a6e23c652602c71`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `5dae048dd175997dacf1baca3af05d5dd2d842a51144a39193776762e9e539f4` |
| `all_components` | `b3e7ceb2e18ec7ccef2500645ea6bd4e28a30743a9888d3194e26f217b716c6d` |
| `random_p025` | `c4a6c8ebda626f2150f3a695861a31a082ec04aa502b01b5d7c769f91afa6c90` |
| `random_p050` | `51e83e85554ac698be3efd830609257a2a5b6c09be7f253ff2185c4276d6ec1b` |
| `fixed_verifier_only` | `f0b5302c11668bcfa2640345599c66d037c0e714b9344b6ed0c0f81c05a1c19d` |
| `fixed_estimator_only` | `e5625fd1525f80e9a18d5ef51275a8d0881847cc59407792a301fd0283967a93` |
| `fixed_heuristic_every2` | `adcf4d4ff3c51569ae18cdc4bf4f5d7eef47d144a91beae2a802e5f1f38401c5` |
| `fixed_heuristic_every4` | `a5dea24e04f182c819f0d4197a290ab133942b761dd92967f12f443c8e365f38` |
| `oracle_evidence_privileged` | `e79da760c228c68eb914fa247b625a8e37255ae184b02e8884ac42166beed2af` |
| `oracle_immediate_privileged` | `63abee6a4fc95ff746fa5aca6edd9c67ee124a74dff12fe16946cc804a9a2611` |

`b3n50-c250000-s1000-1499` (manifest.json `b41116f617245e6ed52a9acf7bafa0a4fdceba68b6c52a324bf567812dd9fbf9`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `ec4ddee1a33242f38bdcf4bfd16810edb434e6399d7221735c29333968d313cc` |
| `all_components` | `6654eac0878293b6ef75e23231839ee1ad8b3d6647b1e39aaadef4b7dd7d5435` |
| `random_p025` | `6c78305a38435c53a4efc2d0267807befcba253c86fb630bf42adf90ef8d3afc` |
| `random_p050` | `c16de4509f775cfc21eab729eba43376b7868c358db9ad45c035e773d3da7b37` |
| `fixed_verifier_only` | `6d13f24a7d91c2402622a747747b188f37c3789a420c552fbc743dddc3015053` |
| `fixed_estimator_only` | `29743c83b34314e8c644613b4346bfa50a4d17f6aebea667e44df74de5077cc0` |
| `fixed_heuristic_every2` | `fe506bfa6eec09eb77f9b6a015ae2b89a30fa679282b79874a95f2f3c52de862` |
| `fixed_heuristic_every4` | `e2772009aaac0f1784b5cb9657019e137e5cc9708fabea7eb8eefae47564bcf2` |
| `oracle_evidence_privileged` | `ea0bf004352d0fa2d309b1f1fdf747eda021752baa92ae963760a197bc2a6eef` |
| `oracle_immediate_privileged` | `478fe4d0dd1a31a622e113cf467f9eb091aa15f0f4d3c8f2e8b500d2c515abb6` |

`b3n50-c100000-s1000-1499` (manifest.json `7d7d87e080868e4b610b73694ac6fa7389ea7339bb747073656103c2ad510b93`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `f469387f7f4b44b71a55e2efe9cb2b5a08df345ebc8c8ed69f7332d907ff2916` |
| `all_components` | `d24c565f09d8151d51b36c977fc670eefe5e02884a87ad3bea667914dde26766` |
| `random_p025` | `f49aa77015f6980773b9cfdc318502b0a63cf922dcb38b4960f940101213ac79` |
| `random_p050` | `4dc7d5060abf3a2a7d77ede64fe5b4a9c02724427a9b34fd2a02c44ab90e6c10` |
| `fixed_verifier_only` | `344e761828ffd41e00cf98cea1e91702353d846a93c28285a1c3d60af4fb740f` |
| `fixed_estimator_only` | `f72c607852c72cdbab4d7f632e8237c078bb2aec7315871b5254b4f332500319` |
| `fixed_heuristic_every2` | `e82541fb87775bccd70e260047fcebd881fea5043334baeb7ab286b6b814e42e` |
| `fixed_heuristic_every4` | `1a99038aa5308b8f9f8a805aa981f585fc14b7b75deee60b33645cf61b0ab2f0` |
| `oracle_evidence_privileged` | `c462d86d71921d7ce977a80183227dbe068203e479a910931c811050eb6329e7` |
| `oracle_immediate_privileged` | `c88c011bad0b473a3ced98e9cffce21cfbad6d70c2474fcae4a0f3297c09c9de` |

`b3n50-c60000-s1000-1499` (manifest.json `d8e9cd0e4405afcd7af40305f2b3ae361c9cd2f574be178567ea48811f65953b`)

| arm | results.csv sha256 |
|---|---|
| `heuristic_only` | `27beca93a57d2c3d90768ab6e1ce108a2fc0d048dc3bf4a3090e2c3ba6fa0f0f` |
| `all_components` | `e822590c4fda955b8b1a4d878e4ec5129a4ab55b412390554fdcf2fdf9e68ad7` |
| `random_p025` | `85829055998a36dc2441bd2df90cc32744b45ffcc5d194f6a1e5179df6f0bc1c` |
| `random_p050` | `53da355f46ae199701d5c4e37212e5f69ab3e57397de36857d47d1f4400b631c` |
| `fixed_verifier_only` | `cb2082b88f892ad3cf62be174905c40bdd7b2bccf57d592c490ad39b72bbd4b0` |
| `fixed_estimator_only` | `280c6ade783770995319f2f49d1def64b387ea76f13ffdec659cc7008dd89c1d` |
| `fixed_heuristic_every2` | `3cdcdc929542975f170cc14c5978abf052f3b9366934f0ee5faf2b41aa08e7e6` |
| `fixed_heuristic_every4` | `17f28e6bd3a1a6cd48d0faa0e9bd16bd1173e34b86f3b068beef5730fc6959ed` |
| `oracle_evidence_privileged` | `513aff44a480031e0cd8f82c1f90615739590ca60283f7b592d67e475590a647` |
| `oracle_immediate_privileged` | `c5eb51f111a2196b3aa3174095ce18de2be4ca38561a4a92b16c38d43cc9366a` |
