"""L2, the reservoir noticer: the settings, the arms, the readings, the tuning rule and the
constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## L2"; clause 2 amended 2026-10-07, before any L2
code), fixed before any L2 code or run: its seeds, margins and bounds are copied here and not
changed. Every reading of something the brief leaves open is stated here, before the tuning run
and the fresh run, so that the manifests and the analysis cannot disagree. The measures are the
evaluator's; the arithmetic is B1's and B2's (`b2_stats.Measures`) and L1's (`l1_stats.Window`),
reused unchanged, and W1's and L2's functions in `gordian_analysis.measures`.

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import l1_common as L1
import m2_common as M

ROOT = M.ROOT
RUNS = M.RUNS
MANIFESTS = M.MANIFESTS
OUT = M.OUT
BIN = M.BIN
NS = M.NS
MS = M.MS
PRIMARY = M.PRIMARY  # b = 5, rho = 0.7
setting_id = M.setting_id
arm_name = M.arm_name  # sel_<name>_privileged: the selection oracle at 16 s, the rung's context
BOOT_SEED = M.BOOT_SEED
N_RESAMPLES = M.N_RESAMPLES

TUNE_SEEDS = (10_000, 100)  # M2's tuning streams: the only seeds tuning may use
FRESH_SEEDS = (40_000, 200)  # L1's fresh streams, never used for tuning
# L1's run seed (it draws the arm order per stream), so that the arms L2 shares with L1 (the learned
# medium, the frozen graph, the re-anchor, ramp + split) are the same arms on the same streams and
# can be checked against L1's run for byte identity.
RUN_SEED = L1.RUN_SEED

REANCHOR = M.REANCHOR
RAMP_SPLIT = L1.RAMP_SPLIT

# The seed of the reservoir's fixed random weights. One value for every arm and every stage: the
# network is not tuned, only its size, leak, radius and the threshold are. Chosen as the date of the
# brief and never varied (a sensitivity to the draw is a thing to test next, not here).
WEIGHT_SEED = 20_261_007


def reservoir(key, **over):
    """A reservoir arm as the manifest spells it: learning on, carried, the fixed weights' seed,
    input scale 1, 100 ms ticks, ridge 0.1; the four tuned quantities are the defaults of
    `ReservoirParams::standard` unless overridden."""
    spec = {
        "noticer": "reservoir",
        "seed": WEIGHT_SEED,
        "size": 32,
        "leak": 0.3,
        "spectral_radius": 0.9,
        "input_scale": 1.0,
        "threshold": 1.0,
        "tick_ns": 100 * MS,
        "learning": True,
        "carry": True,
        "state_key": key,
        "ridge": 0.1,
    }
    spec.update(over)
    return spec


# ---- the criterion, as the chief fixed it (docs/lab-queue.md, "## L2"; not changed here) -----------
#
# On seeds 40000-40199 at 100 ms:
#   (1) the ESN's anchor-correct share over the last 100 streams is at least the frozen graph's
#       minus 0.01 with the paired lower bound above -0.03, AND its background notices per stream
#       are <= 6.82 and its strict precision >= 0.67 over the same streams;
#   (2) its background notices per stream over streams 21-40 are at most the learning-off control's
#       minus 10, with the paired lower bound below -5 (the endpoint, stream 20, is fixed);
#   (3) the learning-off control's end state is below the frozen graph's by at least 0.02.
# The three clauses are conjunctive. A fourth, reported separately: the ESN's end state against
# L1's learned medium, paired.
#
# Readings fixed before the tuning run and the fresh run:
#
# R1. "anchor-correct share": the hard non-leak incidents' anchor-correct share, the evaluator's
#     N5 over the incidents outside the slow-leak family, pooled over the streams of the window (a
#     ratio of sums), as every M2, B2 and L1 table.
# R2. "paired lower bound": the 5th percentile of the paired 90% cluster bootstrap (10,000
#     resamples of whole streams, the same counts for both arms; B1's arithmetic, `l1_stats`).
#     Intervals everywhere are these 90% intervals.
# R3. "end state" and "the last 100 streams": seeds 40100-40199. "Below by at least 0.02" is a
#     point difference (frozen minus control >= 0.02); its paired interval is reported.
# R4. "background notices per stream <= 6.82" and "strict precision >= 0.67": point estimates over
#     the last 100 streams (mean notices on background per stream; the pooled ratio of notices
#     that are anchored on an incident and at its site, over all notices, the evaluator's N16);
#     their intervals are reported beside, as M2 reported its bound (M2, reading 2). The
#     re-anchor's strict precision on these streams is reported beside as the bound's origin.
# R5. Clause 2's window is streams 21 to 40 of the run in stream order (positions, 1-based,
#     seeds 40020-40039): "the endpoint, stream 20". The measure is the mean notices on background
#     per stream; the difference is ESN minus learning-off, paired over the 20 streams
#     (`gordian_analysis.measures.paired_window_difference`, the same 10,000 resamples and seed
#     as every other interval). "At most the control's minus 10" is the point difference <= -10;
#     "with the paired lower bound below -5" is the 5th percentile of the paired difference below
#     -5. As written the second condition cannot fail when the first holds (the lower end of a
#     paired interval is not above its point), so the 95th percentile is reported beside, with
#     the stricter reading "upper bound below -5", and the report says which reading carries the
#     verdict and that the clause's wording admits both.
# R6. Clause 3 is L1's clause 3 with the ESN's learning-off control: the same reservoir, the same
#     threshold, the readout at its all-zero prior.
# R7. The fourth reading is the paired difference ESN minus L1's learned medium of the
#     anchor-correct share over the last 100 streams, with the 90% interval; the background,
#     strict precision and leak measures are reported beside it.
# R8. Sample-efficiency curves are W1's `sample_efficiency` on per-stream counts, as L1's R5, for
#     all six arms; the slopes over the first 50, 100 and 200 streams are L1's R3 and R3b, reported
#     for continuity (L1 showed they cannot see learning that completes in a few streams; they are
#     not clauses here). Background notices per stream and strict precision are also read against
#     streams seen in blocks of 20 (`block_curve`).
END_STREAMS = 100
CLAUSE1_MARGIN = -0.01
CLAUSE1_LOWER_ABOVE = -0.03
BACKGROUND_BOUND = 6.82
STRICT_BOUND = 0.67
CLAUSE2_WINDOW = (21, 40)  # positions, 1-based, inclusive
CLAUSE2_GAP = -10.0  # background per stream, ESN minus learning-off, point at most
CLAUSE2_LOWER_BELOW = -5.0
CLAUSE3_GAP = 0.02
BLOCK = 20
SLOPE_WINDOWS = L1.SLOPE_WINDOWS
CLAUSE_SLOPE_WINDOW = L1.CLAUSE_SLOPE_WINDOW

# ---- the tuning rule (fixed before the first tuning run) ------------------------------------------
#
# What is tuned: the reservoir's size, leak rate and spectral radius, and the residual threshold,
# over the grid below (GRID), on seeds 10000-10099 in stream order (every arm carries its readout
# across the 100 streams, as it does in the fresh run), at b = 5, rho = 0.7 under the selection
# oracle at 16 s with the rung's context, as every noticer in this program is scored. Nothing is
# tuned on 40000-40199, and the fresh seeds are not run before the tuning is frozen.
#
# The tuning window is streams 51-100 (seeds 10050-10099): the end state, after the readout has had
# 50 streams of experience, as the criterion reads an end state after 100 of 200. Over that window,
# for each configuration: the hard non-leak anchor-correct share (AC), the leak noticed share (LN),
# the mean notices on background per stream (BG) and the strict precision (SP).
#
# Selection, in order:
#   1. The feasible set: BG <= TUNE_BG and SP >= TUNE_SP, the criterion's bounds with a margin (the
#      held-out streams may be noisier than the tuning ones; M2 kept a 17% margin on its budget).
#   2. If it is empty, the feasible set under the bounds as written (BG <= 6.82, SP >= 0.67).
#   3. If that is empty too, the configuration whose worse bound ratio, max(BG / 6.82, 0.67 / SP),
#      is smallest; the report then says no configuration of the grid meets the bounds, and clause 1
#      cannot hold by construction.
#   Within the set: the largest AC. Ties (equal AC) go to the larger LN, then the larger SP; background is a bound and never a tie-break (the review log's
#   M3 decision 2). Remaining ties go to the smaller size, then the earlier configuration in grid
#   order (the order of GRID: size, then leak, then radius, then threshold, ascending).
# The learning-off control and the fresh run use the selected configuration unchanged.
TUNE_WINDOW = (51, 100)  # positions, 1-based, inclusive
TUNE_BG = 6.0
TUNE_SP = 0.70

# The grid. The reservoir's size is bounded above by the stream's compute limit at the declared
# price (a size of 128 costs about 1.8 s of the 2 s a segment may use). The threshold's ladder
# spans the range in which the residual energy of one abnormal observation (0.25 for its kind and
# its value squared, 0.5 to 1.0, at 0.5 to 1 over its reading) separates from that of two kinds in
# one tick (above 1), by the unit's own definition of the input.
SIZES = (16, 32, 64)
LEAKS = (0.1, 0.4)
RADII = (0.5, 1.0)
THRESHOLDS = (0.5, 0.75, 1.0, 1.5, 2.0, 3.0)


def grid():
    """Every configuration of the tuning grid, in grid order: (index, size, leak, radius,
    threshold). The index is the arm's name suffix and its readout's state key is 2000 + index."""
    out = []
    for size in SIZES:
        for leak in LEAKS:
            for radius in RADII:
                for threshold in THRESHOLDS:
                    out.append((len(out), size, leak, radius, threshold))
    return out


def grid_name(index):
    return f"esn_c{index:03d}"


def grid_arm(index, size, leak, radius, threshold):
    return (
        grid_name(index),
        reservoir(2000 + index, size=size, leak=leak, spectral_radius=radius, threshold=threshold),
    )


def run_id(stage):
    b, rho = PRIMARY
    return f"l2-{stage}-{setting_id(b, rho)}"


def fresh_arms(selected):
    """The six arms of the main run, in the order the criterion names them: the ESN, the ESN with
    learning off (the same reservoir and threshold), L1's learned medium, M2's frozen graph, the
    re-anchor, ramp + split over the re-anchor. `selected` is the tuned configuration's spec."""
    cfg = {k: selected[k] for k in ("size", "leak", "spectral_radius", "threshold")}
    return [
        ("esn", reservoir(2001, **cfg)),
        ("esn_off", reservoir(2002, learning=False, carry=False, **cfg)),
        ("learned", L1.learned(1001)),
        ("frozen", L1.medium_frozen()),
        ("reanchor", dict(REANCHOR)),
        (RAMP_SPLIT, L1.ramp_split_over_re2()),
    ]
