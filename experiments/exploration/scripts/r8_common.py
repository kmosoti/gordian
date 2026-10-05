"""R8: what a question looks like to the model, and nothing else that needs a decision twice.

Exploration (nothing here tests a hypothesis). The plan is `docs/local-test-plan.md`, 5R, R8. This
module is shared by the question builder, the runner, the demo builder and the analysis. It holds:

* the rendering of an observation as one line of text (`render_obs`, `render_context`);
* the frozen reasoner prompt (`SYSTEM_PROMPT`) and the way a question's user message is built
  (`user_text`), with the worked examples in `r8-demos.json` placed before it as earlier turns;
* the answer parser (`parse_answer`): an answer that does not parse is wrong;
* the design: which contexts a question has at each level (`contexts`) and which incidents are
  questions (`select_questions`).

Standard library only, so the runner needs no environment beyond the interpreter.

Readings of the plan, fixed here before any scored call (they are listed in the report):

* The questions are streams of seeds 30000 upward with the regime schedule empty (the dumper's
  default); the pilot uses 29000 upward and the worked examples 28000 upward.
* The focus is the incident's first observation (its anchor). It is shown in the question, not
  counted in the context unless it is decisive (it is, for a plain incident). The incident's
  own non-decisive observations (its first moments, heartbeats, closure) are in neither the
  context nor the pool, as the plan's pool ("from background and other incidents") says.
* The distractors of the levels are nested: each question has one seeded permutation of its pool
  (keyed by sha256 of the seed, the incident and the observation id), and level m takes its first
  m. The plan says "drawn without replacement by a seeded generator", which a prefix of a
  permutation is; nesting makes the levels a paired design with the least added variance.
* The control (q = 0) is the level-50 distractors without the decisive evidence.
* A context is sorted by time, ties by stream position, as a window builder sorts it.
* Time is shown in whole seconds relative to the focus; services as `s<N>`; free-form
  message ids as a four-hex-digit alias of the id (a pure function of the id). A message line
  carries a tag, `(site)`, `(connected)` or `(unconnected)`, of its service relative to the focus's service,
  computed from the public graph (`relations`), the same for evidence and distractors.
"""

import hashlib
import json
import re

LEVELS = (0, 50, 100, 200, 400)
CONTROL_M = 50
MIN_POOL = max(LEVELS)

KNOWN = ("ResourceExhausted", "ConfigDrift", "DependencyDown", "CredentialExpired", "Intermittent")
HARD = ("Compound", "Cascade", "SplitBrain", "SlowLeak")
KINDS = KNOWN + HARD

CATALOGUE = {
    0x100: "OutOfResource",
    0x101: "ConfigRejected",
    0x102: "ServiceDown",
    0x103: "Unauthorized",
    0x104: "Flapping",
    0x105: "UpstreamUnreachable",
    0x106: "MixedSignals",
    0x107: "CheckHealth",
}
CATALOGUE_LIMIT = 1 << 16
COUNTER_NAME = {
    "ErrorRate": "errors",
    "Latency": "latency",
    "Saturation": "saturation",
    "AuthFailures": "authfail",
    "Restarts": "restarts",
}
HIGH = 50

MAX_TOKENS = 80
SERVER_SEED = 1


# ------------------------------------------------------------------ rendering


def alias(text_id):
    """A four-hex-digit alias of a free-form message id: a function of the id alone."""
    return format(((text_id * 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF) >> 48, "04x")


def stamp(at_ns, focus_ns):
    return f"{round((at_ns - focus_ns) / 1e9):+d}"


def service_of(obs):
    for key in ("Counter", "Message", "Snapshot"):
        if key in obs:
            return obs[key]["service"]
    raise ValueError(f"no service in {obs}")


def connectivity(services, site):
    """Dependents of `site` (directly or through others), its upstream, and the rest."""
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
    dependents = sorted(down - {site})
    upstream = sorted(up - {site})
    unconnected = sorted(set(range(n)) - up - down)
    return dependents, upstream, unconnected


def relations(services, site):
    """The tag of every service relative to the site, from the public graph alone: `site`, `connected`
    (it depends on the site or the site depends on it, directly or through others) or `unconnected`."""
    dependents, upstream, unconnected = connectivity(services, site)
    rel = {s: "connected" for s in dependents + upstream}
    rel.update({s: "unconnected" for s in unconnected})
    rel[site] = "site"
    return rel


def render_obs(rec, focus_ns, services, rel):
    """One line for one observation record (`{"id", "at_ns", "obs"}`). Messages carry the tag of
    their service relative to the focus's service (`rel` from `relations`); counters and
    snapshots do not."""
    t = stamp(rec["at_ns"], focus_ns)
    obs = rec["obs"]
    if "Counter" in obs:
        c = obs["Counter"]
        return f"{t} s{c['service']} {COUNTER_NAME[c['name']]}={c['value']}"
    if "Message" in obs:
        m = obs["Message"]
        tid = m["text_id"]
        where = f"s{m['service']}({rel[m['service']]})"
        if tid < CATALOGUE_LIMIT:
            name = CATALOGUE.get(tid)
            if name is None:
                raise ValueError(f"unknown catalogue id {tid}")
            return f"{t} {where} msg {name} {m['severity']}"
        return f"{t} {where} msg #{alias(tid)} {m['severity']}"
    if "Snapshot" in obs:
        s = obs["Snapshot"]
        same = services[s["service"]]["config_hash"] == s["config_hash"]
        return f"{t} s{s['service']} snapshot {'unchanged' if same else 'CHANGED'}"
    raise ValueError(f"observation kind not rendered: {list(obs)}")


def render_context(services, focus, context):
    """The lines of a context, in order."""
    rel = relations(services, service_of(focus["obs"]))
    return [render_obs(r, focus["at_ns"], services, rel) for r in context]


def user_text(services, focus, context):
    """The user message for one question. `focus` and the entries of `context` are observation
    records; `context` is already in context order. Nothing else about the incident goes in."""
    site = service_of(focus["obs"])
    lines = render_context(services, focus, context)
    rel = relations(services, site)
    parts = [
        f"First alarm of the incident (its site is s{site}):",
        render_obs(focus, focus["at_ns"], services, rel),
        "",
        "Observations (time in seconds relative to the first alarm):",
    ]
    parts.extend(lines if lines else ["(none)"])
    parts.append("")
    parts.append("What is wrong? Reply with the Evidence line and the ANSWER line.")
    return "\n".join(parts)


# ------------------------------------------------------------------ the prompt

SYSTEM_PROMPT = """You diagnose incidents in a service graph from a log.

Each log line is "time service reading". Time is in whole seconds relative to the incident's first alarm (+0). Counters are errors, latency, saturation, authfail, restarts: 50 or more is abnormal, lower is normal. "msg NAME severity" is a catalogue message; "msg #xxxx severity" is a free-form message, not in the catalogue. A message's service carries a tag: (site) is the incident's own service, (connected) is a service connected to it (it depends on the site or the site depends on it, directly or through others), (unconnected) is a service not connected to it. "snapshot unchanged" or "snapshot CHANGED" is a configuration snapshot. The first alarm of an incident is at its site, so the site of your answer is the service of the first alarm.

Known kinds. A kind at its site makes these abnormal (dependents are services that depend on the site, directly or through others):
- ResourceExhausted: site errors, latency, saturation; message OutOfResource; dependents errors, latency.
- ConfigDrift: site errors; message ConfigRejected; snapshot CHANGED at the site; dependents errors.
- DependencyDown: site errors, latency, restarts; messages ServiceDown, MixedSignals; dependents errors, latency, message UpstreamUnreachable.
- CredentialExpired: site errors, authfail; message Unauthorized; dependents errors.
- Intermittent: site errors, latency; messages Flapping, MixedSignals; dependents latency.
A message that only one kind emits at the site (OutOfResource, ConfigRejected, ServiceDown, Unauthorized, Flapping) names that kind. Normal readings, CheckHealth, unchanged snapshots and free-form messages at unrelated services carry no information.

Hard kinds. The rules above cannot explain them. They show extra evidence 6 to 16 seconds after the first alarm, including 3 to 5 free-form messages whose few ids repeat.
- Compound: two known kinds fail together at the site. The free-form messages are at the site only.
- Cascade: a known fault (ResourceExhausted or DependencyDown) at the site also alarms a service that is not connected to the site. The free-form messages alternate between the site and that (unconnected) service.
- SplitBrain: two services both act as primary. The site and a second service both show MixedSignals messages, and the free-form messages alternate between the two.
- SlowLeak: a series of saturation readings at the site that climbs slowly through normal values before it crosses 50. Choose it only if you see that series.
The site of a hard kind is the service of the first alarm.

How to answer. Reply with two lines.
Line 1: "Evidence: site free-form: <yes or no>. connected free-form: <yes or no>. unconnected free-form: <yes or no>. MixedSignals at the site and elsewhere: <yes or no>. Kind message at the site: <name or none>." A free-form message counts if it is between +5 and +20 seconds and its service has that tag; MixedSignals messages count between 0 and +20 seconds.
Line 2: "ANSWER: <Kind> s<N>", where <Kind> is one of ResourceExhausted, ConfigDrift, DependencyDown, CredentialExpired, Intermittent, Compound, Cascade, SplitBrain, SlowLeak and s<N> is the site; or "ANSWER: NONE" if the first alarm is not part of an incident (a short burst that stops, or an isolated abnormal reading).
Choose the kind from the evidence line, first match first:
1. MixedSignals at the site and elsewhere: yes -> SplitBrain.
2. site free-form: yes and unconnected free-form: yes -> Cascade.
3. site free-form: yes and connected free-form: yes -> SplitBrain.
4. site free-form: yes -> Compound, whatever the kind message says.
5. site free-form: no -> a known kind, from the kind message if there is one, or else from the counters."""


ANSWER_RE = re.compile(r"ANSWER:\s*([A-Za-z]+)(?:\s+s(\d+))?")


def parse_answer(text, n_services):
    """Parse a model's reply. Returns `("ok", kind, site)` with `kind` in `KINDS` and `site` an
    int below `n_services`, or `("ok", "NONE", None)`, or `("fail", reason, None)`.

    Only the first line that contains "ANSWER:" counts. A reply that has no such line, names a
    kind outside the nine, omits the site of a kind, or names a service that does not exist, does
    not parse; a reply that does not parse is wrong (the plan)."""
    for line in text.splitlines():
        if "ANSWER:" not in line:
            continue
        m = ANSWER_RE.search(line)
        if not m:
            return ("fail", "malformed answer line", None)
        word, site = m.group(1), m.group(2)
        if word.upper() == "NONE":
            return ("ok", "NONE", None)
        kind = next((k for k in KINDS if k.lower() == word.lower()), None)
        if kind is None:
            return ("fail", f"unknown kind {word}", None)
        if site is None:
            return ("fail", "no site", None)
        if int(site) >= n_services:
            return ("fail", "site out of range", None)
        return ("ok", kind, int(site))
    return ("fail", "no ANSWER line", None)


def truth_pair(truth):
    """`(kind, site)` of a question record's truth (`None` for no incident)."""
    if truth is None:
        return None
    kind = truth["kind"]
    name = kind["Known"] if "Known" in kind else kind["Hard"]
    return (name, truth["site"])


def is_correct(parsed, truth):
    status, kind, site = parsed
    if status != "ok":
        return False
    t = truth_pair(truth)
    if t is None:
        return kind == "NONE"
    return kind != "NONE" and (kind, site) == t


# ------------------------------------------------------------------ the design


def _key(seed, incident, obs_id):
    return hashlib.sha256(f"r8|{seed}|{incident}|{obs_id}".encode()).hexdigest()


def permutation(q):
    """The question's pool in the seeded order. Level m takes the first m."""
    return sorted(q["pool"], key=lambda r: _key(q["seed"], q["incident"], r["id"]))


def order_context(records):
    return sorted(records, key=lambda r: (r["at_ns"], r["id"]))


def contexts(q):
    """Every context of a question: `{m: records}` for the levels, and `"control"`."""
    perm = permutation(q)
    out = {}
    for m in LEVELS:
        out[m] = order_context(list(q["decisive"]) + perm[:m])
    out["control"] = order_context(perm[:CONTROL_M])
    return out


def qkey(q):
    return f"{q['seed']}:{q['incident']}"


def select_questions(records, n_hard, n_plain):
    """The design's questions from the dumper's records, in order, and the exclusions.

    Hard: every hard (non-leak) incident in stream order, those whose pool is at least
    `MIN_POOL` first-come up to `n_hard`; the ones with a smaller pool are excluded and counted
    (only those met before `n_hard` is reached). Plain: from each stream, in order, the one
    plain incident at position `seed mod k` among its `k` plain incidents whose pool is large
    enough, up to `n_plain`; plain incidents with a small pool are excluded and counted among the
    streams visited. Returns `(hard, plain, exclusions)`."""
    hard, plain = [], []
    ex = {"hard_small_pool": 0, "plain_small_pool": 0}
    by_seed = {}
    for r in records:
        by_seed.setdefault(r["seed"], []).append(r)
    for seed in sorted(by_seed):
        rs = by_seed[seed]
        for r in rs:
            if r["tier"] == "Hard" and len(hard) < n_hard:
                if r["pool_size"] >= MIN_POOL:
                    hard.append(r)
                else:
                    ex["hard_small_pool"] += 1
        if len(plain) < n_plain:
            ps = [r for r in rs if r["tier"] == "Plain"]
            big = [r for r in ps if r["pool_size"] >= MIN_POOL]
            ex["plain_small_pool"] += len(ps) - len(big)
            if big:
                plain.append(big[seed % len(big)])
    return hard, plain, ex


def load_jsonl(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def demos(path):
    """The worked examples as chat turns: a list of `(user text, assistant text)`."""
    with open(path) as f:
        return [(d["user"], d["answer"]) for d in json.load(f)]


def messages(system, demo_turns, user):
    msgs = [{"role": "system", "content": system}]
    for u, a in demo_turns:
        msgs.append({"role": "user", "content": u})
        msgs.append({"role": "assistant", "content": a})
    msgs.append({"role": "user", "content": user})
    return msgs


def prompt_hash(msgs):
    return hashlib.sha256(json.dumps(msgs, sort_keys=True, ensure_ascii=True).encode()).hexdigest()


def first_decisive_position(context, decisive):
    """Index in the context of its first decisive record, or None (the control)."""
    ids = {r["id"] for r in decisive}
    for i, r in enumerate(context):
        if r["id"] in ids:
            return i
    return None
