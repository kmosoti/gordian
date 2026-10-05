# R8 runtime: llama.cpp for a real small model on this CPU

R8 (`docs/local-test-plan.md`, 5R) asks one real model, run locally, how its accuracy on this world's
own diagnosis questions moves with the number of irrelevant references. This file records what
runs it. Nothing here is a Gordian dependency: the Rust workspace and `analysis/` are untouched by
it, and no Python package is installed for it.

## What is installed, and where

| What | Where | How it was made |
|---|---|---|
| llama.cpp, release tag `b11429` (commit `d81235049384534c167caea52b85a694f6103d14`) | `/home/user/gordian/artifacts/runtime/llama.cpp` (shallow clone, git-ignored) | `git clone --depth 1 --branch b11429 https://github.com/ggml-org/llama.cpp` |
| Build | `/home/user/gordian/artifacts/runtime/build` (git-ignored; 111 MB) | `cmake -S llama.cpp -B build -DCMAKE_BUILD_TYPE=Release -DGGML_NATIVE=ON -DLLAMA_OPENSSL=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF -DLLAMA_BUILD_SERVER=ON -DLLAMA_BUILD_TOOLS=ON`, then `cmake --build build -j 3 --target llama-server llama-bench llama-tokenize` on cores 0-2 |
| Qwen2.5-1.5B-Instruct, Q4_K_M | `/home/user/gordian/artifacts/models/qwen2.5-1.5b-instruct-q4_k_m.gguf` (1,117,320,736 bytes) | sha256 `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e`, equal to the `x-linked-etag` Hugging Face reports for `Qwen/Qwen2.5-1.5B-Instruct-GGUF` at commit `91cad51170dc346986eccefdc2dd33a9da36ead9` |
| Qwen2.5-3B-Instruct, Q4_K_M (the plan's fallback) | `/home/user/gordian/artifacts/models/qwen2.5-3b-instruct-q4_k_m.gguf` (2,104,932,768 bytes) | sha256 `626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d`, equal to the `x-linked-etag` for `Qwen/Qwen2.5-3B-Instruct-GGUF` at commit `7dabda4d13d513e3e842b20f0d435c732f172cbe` |

The build has `-march=native` (AVX-512 with VNNI on this CPU) and uses the machine's CPU backend only.
The models are the files at the paths above; nothing else downloads anything at run time.

## Dependency note (AGENTS.md: requirement, simpler alternative, cost)

- **Requirement.** Local CPU inference of a 1.5B to 3B instruction model with deterministic greedy
  decoding, a fixed seed, a hard cap on output tokens, token counts and timings per call, and reuse
  of the shared prompt prefix (the system prompt and worked examples are the same in every call).
- **The runtime used.** The `llama-server` binary from the pinned release tag, called over HTTP on
  the loopback interface by `r8_run.py`, which uses only the Python standard library.
- **Simpler alternatives considered.**
  - `llama-cpp-python` at a pinned version: the same engine behind a Python binding. It would compile
    llama.cpp anyway (through pip's build step), add a wheel and a Python layer to pin, and give no
    capability this item needs that the server does not. Rejected for that reason.
  - `llama-cli` per call: reloads the model every call and cannot keep the prompt cache. Rejected.
  - PyTorch or `transformers` on CPU: a multi-gigabyte dependency, slower on this machine for a
    4-bit model, no prefix reuse out of the box. Rejected.
  - A prebuilt release binary: possible, but built without this CPU's instruction set. Rejected;
    the source build is under a few minutes.
- **Cost.** Build time of a few minutes with three jobs; 214 MB of source and 111 MB of build in
  `artifacts/runtime/`; 1.1 GB for the first model and 2.1 GB for the second in `artifacts/models/`.
  No Python or Rust dependency is added to the repository.

## How a run is made

`r8_run.sh RUN_ID DESIGN.json QUESTIONS.jsonl MODEL.gguf [LABEL]`:

- refuses to start while `cargo`, `rustc` or another llama process runs, or with under 4 GB free;
- starts `llama-server` under `scripts/cgroup-run.sh --cpus 0-2 --cpu-quota 300 --memory 6G`, with
  `-t 3 -tb 3 -np 1 -cram 0 -c 16384 --seed 1`: three threads, one slot, one model, the host-memory
  prompt cache off, a 16,384-token context;
- runs the client (`r8_run.py`), pinned to core 3, which makes the calls with temperature 0,
  `top_k` 1, seed 1, `max_tokens` 80 and `cache_prompt` on, and appends one line per call to
  `artifacts/runs/RUN_ID/calls.jsonl`;
- stops the server when the client exits; `usage.json` is the runner's report of CPU time, peak
  memory and OOM kills.

**Prompt caching.** The slot keeps the key-value cache of the previous prompt and reuses the longest
common prefix of the next one (`cache_prompt`). Every call starts with the same system prompt and
worked examples, so after the first call of a run only the question is evaluated; each call's
`timings.cache_n` records how many prompt tokens were reused. Calls are never replayed to rebuild
state.

**Determinism.** Greedy decoding at a fixed seed is deterministic for a given prompt and cache state.
The cache state differs between a cold and a warm slot, and floating-point results can differ in the
last bits with the batch layout, so a replay of the same prompt in a different order is not promised
to be token-identical. Every call's prompt hash and full response are recorded, which is the
record the analysis uses.
