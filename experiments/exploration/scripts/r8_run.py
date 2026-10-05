"""R8: make a phase's calls against a running llama-server and record every one.

Usage: r8_run.py --design DESIGN.json --questions Q.jsonl --demos r8-demos.json --out RUN_DIR
                 [--url http://127.0.0.1:8089] [--model-label NAME] [--wait 300]

Normally started by `r8_run.sh`, which starts the server under `scripts/cgroup-run.sh`, pinned to
cores 0-2, and runs this client on core 3. The client uses only the standard library.

For every call of the design, in order, it builds the messages (the frozen system prompt, the
worked examples as earlier turns, the question), posts them to `/v1/chat/completions` with greedy
decoding (temperature 0, top_k 1, a fixed seed, a fixed maximum of output tokens) and prompt caching
on, and appends one line to `RUN_DIR/calls.jsonl`: the prompt hash, the full response, the parsed
answer, latency, token counts, the server's timings, and what the analysis needs about the question
(truth, family, level, the position of the first decisive reference). A call is never repeated: a
transport failure, a timeout or an error status is recorded as a call whose answer did not parse. If
the file already holds a call id, that call is skipped (a restart after an interruption continues
where it stopped; nothing is replayed).
"""

import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request

import r8_common as C

CALL_TIMEOUT_S = 900


def post(url, payload, timeout):
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def get(url, timeout=10):
    with urllib.request.urlopen(url, timeout=timeout) as r:
        return json.loads(r.read())


def wait_for(url, seconds):
    t0 = time.time()
    last = None
    while time.time() - t0 < seconds:
        try:
            if get(url + "/health").get("status") == "ok":
                return
        except Exception as e:  # the server is still loading the model
            last = e
        time.sleep(2)
    raise SystemExit(f"server not ready after {seconds} s: {last}")


def context_tokens(url, text):
    try:
        r = post(url + "/tokenize", {"content": text, "add_special": False}, 60)
        return len(r["tokens"])
    except Exception:
        return None


def done_ids(path):
    ids = set()
    if os.path.exists(path):
        with open(path) as f:
            for line in f:
                if line.strip():
                    ids.add(json.loads(line)["call_id"])
    return ids


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--design", required=True)
    ap.add_argument("--questions", required=True)
    ap.add_argument("--demos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--url", default="http://127.0.0.1:8089")
    ap.add_argument("--model-label", default="")
    ap.add_argument("--wait", type=int, default=300)
    args = ap.parse_args()

    design = json.load(open(args.design))
    qs = {C.qkey(q): q for q in C.load_jsonl(args.questions)}
    demo_turns = C.demos(args.demos)
    os.makedirs(args.out, exist_ok=True)
    calls_path = os.path.join(args.out, "calls.jsonl")
    meta = {q["qkey"]: q for q in design["questions"]}

    wait_for(args.url, args.wait)
    props = get(args.url + "/props")
    manifest = {
        "phase": design["phase"],
        "model_label": args.model_label,
        "system_prompt_sha256": hashlib.sha256(C.SYSTEM_PROMPT.encode()).hexdigest(),
        "demos_sha256": hashlib.sha256(open(args.demos, "rb").read()).hexdigest(),
        "design_sha256": hashlib.sha256(open(args.design, "rb").read()).hexdigest(),
        "decoding": {
            "temperature": 0,
            "top_k": 1,
            "seed": C.SERVER_SEED,
            "max_tokens": C.MAX_TOKENS,
            "cache_prompt": True,
        },
        "server_props": {k: props.get(k) for k in ("build_info", "model_path", "total_slots")},
        "n_ctx": (props.get("default_generation_settings") or {}).get("n_ctx"),
        "calls_in_design": len(design["calls"]),
    }
    with open(os.path.join(args.out, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=1)

    already = done_ids(calls_path)
    t_run = time.time()
    n_new = 0
    with open(calls_path, "a") as out:
        for c in design["calls"]:
            if c["call_id"] in already:
                continue
            q = qs[c["qkey"]]
            m = meta[c["qkey"]]
            ctxs = C.contexts(q)
            ctx = ctxs[c["level"]]
            user = C.user_text(q["services"], q["focus"], ctx)
            msgs = C.messages(C.SYSTEM_PROMPT, demo_turns, user)
            ph = C.prompt_hash(msgs)
            n_services = len(q["services"])
            ctx_text = "\n".join(C.render_context(q["services"], q["focus"], ctx))
            payload = {
                "messages": msgs,
                "temperature": 0,
                "top_k": 1,
                "seed": C.SERVER_SEED,
                "max_tokens": C.MAX_TOKENS,
                "cache_prompt": True,
                "stream": False,
            }
            t0 = time.time()
            err = None
            resp = None
            try:
                resp = post(args.url + "/v1/chat/completions", payload, CALL_TIMEOUT_S)
            except Exception as e:  # recorded as a failed call, never retried
                err = f"{type(e).__name__}: {e}"
            latency = time.time() - t0
            text = ""
            finish = None
            usage = timings = None
            if resp is not None:
                choice = resp["choices"][0]
                text = choice["message"].get("content") or ""
                finish = choice.get("finish_reason")
                usage = resp.get("usage")
                timings = resp.get("timings")
            parsed = C.parse_answer(text, n_services) if err is None else ("fail", "transport", None)
            correct = C.is_correct(parsed, q["truth"])
            truth = C.truth_pair(q["truth"])
            decisive_in_ctx = sum(1 for r in ctx if r["id"] in {d["id"] for d in q["decisive"]})
            rec = {
                "call_id": c["call_id"],
                "phase": design["phase"],
                "qkey": c["qkey"],
                "seed": q["seed"],
                "incident": q["incident"],
                "tier": q["tier"],
                "family": q["family"],
                "mode": q["mode"],
                "duo": q["duo"],
                "recurrence_of": q["recurrence_of"],
                "truth_kind": truth[0] if truth else None,
                "truth_site": truth[1] if truth else None,
                "n_services": n_services,
                "level": c["level"],
                "control": c["level"] == "control",
                "m": C.CONTROL_M if c["level"] == "control" else c["level"],
                "context_lines": len(ctx),
                "decisive_total": len(q["decisive"]),
                "decisive_in_context": decisive_in_ctx,
                "first_decisive_position": C.first_decisive_position(ctx, q["decisive"]),
                "context_tokens": context_tokens(args.url, ctx_text) if ctx_text else 0,
                "prompt_sha256": ph,
                "response": text,
                "finish_reason": finish,
                "parse_status": parsed[0],
                "parsed_kind": parsed[1] if parsed[0] == "ok" else None,
                "parsed_site": parsed[2],
                "parse_failure_reason": parsed[1] if parsed[0] != "ok" else None,
                "correct": bool(correct),
                "error": err,
                "latency_s": round(latency, 3),
                "usage": usage,
                "timings": timings,
                "pool_size": m["pool_size"],
            }
            out.write(json.dumps(rec) + "\n")
            out.flush()
            os.fsync(out.fileno())
            n_new += 1
            print(
                f"[{c['call_id'] + 1}/{len(design['calls'])}] {c['qkey']} {m['tier']} "
                f"{m['family']} level={c['level']} -> {parsed[1]} {parsed[2]} "
                f"correct={correct} {latency:.1f}s",
                flush=True,
            )
    print(f"done: {n_new} new calls in {time.time() - t_run:.0f} s", flush=True)


if __name__ == "__main__":
    sys.exit(main())
