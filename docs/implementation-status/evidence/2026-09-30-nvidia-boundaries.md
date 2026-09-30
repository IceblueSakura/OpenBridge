# NVIDIA targeted boundaries and remaining failures

- Date: 2026-09-30 UTC; runtime baseline `ded965e`, with the accompanying Python probe changes. Runtime changes used solely for status/stage diagnosis were removed and the normal binary rebuilt.
- Target: NVIDIA hosted NIM, public `nemotron-3-super` → `nvidia/nemotron-3-super-120b-a12b`, native Chat only. Kimi was skipped completely; no other Provider was called.
- Bounds: **16 generation requests plus one NVIDIA model-directory lookup**. Serial, no automatic retries, at most 2048 output tokens/request; explicit short-budget cases used 8. Synthetic JSON and lookup inputs only. Account credentials were not changed; error bodies, headers and credentials were not printed.
- Consumer: fixed OpenAI Python SDK `3.19.0` through the actual ephemeral binary, plus one existing library-level diagnostic pair. A request-count ceiling does not establish an invoice or general free-tier entitlement.

## Observations

| Scenario | Observation | Boundary |
|---|---|---|
| JSON object, default reasoning, JSON delivery | Initial request failed its expected-terminal assertion. A single diagnostic repetition returned HTTP 200, `finish_reason=length`, reported 2048 output tokens, 2 visible text characters and 174 readable-reasoning characters | The second call establishes valid budget truncation, not a transport failure or a completed JSON answer. The first call's exact unexpected terminal was not retained; do not retroactively invent it. These counts do not independently establish why the backend exhausted its budget. |
| JSON object, explicit `reasoning_effort=none` | JSON and SSE both completed with the independently expected object `{"answer":7}` and no tool calls; no readable reasoning was reported in these two samples | A tested explicit request setting, not a default change, guarantee of internal reasoning suppression, or claim that default reasoning is generally unsupported. |
| First JSON tool-history attempt | Real `lookup(alpha)` call passed; the tool-result continuation returned downstream 502 after approximately 177 ms | The failing call's upstream status and codec stage were not observed. No evidence establishes that it was rate limiting or a gateway codec defect. |
| Library-level diagnostic pair | Both native JSON tool turns passed, with actual history carried forward | Different calls/prompt details; success does not explain the previous binary failure. |
| One instrumented binary JSON history group | Four requests covering `lookup(alpha)` → result 17 → `lookup(beta)` → result 29 passed. Upstream HTTP status was 200 for all four | The earlier 502 did not reproduce. The SDK's actual serialized complete history, including call IDs, argument strings, results and readable reasoning, matched the immutable expected history before each send. This is not proof of internal Provider use or byte-identical upstream serialization. |
| 8-token truncation | JSON and SSE both returned `length`, consumed by a scenario that explicitly expected it; JSON reported 8 output tokens | Normal success scenarios still reject `length`; it was not rewritten into `stop`. |
| Early SSE close and following request | Client received meaningful content/reasoning before any observed finish, explicitly closed the stream, and a subsequent text request succeeded | Proves the selected client-close path and later request availability, not upstream compute cancellation, billing cessation or semaphore-leak freedom. |

The generation count is 2 default-JSON attempts + 2 calls in the failing history group + 2 library diagnostic calls + 4 calls in the instrumented history group + 2 short-budget cases + 2 close/follow-up cases + 2 explicit-none JSON cases = 16. Four-request **SSE** tool history was not executed: failures and targeted diagnosis used the remaining budget. No further calls were made to complete a cosmetically all-green matrix.

## Probe corrections and safeguards

- `examples/live_nvidia_probe.py` fixes the model, owned loopback destination, path, request/body/output ceilings and serialized history. Its full planned matrix is 14 requests with a hard send ceiling of 16; filter variables only narrow the matrix. Any failure stops that batch.
- `chat_result` keeps its normal `stop/tool_calls` rule. Only a scenario that explicitly supplies `allowed_finishes=('length',)` accepts truncation. `UnexpectedChatFinish` carries a closed terminal category, not response text.
- A valid but unexpected terminal is now classified as `terminal_oracle`, rather than suggesting broken transport. The diagnostic report that exposed this bug still contains its earlier coarse classification; the preserved terminal and token facts are the evidence.
- Retry-After parsing accepts bounded delay seconds or dates, never arbitrary header text. **No live 429 was observed.** Current gateway policy hides non-429 upstream errors as 502 and does not forward Retry-After. A null downstream Retry-After is not proof that the Provider omitted it. Synthetic parser tests do not establish real rate-limit or backoff behavior.
- The general live matrix now defaults to NVIDIA first, excludes paused Kimi, and rejects explicit paused/unknown/duplicate selections before credential loading. Its current hard send ceiling is 60. Product bindings and offline semantic coverage were not removed.

Temporary gateway diagnostics emitted only HTTP status numbers and fixed stage labels. They saw only successful HTTP 200 responses in the instrumented groups, not the original failing call. They were removed from both gateway source and probe collection; no production diagnostic setting was introduced.

## Reproduction and retained evidence

All live commands require a newly authorized bounded batch. For the successful explicit JSON control:

```sh
cargo build --locked --offline --bin openbridge
OPENBRIDGE_NVIDIA_PROBE=1 OPENBRIDGE_NVIDIA_CASE=json OPENBRIDGE_NVIDIA_EFFORT=none \
  uv run --project tests/sdk --locked --offline python examples/live_nvidia_probe.py
```

Other fixed cases: `history`, `length`, `cancel`; delivery can be narrowed with `OPENBRIDGE_NVIDIA_DELIVERY=json|sse`. Omitting effort retains the Provider default and deliberately preserves the default-JSON failure boundary; it is not a promise that the full default matrix is green.

Offline guard tests:

```sh
uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p 'test_*.py'
```

Sanitized reports under ignored `testdata/runtime/`:

- `nvidia-boundaries-1790735228214503084/calls.json`: initial default JSON failure.
- `nvidia-boundaries-1790735454462606330/calls.json`: diagnostic terminal/usage classification.
- `nvidia-boundaries-1790735735327010296/calls.json`: real binary continuation 502.
- `probe-2026-09-30-1790735951153-2005323/`: library diagnostic pair, including bounded synthetic captures not used as independent fixtures.
- `nvidia-boundaries-1790736137874192464/calls.json`: four-request JSON history.
- `nvidia-boundaries-1790736290044225442/calls.json`: short-budget JSON/SSE.
- `nvidia-boundaries-1790736291597068287/calls.json`: close and subsequent request.
- `nvidia-boundaries-1790736293459979748/calls.json`: explicit-none JSON/SSE.

The existing [onboarding](2026-09-29-api-key-provider-onboarding.md) and [follow-up](2026-09-29-provider-followup.md) retain their original scope. This batch adds boundary evidence; it does not resolve intermittent failures, prove throughput/long-run reliability, or modify the semantic core to hide model behavior.
