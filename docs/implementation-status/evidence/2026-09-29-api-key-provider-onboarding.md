# Eight API-key Providers: bounded gateway and pi acceptance

- Date: **2026-09-29 UTC** (local capture directories may use September 30).
- Implementation: `semantic-v2`, base `372105e` plus the accompanying onboarding changes; not the unchanged base commit.
- Consumers: pinned OpenAI Python SDK `3.19.0`; installed pi `0.87.1` SDK agent using an independent model configuration, in-memory sessions and a fixed synthetic read tool. This is not an interactive TUI or unrestricted built-in-tool test.
- Scope: existing DeepSeek, Xiaomi and OpenRouter bindings plus one fixed text model each on LongCat, NVIDIA, Bailian, Kimi and Zhipu. Codex/OAuth, media and other tasks were excluded.
- Bounds: **113 generation-request attempts**, including rejected/failed attempts, under a 160-attempt ceiling; each request capped at 2048 output tokens. Nine model-directory requests in total, at most two per Provider. No automatic retries, fallback, external tool execution or invoice claim.
- Inputs: synthetic `pong`, a `lookup(alpha)` result containing `42`, and a read tool confined to a synthetic `fixture.txt`. No project source or existing conversation history was submitted. Keys stayed in process and were not rewritten; OAuth references were not opened.

## Acceptance by Provider

The rows summarize observed scenarios across bounded batches, not one uniform all-green matrix. Failure stops the remaining unchanged scenarios for that Provider. Explicit diagnosis and changed request/configuration checks are separate attempts and remain in the total above.

| Provider / fixed upstream | Actual binary / consumer evidence | Current boundary |
|---|---|---|
| DeepSeek / `deepseek-flash` | Chat text and tool JSON/SSE; Responses JSON text and tool JSON/SSE; final pi Responses SSE text/read continuation passed | Forced tool selection was rejected under the upstream thinking default. The Agent matrix uses auto and independently requires a real tool call and correct continuation. Responses SSE text is covered by pi rather than a separately repeated Python text cell. |
| Xiaomi / `mimo-v2.6-pro` | Python SDK binary matrix passed both protocols × JSON/SSE × text/tool continuation; pi minimal-effort configuration also passed | With the final explicit `effort:none` pi configuration, one text response completed but did not equal exact `pong`. The remaining tool case in that batch was skipped, not counted as passed. This is an observed output-adherence limit, not a parser fix. |
| OpenRouter / `openai/gpt-6-luna` | Python SDK binary matrix passed both protocols and deliveries; final pi Responses text/read continuation passed | One earlier pi continuation had extra answer formatting. Exact formatting and presence of the correct tool result are reported separately; ordinary tool turns do not prove encrypted-state use. |
| LongCat / `LongCat-2.5-Preview` | Binary Chat JSON/SSE text/tool continuation and final pi Chat read continuation passed | An actual per-chunk timestamp drift failure was reproduced, fixed through a scoped rule and rechecked. Native Responses remains unadmitted. |
| NVIDIA / `nvidia/nemotron-3-super-120b-a12b` | Binary Chat JSON auto-tool continuation, library SSE tool continuation and final pi-through-binary Chat text/read continuation passed | `required` once produced ordinary text containing a tool-like string, not `tool_calls`; it was rejected by the scenario oracle, never parsed into a synthetic call. Earlier Python binary SSE 502s were not fully diagnosed. Later successful calls do not erase that limitation. |
| Bailian / Beijing `qwen3.8-max` | Offline source-based adapter and pi admission only; actual binary request failed | Existing key returned 401 at both Beijing and Singapore directory endpoints. The generation gate returned sanitized 502. No claim of current credential validity or live codec closure; confirm account/key/region before further paid validation. |
| Kimi CN / `kimi-k3` | Model directory succeeded; offline adapter and pi admission only | Direct generation and binary gate returned 429. Rate-limit/quota cause was not independently established; no successful generation or live tool acceptance is claimed. |
| Zhipu CN / `glm-5.3` | Binary Chat JSON/SSE text/tool continuation and final pi Chat text/read continuation passed | Initial pi configuration sent unsupported `minimal` effort and received upstream 400. Corrected client configuration was verified before repeating the gate. Native Responses remains unadmitted. |

The additional five providers currently have **Chat-only startup entries**. That is an implementation admission boundary, not a claim that their products have no Responses API. Public names and environment variables are owned by the [HTTP guide](../../http-gateway.md), and wire rules by the [API-key text profiles](../../architecture-v2/api-key-text-profiles.md).

## Executed discrepancies and ownership

1. **Pi text-array input:** its Chat client sends text content arrays. An independent failing codec test established the gap. Requests now decode into ordered typed parts; insertion/replacement/deletion drive subsequent encoding. Single-part canonical strings and multi-part arrays are distinct projections. Media, empty/unknown arrays and response-array carriers remain rejected.
2. **Output-budget spelling:** selected Chat providers document `max_tokens`. The named provider rule maps the final IR budget both ways at the boundary, rejects conflicting spellings, and does not admit the deprecated alias at OpenBridge client ingress.
3. **LongCat wire:** `lastOne` is a boolean framing hint, not success; `matched_stop` is bounded source-bound stop diagnostics; a static null delta and documented zero-only counters normalize without accepting active unsupported values. The first capture happened within one timestamp and passed; a later actual failure had one ID/model but two `created` values. A synthetic timestamp-drift test failed before the scoped timestamp-only rule was added, then actual binary SSE text/tool continuation passed. Fingerprint and identity checks were not relaxed.
4. **Zhipu diagnostics:** `request_id` uses the existing classified-fidelity dependency/target checks. It cannot overwrite response identity or leak into unrelated client/provider projections.
5. **Bailian inactive placeholders:** the current official Chat page documents `audio:null` and `function_call:null`. Independent fixtures admit only these inactive values and reject active payloads. Because authentication is blocked, this is source-based offline closure, not live evidence.
6. **Pi thinking configuration:** on pi 0.87.1, `thinkingLevelMap.off:null` removes off from supported levels; selecting off is then clamped upward to minimal. A rejecting-loopback capture confirmed `reasoning_effort:"minimal"` in the actual Chat request, and temporary stage diagnostics confirmed Zhipu HTTP 400. The corrected configuration leaves Chat off unmapped (no effort field) and maps Responses off to `none`; a request-time guard now checks these exact fields. System-role compatibility is also explicit. No gateway transform silently changes a requested effort.
7. **Model behavior versus transport:** forced tool selection and literal-answer formatting can fail even when wire consumption succeeds. The final Agent gate requires an actual fixed-path tool execution, actual result serialization in the next request, a normal terminal and the returned value. Exact answer formatting is a separate metric. Previous failures are retained rather than rewritten as codec successes.

## Reproduction and retained evidence

These commands make paid calls and require a freshly bounded target/matrix authorization:

```sh
cargo build --locked --offline --bin openbridge
OPENBRIDGE_PROVIDER_MATRIX=1 OPENBRIDGE_MATRIX_PROVIDERS=longcat \
  uv run --project tests/sdk --locked --offline python examples/live_provider_matrix.py
```

`OPENBRIDGE_MATRIX_PROTOCOL`, `OPENBRIDGE_MATRIX_DELIVERY` and `OPENBRIDGE_MATRIX_CASE` narrow the fixed matrix. The driver enforces at most 66 actual SDK sends and 2048 output tokens per request, uses the real ephemeral binary, disables proxy inheritance in the downstream SDK and only supplies trusted gateway egress. It writes sanitized scenario facts, not bodies or headers.

Portable findings are in this document; ignored local artifacts include:

- `testdata/runtime/provider-onboarding/`: initial bounded synthetic Chat captures and diagnostic scripts; not independent regression fixtures.
- `testdata/runtime/provider-matrix-1790706373469503337/calls.json`: initial binary matrix, including failures and skipped subsequent Provider cases.
- `testdata/runtime/provider-matrix-1790706787891862997/calls.json`: auto-tool DeepSeek/NVIDIA diagnostic batch.
- `testdata/runtime/provider-matrix-1790707007109843450/calls.json`: LongCat timestamp-fix binary acceptance and retained NVIDIA failure.
- `testdata/runtime/provider-matrix-1790708018463608889/calls.json` and `provider-matrix-1790708020787214357/calls.json`: DeepSeek final auto-tool Responses and remaining Chat text cells.
- Library diagnostic directories `probe-2026-09-30-1790706440168-1916139`, `probe-2026-09-30-1790706873035-1917921`, `probe-2026-09-30-1790706957089-1918785` under `testdata/runtime/`: DeepSeek forced-tool rejection, LongCat timestamp drift and NVIDIA library SSE continuation respectively.

The independent local pi configuration/launcher is documented at `~/.pi/openbridge-test/README.md`. Its restricted SDK probe enforces at most three calls per selected model, keeps raw conversations only in memory, and does not inherit project context or Provider credentials into pi. All owned listeners/processes were closed after testing. Temporary stage-only diagnostics used during investigation were removed from the gateway.

## Offline and unexecuted layers

Default locked/offline Rust tests, clippy, formatting, all three pinned synthetic SDK gates, and the probe replay/budget Python unit tests passed. Independent regressions protect provider-profile isolation, invalid/active values, identity and timestamp behavior, static/event consistency, stale-fidelity invalidation, text-array edits, optional native-entry rejection and startup admission.

This does not prove all model parameters, all tools, multi-part input acceptance by every upstream, other accounts/regions/backends, full pi TUI/built-in tool behavior, encrypted replay, cache benefit, production observability, load or long-run stability. Bailian/Kimi remain external acceptance blockers; Xiaomi format adherence and historical NVIDIA failures remain visible limitations rather than reasons to fabricate output or narrow the semantic core.
