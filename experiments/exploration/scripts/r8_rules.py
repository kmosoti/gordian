"""R8: the stated rules, read by a program.

`read(services, focus, context)` applies the rules in the reasoner prompt (`r8_common.SYSTEM_PROMPT`)
to a context exactly as written there, and returns the answer. `evidence_line` writes the prompt's
evidence line. They have two uses:

* they write the evidence line of each worked example, and check the example's answer against the
  rules, so that an example never teaches something the prompt's own rules contradict;
* the reader is a **reference reader** in the report: what a deterministic reader of the same rules
  gets on the same contexts at each level. It is not a policy, not an arm, and not a baseline for
  any experiment; it shows whether the contexts are answerable in principle and how a program's
  accuracy moves with the number of distractors, beside the model's.

The window is +5 to +20 seconds after the first alarm for free-form messages (the hard kinds show
their extra evidence 6 to 16 seconds after it) and 0 to +20 seconds for the other signs; times are
the whole seconds the rendering shows. The reader never sees hidden state: it reads rendered
observations and the public graph.

Exploration (nothing here tests a hypothesis).
"""

import r8_common as C

FF_LO, FF_HI = 5, 20
SIGN_LO, SIGN_HI = 0, 20
KIND_MESSAGE = {
    "OutOfResource": "ResourceExhausted",
    "ConfigRejected": "ConfigDrift",
    "ServiceDown": "DependencyDown",
    "Unauthorized": "CredentialExpired",
    "Flapping": "Intermittent",
}
TAG_ORDER = ("site", "connected", "unconnected")


def seconds(rec, focus):
    return round((rec["at_ns"] - focus["at_ns"]) / 1e9)


def facts(services, focus, context):
    """What the evidence line states, from the rendered context: the tags of the free-form
    messages in their window, the tags of the MixedSignals messages, the kind messages at the site
    and the abnormal counters at the site."""
    site = C.service_of(focus["obs"])
    rel = C.relations(services, site)
    ff, mixed, kind_msgs = set(), set(), []
    counters_at_site = {}
    for r in context:
        t = seconds(r, focus)
        obs = r["obs"]
        if "Message" in obs:
            m = obs["Message"]
            tid, tag = m["text_id"], rel[m["service"]]
            if tid >= C.CATALOGUE_LIMIT:
                if FF_LO <= t <= FF_HI:
                    ff.add(tag)
            elif SIGN_LO <= t <= SIGN_HI:
                name = C.CATALOGUE[tid]
                if name == "MixedSignals":
                    mixed.add(tag)
                elif tag == "site" and name in KIND_MESSAGE and name not in kind_msgs:
                    kind_msgs.append(name)
        elif "Counter" in obs and SIGN_LO <= t <= SIGN_HI:
            c = obs["Counter"]
            if c["service"] == site and c["value"] >= C.HIGH:
                counters_at_site[c["name"]] = True
    order = lambda tags: [t for t in TAG_ORDER if t in tags]  # noqa: E731
    return site, order(ff), order(mixed), kind_msgs, counters_at_site


def tags_text(tags):
    return ", ".join(tags) if tags else "none"


def yes(flag):
    return "yes" if flag else "no"


def evidence_line(services, focus, context):
    _, ff, mixed, kind_msgs, _ = facts(services, focus, context)
    return (
        f"Evidence: site free-form: {yes('site' in ff)}. connected free-form: {yes('connected' in ff)}. "
        f"unconnected free-form: {yes('unconnected' in ff)}. "
        f"MixedSignals at the site and elsewhere: {yes('site' in mixed and len(mixed) >= 2)}. "
        f"Kind message at the site: {', '.join(kind_msgs) if kind_msgs else 'none'}."
    )


def read(services, focus, context):
    """`(kind, site)` by the rules of the prompt, in the order the prompt gives them. Where the
    rules leave a choice open (nothing names a known kind) the first listed kind is taken."""
    site, ff, mixed, kind_msgs, counters = facts(services, focus, context)
    if "site" in mixed and len(mixed) >= 2:
        return ("SplitBrain", site)
    if "site" in ff and "unconnected" in ff:
        return ("Cascade", site)
    if "site" in ff and "connected" in ff:
        return ("SplitBrain", site)
    if "site" in ff:
        return ("Compound", site)
    if kind_msgs:
        return (KIND_MESSAGE[kind_msgs[0]], site)
    if counters.get("AuthFailures"):
        return ("CredentialExpired", site)
    if counters.get("Restarts"):
        return ("DependencyDown", site)
    if counters.get("Saturation"):
        return ("ResourceExhausted", site)
    if "site" in mixed:
        return ("DependencyDown", site)
    return ("ResourceExhausted", site)
