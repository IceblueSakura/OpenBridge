# Shared probe execution and original-attempt diagnostics

Date: 2026-09-30 UTC. Runtime/library baseline `5c382d0` plus this change; fixed OpenAI SDK `3.19.0`, HTTPX2 `2.13.1`, pi `0.87.1`. Operations and interpretation are maintained in [the probe guide](../../probes.md), not this historical report.

## Executed scope

One shared run capped at 32 sends and 2048 output tokens/request was used across separate SDK processes, the repository-owned pi child and the Rust library example. Short-budget cases used 8 tokens. **22 actual reservations/sends** occurred: NVIDIA 7, DeepSeek 11, OpenRouter/Luna 4. No Kimi, model-directory requests, retry/fallback or raw capture was used. Synthetic prompts and lookup/read values only; keys were read in memory and never changed.

The ledger retained 21 passed attempts, one failed attempt and five planned `not_run` entries. The failure was preserved; the NVIDIA target stopped and its remaining cases were not replayed. The unused budget was not spent to manufacture a fully green matrix.

## Original NVIDIA failure is now localized

The four-request JSON history and its strict numeric results passed, as did JSON short-budget `length`. The SSE history's first tool call passed. Its result continuation, **attempt 7**, failed:

| Boundary | Observed fact |
|---|---|
| Downstream | HTTP 502; SDK did not consume a generation result |
| Actual upstream response of the same attempt | HTTP 200 |
| Gateway failure phase | `intake` |
| Upstream bytes observed before failure | 116 |
| Generation bytes handed off | 0; the separately generated sanitized HTTP error body is not counted here |
| Retry-After | No usable delay recorded; not proof that every upstream form/header was absent |
| Policy | Target stopped; five dependent/later NVIDIA requests remain `not_run` |

This is stronger evidence than a subsequent successful repetition: it identifies the failing request's input boundary without reading or logging its payload. It does **not** establish whether those bytes were an upstream streamed error or another wire incompatibility, nor whether the codec rejected valid or invalid data. It is not evidence of HTTP 429. Earlier NVIDIA failures in other batches cannot automatically be assigned the same cause.

## Positive cross-entry validation

- DeepSeek Responses JSON/SSE text and actual tool-result continuation: six SDK requests passed with raw wire closure, SDK consumption and strict content checks separated.
- The repository-owned pi gate used three further DeepSeek requests for text and synthetic read/result continuation. Actual client Authorization was preserved, wire/typed text matched, and reservations continued the same ledger sequence rather than restarting a local batch counter.
- The Rust native Chat tool diagnostic used the next two DeepSeek slots. Its model-directory precheck and raw capture were disabled. Native results were settled into the same SQLite ledger; its boundary is intentionally independent of gateway diagnostics.
- Luna Responses JSON/SSE acquired and replayed actual opaque reasoning across two turns per delivery. Nonempty issuer IDs/tokens were required, and complete serialized history was compared in memory before send. Four requests passed. No ciphertext or reasoning text was persisted; acceptance does not prove internal Provider use or cache benefit.

All 20 actual gateway attempts had matching operator records. The two native-library attempts correctly had no gateway record. No raw capture files were created. Formatting/documentation refinements after these runs did not constitute additional live validation; per-attempt source fingerprints retain the executed source versions, not a claim that every later byte of tooling was exercised against a Provider.

## Independent local boundaries

Synthetic tests separately cover:

- Original 429/503 status and a failing static JSON final-intake phase, without exposing body or changing existing client error mapping.
- Authentication before diagnostic collection; private new-file creation; queue-full/disconnected sink not blocking a request.
- Cross-process quota contention, crash-surviving reservations, immutable plan views, tamper rejection and expired-plan readback.
- Actual send destination/model/cap/history guards, including opaque issuer mutation and readable-history deletion.
- UTF-8/SSE raw budgets, missing/duplicate/trailing terminal data, sticky failure, and an actual fixed SDK consumer unable to hide data after DONE.
- Runtime checks under optimized Python, rather than relying on removable `assert` statements.
- Pi's pre-await local slot reservation, and actual Chat/Responses SDK→relay→binary wrong-token 401/zero-egress versus correct-token/rejecting-proxy controls.
- Explicit native capture rejects current-key echoes and omits known opaque/signature fields; redacted captures are not independent wire fixtures.

Rust baseline/clippy/fmt, fixed SDK synthetic gates and Python/Node checks remain separate from live/model-quality acceptance. No live rate-limit, production logging, load, long-run stability, full TUI or remote compute-cancellation claim is made.

## Local evidence

The ignored run directory is `testdata/runtime/probe-consolidation-20260930/`: immutable `plan.json`, authoritative `ledger.sqlite3`, derived `summary.json`, four bounded `gateway-*.jsonl` files and the native example's subdirectory. Reports contain closed metadata, not payloads, credential locators or raw errors. The operator ID joins the **same** HTTP attempt; it is not a downstream attestation or semantic field.
