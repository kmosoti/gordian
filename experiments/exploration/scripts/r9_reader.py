"""R9: the reader. A deterministic program that answers a diagnosis question from its context alone.

FROZEN. This file and `r9_reader_weights.json` are the reader of R9's evaluation (seeds 30000 and
above). The commit that carries this note is the freeze (its hash is in the report and in the git tag
`r9-reader-frozen`; `r9-reader-freeze.json` holds the sha256 of both files). A change to either after
the freeze starts a new evaluation on seeds 32000 upward, and both are reported.

Exploration (nothing here tests a hypothesis). Evaluator-side, like `r8_rules.py`. It is not an arm,
not a baseline of any experiment, and never enters a policy. `read(services, focus, context)` returns
`(kind, site, trace)` and sees exactly what a reasoner's context holds: the public graph (`services`),
the first alarm (`focus`, an observation record) and the context's observation records
(`{"id", "at_ns", "obs"}`), at full time resolution. It reads no label, no tier, no per-stream
vocabulary assignment and no stream state outside the context.

What it may use (`docs/local-test-plan.md`, 5R, R9): the first world's public physics, the hard
families' rules (`HIDDEN-DESIGN.md`, sections 4 and 4.2), their timing structure, counters, and the
repetition of ids within the context. The rules it applies, as written there:

* A known kind at its site makes these abnormal in its first 300 ms (the burst; a counter is abnormal
  at 50 or more): see `KINDS` below. Dependents are services that depend on the site.
* A hard incident adds evidence 6 to 16 s after onset (phase 2: 3 to 5 free-form messages, which are
  catalogue-free messages whose ids come from a small vocabulary and so repeat) and in its burst
  (Contradict) or in phase 2 (Mimic) breaks the first world's rules:
  - Compound: two known kinds fail together at the site. The free-form messages are at the site only.
    Mimic: the second kind's characteristic message and counters at the site in phase 2.
  - Cascade: a known fault at the site also alarms a service that the public graph does not connect to
    it (`ErrorRate`, `Latency`, and `ErrorRate` at up to three of its dependents), 20 to 230 ms after
    the first alarm (Contradict) or in phase 2 (Mimic). The free-form messages alternate between the
    site and that partner.
  - SplitBrain: the site and a peer both show `MixedSignals` (with `ErrorRate`, `Latency` at the peer),
    in the burst (Contradict) or phase 2 (Mimic); the free-form messages alternate between the two.
* Heartbeats: one reading every 0.8 to 1.5 s at the site of a live incident, abnormal with
  probability 3/4.
* Background: free-form messages, benign counters, catalogue strays, abnormal counter blips,
  mini-bursts (an `ErrorRate` anchor and a characteristic message within 30 ms that then stops) and
  snapshots, at rates the reader estimates from its own context where it needs them.

How it decides: it computes a fixed list of features (`FEATURES`) from the rules above, each a count
or a tail probability against the background rate read from the context itself, and combines them
with a softmax over four classes (not hard, Compound, Cascade, SplitBrain) whose weights were fitted
on development questions only (`r9_train.py`, seeds 29000 to 29999) and are stored in
`r9_reader_weights.json`. If it says "not hard" it names the known kind by the first world's public
rules from the burst. Its trace records what each answer rested on.
"""

import json
import math
import os

HIGH = 50
CATALOGUE_LIMIT = 1 << 16
CAT = {
    0x100: "OutOfResource",
    0x101: "ConfigRejected",
    0x102: "ServiceDown",
    0x103: "Unauthorized",
    0x104: "Flapping",
    0x105: "UpstreamUnreachable",
    0x106: "MixedSignals",
    0x107: "CheckHealth",
}
CHARACTERISTIC = {
    "OutOfResource": "ResourceExhausted",
    "ConfigRejected": "ConfigDrift",
    "ServiceDown": "DependencyDown",
    "Unauthorized": "CredentialExpired",
    "Flapping": "Intermittent",
}

# The first world's public physics, as the reasoner prompt of R8 states it: for each known kind, the
# counters it makes abnormal at its site and its characteristic message.
KINDS = {
    "ResourceExhausted": {"site": {"ErrorRate", "Latency", "Saturation"}, "msg": "OutOfResource"},
    "ConfigDrift": {"site": {"ErrorRate"}, "msg": "ConfigRejected"},
    "DependencyDown": {"site": {"ErrorRate", "Latency", "Restarts"}, "msg": "ServiceDown"},
    "CredentialExpired": {"site": {"ErrorRate", "AuthFailures"}, "msg": "Unauthorized"},
    "Intermittent": {"site": {"ErrorRate", "Latency"}, "msg": "Flapping"},
}
HARD_CLASSES = ("Compound", "Cascade", "SplitBrain")
CLASSES = ("NotHard",) + HARD_CLASSES

# ----------------------------------------------------------------------------- windows

B_HI = 0.30  # the burst: the first 300 ms after the first alarm
P_LO, P_HI = 5.95, 16.05  # phase 2: [6 s, 16 s) after onset; the first alarm is at onset
OUT_LO, OUT_HI = 5.5, 16.5  # the background's density is read outside this window
MIN_COVERAGE = 30.0  # seconds: the least time a context is taken to cover
ALARM_PAIR_S = 0.05  # an ErrorRate and a Latency alarm within 50 ms are one alarm cluster
CHAR_NEAR_S = 0.10

WEIGHTS_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "r9_reader_weights.json")


def service_of(obs):
    for key in ("Counter", "Message", "Snapshot"):
        if key in obs:
            return obs[key]["service"]
    return None


def relations(services, site):
    """Public graph: `{service: "site" | "dependent" | "upstream" | "unconnected"}`. A dependent
    depends on the site directly or through others; an upstream service is one the site depends on
    (directly or through others)."""
    n = len(services)
    up = {site}
    stack = [site]
    while stack:
        s = stack.pop()
        for d in services[s]["depends_on"]:
            if d not in up:
                up.add(d)
                stack.append(d)
    down = {site}
    changed = True
    while changed:
        changed = False
        for s in range(n):
            if s not in down and any(d in down for d in services[s]["depends_on"]):
                down.add(s)
                changed = True
    rel = {}
    for s in range(n):
        if s == site:
            rel[s] = "site"
        elif s in down:
            rel[s] = "dependent"
        elif s in up:
            rel[s] = "upstream"
        else:
            rel[s] = "unconnected"
    return rel


def parse(context, focus_ns):
    """Context records as plain tuples with seconds relative to the first alarm:
    `(t, kind, service, a, b, id)`: counters `("C", service, name, value)`, messages
    `("M", service, text_id, severity)` and snapshots `("S", service, config_hash, None)`."""
    out = []
    for r in context:
        t = (r["at_ns"] - focus_ns) / 1e9
        o = r["obs"]
        if "Counter" in o:
            c = o["Counter"]
            out.append((t, "C", c["service"], c["name"], c["value"], r["id"]))
        elif "Message" in o:
            m = o["Message"]
            out.append((t, "M", m["service"], m["text_id"], m["severity"], r["id"]))
        elif "Snapshot" in o:
            s = o["Snapshot"]
            out.append((t, "S", s["service"], s["config_hash"], None, r["id"]))
    return out


def poisson_tail_score(n, mu):
    """-ln P(N >= n) for N ~ Poisson(mu), capped at 20; 0 for n <= 0."""
    if n <= 0:
        return 0.0
    mu = max(mu, 1e-9)
    term = math.exp(-mu)
    cdf = term
    for k in range(1, n):
        term *= mu / k
        cdf += term
    tail = max(1.0 - cdf, 1e-12)
    return min(-math.log(tail), 20.0)


def pairs_same(ids):
    """Number of unordered pairs of equal ids in a list."""
    c = {}
    for i in ids:
        c[i] = c.get(i, 0) + 1
    return sum(v * (v - 1) // 2 for v in c.values())


def matches(ids_a, ids_b):
    """Number of (a, b) pairs of equal ids."""
    c = {}
    for i in ids_b:
        c[i] = c.get(i, 0) + 1
    return sum(c.get(i, 0) for i in ids_a)


SITE_FEATURES = [
    "site_ff_score",         # tail score of the free-form messages at the site in phase 2
    "site_ff_n",             # their count (capped at 6)
    "site_ff_rep",           # repeated-id pairs among them (capped at 4)
    "site_char_p2",          # characteristic messages at the site in phase 2 (capped at 3)
    "site_char_p2_score",    # their tail score against the catalogue stray rate
    "site_nonerr_p2",        # abnormal counters other than ErrorRate at the site in phase 2 (cap 4)
    "site_nonerr_p2_score",  # their tail score against the abnormal-counter rate
    "burst_union",           # distinct characteristic messages at the site in the burst (capped at 3)
    "burst_inconsistent",    # the site's burst fits no single known kind (0/1)
    "burst_char",            # a characteristic message at the site in the burst (0/1)
    "site_mixed_burst",      # MixedSignals at the site in the burst (0/1)
]
PARTNER_FEATURES = [
    "ff_n",        # free-form messages of the partner candidate in phase 2 (capped at 5)
    "ff_score",    # their tail score against the free-form rate
    "share",       # id pairs (site message, candidate message) of equal id in phase 2 (capped at 4)
    "alarm_burst",  # distinct abnormal counters at the candidate in the burst (capped at 3)
    "cluster_p2",  # an ErrorRate and a Latency alarm within 50 ms in phase 2, no characteristic message near
    "mixed_burst",  # MixedSignals at the candidate in the burst
    "mixed_p2",    # MixedSignals at the candidate in phase 2
    "mixed_alarm",  # MixedSignals at the candidate within 100 ms of an abnormal counter there (burst or phase 2)
    "pulses",      # abnormal readings at the candidate in the 6 s after its first alarm (capped at 4)
]
# The partner candidates: the best unconnected service and the best connected one (a dependent or an
# upstream service of the site), each chosen by the score `partner_score`.
GROUPS = ("unc", "dep", "up")
GROUP_REL = {"unc": "unconnected", "dep": "dependent", "up": "upstream"}
FEATURES = SITE_FEATURES + [f"{g}_{k}" for g in GROUPS for k in PARTNER_FEATURES]


def _groups(recs, site, rel):
    ff_p2 = {}
    mixed_burst, mixed_p2 = set(), set()
    mixed_times = {}
    site_mixed_burst = False
    char_site_p2, char_site_burst = [], []
    nonerr_site_p2 = 0
    ab_burst = {}     # service -> set of abnormal counter names in the burst
    ab_series = {}    # service -> [(t, name)] abnormal counters over the whole context
    p2_alarm = {}     # service -> {"E": [t], "L": [t]} in phase 2
    char_times = {}
    ab_out = ff_out = cat_out = 0
    site_burst_ab = set()
    for (t, k, s, a, b, i) in recs:
        in_b = 0 < t <= B_HI
        in_p = P_LO <= t <= P_HI
        outside = not (OUT_LO <= t <= OUT_HI) and not (-0.1 <= t <= 0.5)
        if k == "M":
            if a >= CATALOGUE_LIMIT:
                if in_p:
                    ff_p2.setdefault(s, []).append((a, i))
                if not (OUT_LO <= t <= OUT_HI):
                    ff_out += 1
            else:
                name = CAT.get(a)
                if outside:
                    cat_out += 1
                if name in CHARACTERISTIC:
                    char_times.setdefault(s, []).append(t)
                    if s == site and in_p:
                        char_site_p2.append(name)
                    if s == site and in_b:
                        char_site_burst.append(name)
                if name == "MixedSignals":
                    if in_b or in_p:
                        mixed_times.setdefault(s, []).append(t)
                    if s == site and in_b:
                        site_mixed_burst = True
                if name == "MixedSignals" and s != site:
                    if in_b:
                        mixed_burst.add(s)
                    if in_p:
                        mixed_p2.add(s)
        elif k == "C":
            abn = b >= HIGH
            if abn and outside:
                ab_out += 1
            if abn:
                ab_series.setdefault(s, []).append((t, a))
            if abn and in_p and s == site and a != "ErrorRate":
                nonerr_site_p2 += 1
            if abn and in_b and s == site:
                site_burst_ab.add(a)
            if abn and in_b and s != site:
                ab_burst.setdefault(s, set()).add(a)
            if abn and in_p and s != site:
                d = p2_alarm.setdefault(s, {"E": [], "L": []})
                if a == "ErrorRate":
                    d["E"].append(t)
                elif a == "Latency":
                    d["L"].append(t)
    return {
        "ff_p2": ff_p2, "ff_out": ff_out, "cat_out": cat_out, "ab_out": ab_out,
        "mixed_burst": mixed_burst, "mixed_p2": mixed_p2, "char_site_p2": char_site_p2,
        "char_site_burst": char_site_burst, "nonerr_site_p2": nonerr_site_p2,
        "ab_burst": ab_burst, "ab_series": ab_series, "p2_alarm": p2_alarm, "char_times": char_times,
        "site_burst_ab": site_burst_ab, "mixed_times": mixed_times, "site_mixed_burst": site_mixed_burst,
    }


def _burst_inconsistent(g):
    """The site's burst fits no single known kind: two characteristic messages, or abnormal counters
    that no known kind makes abnormal together (with its message, if there is one)."""
    chars = set(g["char_site_burst"])
    if len(chars) >= 2:
        return 1
    ab = g["site_burst_ab"]
    if not ab:
        return 0
    for kind, spec in KINDS.items():
        if ab <= spec["site"] and (not chars or chars == {spec["msg"]}):
            return 0
    return 1


def _partner_block(s, g, site_ids, mu_ff):
    """The partner features of candidate service `s`, and the observation ids they cite."""
    msgs = g["ff_p2"].get(s, [])
    ids = [a for a, _ in msgs]
    d = g["p2_alarm"].get(s)
    cluster = 0
    first_alarm = None
    times = [t for t, _ in g["ab_series"].get(s, []) if (0 < t <= B_HI) or (P_LO <= t <= P_HI)]
    if d and any(abs(x - y) < ALARM_PAIR_S for x in d["E"] for y in d["L"]):
        near = any(abs(tc - x) < CHAR_NEAR_S for tc in g["char_times"].get(s, []) for x in d["E"] + d["L"])
        if not near:
            cluster = 1
    if times:
        first_alarm = min(times)
    pulses = 0
    if first_alarm is not None:
        pulses = sum(1 for t, _ in g["ab_series"].get(s, []) if first_alarm + 0.5 < t <= first_alarm + 6.0)
    return {
        "ff_n": float(min(len(ids), 5)),
        "ff_score": poisson_tail_score(len(ids), mu_ff),
        "share": float(min(matches(site_ids, ids), 4)),
        "alarm_burst": float(min(len(g["ab_burst"].get(s, ())), 3)),
        "cluster_p2": float(cluster),
        "mixed_burst": float(s in g["mixed_burst"]),
        "mixed_p2": float(s in g["mixed_p2"]),
        "mixed_alarm": float(any(abs(tm - ta) < 0.1 for tm in g["mixed_times"].get(s, [])
                                 for ta, _ in g["ab_series"].get(s, []))),
        "pulses": float(min(pulses, 4)),
    }, [i for _, i in msgs]


def partner_score(b):
    """How much a candidate looks like the cascade's partner or the split brain's peer: the sum of its
    evidence, each item on a comparable scale. Used only to choose which service of a group the
    classifier looks at."""
    return (min(b["ff_score"], 8.0) / 4.0 + b["share"] + b["alarm_burst"] / 2.0 + b["cluster_p2"]
            + b["mixed_burst"] + b["mixed_p2"] + b["mixed_alarm"] - 0.25 * b["pulses"])


def features(services, focus, context):
    """The reader's features of one question and a trace of what they were computed from."""
    site = service_of(focus["obs"])
    rel = relations(services, site)
    n = len(services)
    recs = parse(context, focus["at_ns"])
    g = _groups(recs, site, rel)
    win = P_HI - P_LO
    # The time the context covers, read from the context itself (its earliest and latest observation,
    # taken to include phase 2 and at least `MIN_COVERAGE` seconds), less the window where the
    # evidence sits: the background's rate is read from what lies outside that window.
    ts = [r[0] for r in recs] or [0.0]
    coverage = max(max(ts), OUT_HI) - min(min(ts), OUT_LO)
    span_out = max(coverage, MIN_COVERAGE) - (OUT_HI - OUT_LO)
    mu_ff = (g["ff_out"] + 0.5) / span_out * win / n  # free-form messages at one service in phase 2
    mu_cat_type = (g["cat_out"] + 0.5) / span_out / 8.0 / n  # one catalogue type at one service, per second
    mu_ab = (g["ab_out"] + 0.5) / span_out / n  # abnormal counters at one service, per second
    f = dict.fromkeys(FEATURES, 0.0)
    site_msgs = g["ff_p2"].get(site, [])
    site_ids = [a for a, _ in site_msgs]
    f["site_ff_n"] = float(min(len(site_ids), 6))
    f["site_ff_score"] = poisson_tail_score(len(site_ids), mu_ff)
    f["site_ff_rep"] = float(min(pairs_same(site_ids), 4))
    f["site_char_p2"] = float(min(len(g["char_site_p2"]), 3))
    f["site_char_p2_score"] = poisson_tail_score(len(g["char_site_p2"]), mu_cat_type * 5 * win)
    f["site_nonerr_p2"] = float(min(g["nonerr_site_p2"], 4))
    f["site_nonerr_p2_score"] = poisson_tail_score(g["nonerr_site_p2"], mu_ab * win)
    f["burst_union"] = float(min(len(set(g["char_site_burst"])), 3))
    f["burst_char"] = float(bool(g["char_site_burst"]))
    f["site_mixed_burst"] = float(g["site_mixed_burst"])
    f["burst_inconsistent"] = float(_burst_inconsistent(g))
    cited = [i for _, i in site_msgs]
    chosen = {}
    for grp in GROUPS:
        best, best_b, best_cite = None, None, []
        for s in range(n):
            if s == site or rel[s] != GROUP_REL[grp]:
                continue
            b, cite = _partner_block(s, g, site_ids, mu_ff)
            sc = partner_score(b)
            if best is None or sc > best[0]:
                best, best_b, best_cite = (sc, s), b, cite
        if best is not None:
            for k, v in best_b.items():
                f[f"{grp}_{k}"] = v
            chosen[grp] = best[1]
            cited += best_cite
    trace = {"site": site, "cited_ff": sorted(cited), "partner": chosen, "mu_ff": mu_ff}
    return f, trace


# ----------------------------------------------------------------------------- the decision

_W = None


def weights():
    global _W
    if _W is None:
        if os.path.exists(WEIGHTS_PATH):
            with open(WEIGHTS_PATH) as fh:
                _W = json.load(fh)
        else:
            _W = {}
    return _W


def class_probs(f):
    w = weights()
    if not w:
        raise RuntimeError("no weights: run r9_train.py")
    z = []
    for c in CLASSES:
        wc = w["classes"][c]
        z.append(wc["bias"] + sum(wc["w"][k] * (f[k] - w["mean"][k]) / w["scale"][k] for k in FEATURES))
    m = max(z)
    e = [math.exp(x - m) for x in z]
    s = sum(e)
    return {c: x / s for c, x in zip(CLASSES, e)}


def plain_kind(services, focus, context):
    """The known kind of a plain-looking incident by the first world's public rules, from the burst
    at the site. Where the burst leaves two kinds open, the first listed."""
    site = service_of(focus["obs"])
    recs = parse(context, focus["at_ns"])
    chars, ab, changed = [], set(), False
    for (t, k, s, a, b, i) in recs:
        if s != site or not (-0.01 <= t <= B_HI):
            continue
        if k == "M" and a < CATALOGUE_LIMIT and CAT.get(a) in CHARACTERISTIC:
            chars.append(CHARACTERISTIC[CAT[a]])
        elif k == "C" and b >= HIGH:
            ab.add(a)
        elif k == "S" and services[s]["config_hash"] != a:
            changed = True
    if chars:
        return max(set(chars), key=lambda x: (chars.count(x), x == chars[0]))
    if changed:
        return "ConfigDrift"
    if "AuthFailures" in ab:
        return "CredentialExpired"
    if "Restarts" in ab:
        return "DependencyDown"
    if "Saturation" in ab:
        return "ResourceExhausted"
    return "ResourceExhausted"


def read(services, focus, context):
    """`(kind, site, trace)`."""
    f, trace = features(services, focus, context)
    p = class_probs(f)
    best = max(CLASSES, key=lambda c: p[c])
    site = trace["site"]
    trace = dict(trace, features=f, probs=p, decided=best)
    if best == "NotHard":
        return plain_kind(services, focus, context), site, trace
    return best, site, trace
