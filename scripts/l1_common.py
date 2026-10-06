"""L1, the learned noticer: the settings, the arms, the readings and the constants, in one place.

Exploration (nothing here tests a hypothesis; nothing may later be cited as confirmation). The
criterion is the chief's (docs/lab-queue.md, "## L1"), fixed before any L1 code or run: its seeds,
margins and bounds are copied here and not changed. Every reading of something the brief leaves
open is stated here, before the fresh run, so that the manifests and the analysis cannot disagree.
The measures are the evaluator's; the arithmetic is B1's and B2's (`b2_stats.Measures`), reused
unchanged, and W1's sample-efficiency function (`gordian_analysis.measures`).

Run directories live in artifacts/runs/ of this worktree (git-ignored); manifests in
artifacts/runs/_manifests/.
"""

import json

import b3_common as B3
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

DEV_SEEDS = (10_000, 100)  # M2's tuning streams: the only seeds development may use
FRESH_SEEDS = (40_000, 200)  # never used for anything before the fresh run
RUN_SEED = 14_000  # draws the arm order per stream, as M2's 12,900 did

REANCHOR = M.REANCHOR  # B2's comparator in the manifest's spelling


def frozen():
    """M2's frozen 100 ms graph, as the manifest spells it (without the `noticer` tag)."""
    n = dict(M.selected()["ticks"]["100"]["noticer"])
    assert n.pop("noticer") == "medium"
    return n


def medium_frozen():
    return dict(M.selected()["ticks"]["100"]["noticer"])


def learned(key, **over):
    """A learned arm: M2's structure, learning on, carried, step 1/4, marginal precision 1/2, the
    rung's burst_ns (0) as the prior window, two readings as the prior ramp threshold."""
    spec = {
        "noticer": "learned",
        "structure": frozen(),
        "learning": True,
        "carry": True,
        "state_key": key,
        "step": 0.25,
        "min_precision": 0.5,
        "prior_window_ns": 0,
        "prior_ramp_threshold": 2.0,
    }
    spec.update(over)
    return spec


# The four arms of the criterion (names are the manifest's; the arm directory is sel_<name>_privileged)
CRITERION_ARMS = ("learned", "learned_off", "frozen", "reanchor")

# B3 changed the public comparator after the brief was written (docs/review-log.md, entry B3): the
# best public noticer is the ramp noticer and the splitting noticer over the re-anchor, as B3 tuned
# them on seeds 10000-10099 and recorded in experiments/exploration/b3-selected.json. It is a fifth
# control row, `ramp_split_over_re2` (B3's own arm name), not part of the criterion, which names the
# frozen graph; nothing about it is tuned here.
RAMP_SPLIT = "ramp_split_over_re2"


def ramp_split_over_re2():
    """B3's `sel_ramp_split_over_re2_privileged` noticer, from the recorded selection."""
    sel = json.load(open(OUT / "b3-selected.json"))
    return B3.composed("re2", ramp=sel["ramp"]["chosen"]["params"],
                       split=sel["split_re2"]["chosen"]["params"])


def arms():
    """(name, noticer) of every arm of a run: the four criterion arms and the fifth control (B3's
    ramp + split over the re-anchor) first, then the labelled sensitivity arms (nothing is chosen
    from them)."""
    return [
        ("learned", learned(1001)),
        ("learned_off", learned(1002, learning=False, carry=False)),
        ("frozen", medium_frozen()),
        ("reanchor", dict(REANCHOR)),
        (RAMP_SPLIT, ramp_split_over_re2()),
        # sensitivity, labelled:
        ("learned_nocarry", learned(1003, carry=False)),
        ("learned_p080", learned(1004, min_precision=0.8)),
        ("learned_w050", learned(1005, prior_window_ns=50 * MS)),
        ("learned_w1000", learned(1006, prior_window_ns=1000 * MS)),
    ]


def run_id(stage):
    b, rho = PRIMARY
    return f"l1-{stage}-{setting_id(b, rho)}"


def stage(name):
    """(arms, seeds, run_seed, experiment) of a stage: 'dev' on the tuning seeds, 'fresh'."""
    if name == "dev":
        return arms(), DEV_SEEDS, RUN_SEED, "exploration-l1-dev"
    if name == "fresh":
        return arms(), FRESH_SEEDS, RUN_SEED, "exploration-l1-fresh"
    raise SystemExit(f"unknown stage {name!r}")


# ---- the criterion, as the chief fixed it (docs/lab-queue.md, "## L1"; not changed here) -------------
#
# At 100 ms on the 200 fresh streams (40000-40199), the learned arm counts toward the aim if
#   (1) its anchor-correct share over the last 100 streams is at least the frozen graph's over the
#       same streams minus 0.01, with the paired lower bound above -0.03, AND
#   (2) its slope over the first 100 streams is positive with the lower bound above zero, AND
#   (3) the learning-off control's end state is below the frozen graph's by at least 0.02.
# Conjunctive; each is reported.
#
# Readings fixed before the fresh run:
#
# R1. "anchor-correct share": the hard non-leak incidents' anchor-correct share, the evaluator's N5
#     over the incidents outside the slow-leak family, pooled over the streams of the window
#     (a ratio of sums), as every M2 and B2 table.
# R2. "paired lower bound": the 5th percentile of the paired 90% cluster bootstrap (10,000 resamples
#     of whole streams, the same counts for both arms; B1's arithmetic). Intervals everywhere are
#     these 90% intervals; a "lower bound above zero" is their 5th percentile above zero.
# R3. "its slope over the first 100 streams" (clause 2), read as the brief's own list reads it
#     ("W1's sample-efficiency curve per arm ..., the slope over the first 50, 100 and 200
#     streams"): the least-squares slope of W1's curve (`sample_efficiency` of the anchor-correct
#     hard non-leak incidents over the hard non-leak incidents seen, cumulative in seed order)
#     against the number of streams, over the points n = 1..K that have an incident seen, in
#     efficiency per 100 streams. Cluster bootstrap: whole streams are resampled with replacement
#     in place (multiplicities kept in stream order, so the curve is the weighted cumulative ratio)
#     and the slope recomputed. K = 100 is clause 2; 50 and 200 are reported. The same is reported
#     for the leak-noticed curve.
# R3b. The alternative reading, reported beside and not the clause: the least-squares slope of the
#     anchor-correct outcome of the hard non-leak incidents (1 or 0) against the incident's position
#     in the sequence of such incidents seen in stream order, over the incidents of the first K
#     streams, in share per 100 incidents seen (`gordian_analysis.measures.outcome_slope`), whole
#     streams resampled with every incident keeping its position. A learner that reaches its end
#     state within a few streams has an early curve that rises and an incident-level slope that
#     is small, because almost all its incidents come after the rise; the two readings differ for
#     such a learner, which is why both are stated before the run.
# R4. "end state": the last 100 streams, 40100-40199. "Below by at least 0.02" is a point
#     difference (frozen minus control >= 0.02); its paired interval is reported.
# R5. The curves are W1's `sample_efficiency` on per-stream counts of anchor-correct hard non-leak
#     incidents over hard non-leak incidents seen, and of leak-noticed over leaks seen, cumulative
#     in seed order; a third curve pools both over all hard incidents seen.
# R6. The background and strict-precision bounds are those of M2: notices on background per stream
#     at most 6.82, strict precision against the re-anchor's 0.67; reported beside, not clauses.
END_STREAMS = 100
SLOPE_WINDOWS = (50, 100, 200)
CLAUSE_SLOPE_WINDOW = 100
CLAUSE1_MARGIN = -0.01
CLAUSE1_LOWER_ABOVE = -0.03
CLAUSE3_GAP = 0.02
BACKGROUND_BOUND = 6.82
