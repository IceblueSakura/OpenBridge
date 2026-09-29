# OpenRouter GPT-6 Luna: bounded live acceptance

- Date: 2026-09-29.
- Implementation: `semantic-v2`, base `dee4c05` plus the OpenRouter adapter/live-closure working-tree changes accompanying this document; not a claim about the unchanged base commit.
- Target: public `gpt-6-luna`, fixed upstream `openai/gpt-6-luna`; Chat Completions and Responses over JSON/SSE. Pro, batch and other models were not invoked.
- Inputs: synthetic `pong`, a small JSON object, and a `lookup` function for key `alpha` returning the synthetic value `42`. No real tool execution, hosted tools, searches or user conversation data.
- Bounds: 40 inference requests total, at most 2048 output tokens/request; no client retries or OpenBridge fallback. Diagnostic body capture is bounded at 2 MiB. OpenRouter's internal routing/account defaults remain external.
- Secrets: read in process from the workspace credential binding; neither credential values nor locators, headers or encrypted replay values are reproduced here. Private configuration was not modified.

## Results by acceptance layer

| Layer | Matrix | Observed result |
|---|---|---|
| Initial library live probe | 2 protocols × 2 deliveries × text/JSON object/tool, first turns | 12 HTTP 200 responses, **0/12 consumed**: real wire shapes contradicted the initial adapter subset |
| Offline replay after fixes | The same 12 bounded captures through Attempt, adapter, ResponseDelivery and client decode | **12/12 consumed**, with no further Provider requests |
| Fresh library live probe | Same 12 cells plus one continuation for each tool cell | **16/16 consumed and scenario oracles passed** |
| Pinned SDK → actual binary → OpenRouter | 2 protocols × 2 deliveries × text and tool (tool continues once) | **12/12 passed**; binary started on an ephemeral literal loopback listener and was shut down afterward |

The final library matrix checks exact `pong`, parsed JSON with the requested bool/string fields, the function name/arguments/call identity, actual assistant history replay and a final answer containing `42`. The four library tool first-turn captures contain no encrypted reasoning token: the continuation code preserves any actual history, but these paid tool continuations do not independently prove encrypted-token reuse. Source-bound token replay/mutation is covered by synthetic regressions. “Consumed” is not merely HTTP 200 or a successful codec parse. The binary gate uses OpenAI SDK `3.19.0` with retries disabled and an SDK HTTP client that ignores ambient proxies; only the trusted gateway egress uses its explicit proxy configuration.

The SDK live gate uses normal SDK consumption, not a claim of strict completeness for every optional reported fact. Its Chat stream consumer explicitly accumulates the admitted indexed reasoning-details extension; success does not establish that every SDK convenience accumulator or Agent preserves unknown delta fields. The independent strict synthetic SDK gates were also executed successfully, but they prove their own fixture contracts rather than live-provider completeness.

## Executed discrepancies and fixes

| Observed shape | Correct ownership / change |
|---|---|
| Chat `reasoning_details` with `reasoning.summary` and/or `reasoning.encrypted`, format `openai-responses-v1`; readable `reasoning` may duplicate the summary | Decode to existing typed ReasoningItem/ReasoningContent and origin-bound ReasoningReplay. Validate the derived view; preserve the issuer identity and actual continuation data. Unknown formats, conflicting views, duplicate encrypted records and invalid grouping fail |
| Issuer reasoning ID arrives after summary deltas | Chat binds the ID before emitting the encrypted detail; no renaming of already-emitted Responses identities |
| Chat `service_tier` and zero audio/video/image usage details | Tier maps to existing standard reported context. This text-only adapter normalizes only integer-zero modality counters; nonzero/null/malformed values remain rejected |
| Responses penalty echoes and `cost_details.upstream_inference_input_cost` / `upstream_inference_output_cost` | Reuse bounded classified response fidelity, with protocol/profile/scope/identity/semantic-dependency checks; never overwrite typed token counts or copy request settings into reported facts |
| Responses reasoning item `format: openai-responses-v1` | Validate the redundant syntax marker, then discard it; unknown formats cannot borrow standard replay permissions |
| `response.function_call_arguments.done.name` | Check against the existing typed call's name; disagreement fails rather than replacing the owner |
| Responses data-only `[DONE]` after `response.completed` | Permit one optional adapter-scoped transport trailer only after a real semantic terminal; early/duplicate/trailing-data cases still fail and EOF still gates success |

The initial Chat repeated-finish usage normalization also remained active. No structural `sequence_number`, output array, item identity, call ID or successful terminal was synthesized to make the live matrix pass. Standard/client profiles do not globally inherit router billing fields or Provider compatibility defaults. Exact admission and remaining limits are maintained in the [OpenRouter text profile](../../architecture-v2/openrouter-text-profile.md), not duplicated as a new schema here.

Independent synthetic regressions are in `tests/semantic/router_adapter.rs` and `tests/semantic/function_events.rs`; existing stream, scope, mutation and gateway tests remain part of the baseline. The probe also tests credential-parse error sanitization and preservation of actual continuation history.

## Reproduction boundaries

The following are **paid opt-in** commands; their presence does not authorize a future run. Agree target, request bounds and data handling before using a real credential.

```sh
OPENBRIDGE_PROBE=1 OPENBRIDGE_PROBE_MODEL=gpt-6-luna \
  OPENBRIDGE_PROBE_MAX_TOKENS=2048 \
  cargo run --locked --offline --example live_probe

cargo build --locked --offline --bin openbridge
OPENBRIDGE_GATEWAY_PROBE=1 uv run --project tests/sdk --locked --offline \
  python examples/live_gateway_probe.py
```

`OPENBRIDGE_PROBE_PROTOCOL=chat|responses`, `OPENBRIDGE_PROBE_CASE=text|json_object|tool` and `OPENBRIDGE_PROBE_DELIVERY=json|sse` narrow the fixed library matrix. The OpenRouter model requires explicit selection; the probe does not silently add it to the predecessor default paid matrix. Capture directories are unique, ignored, and never overwritten by date alone. `OPENBRIDGE_PROBE_REPLAY_DIR` runs the fixed first-turn capture matrix offline without loading credentials or opening Provider connections.

The original local artifacts are retained under ignored directories:

- initial: `testdata/runtime/probe-2026-09-29-1790688767249-1846755/`;
- fresh library: `testdata/runtime/probe-2026-09-29-1790691384353-1858085/`;
- SDK/binary sanitized facts: `testdata/runtime/gateway-luna-1790692140850813664/calls.json`.

Those raw captures are not checked-in test fixtures. This document is the portable, sanitized evidence entry; independently authored synthetic fixtures carry regression assertions. A first SDK launcher setup failed before any HTTP request because it imported a non-installed standalone `httpx`; using the pinned SDK's exported `DefaultHttpxClient`, as the existing SDK gates do, fixed that tooling issue. It is not an upstream failure or an extra inference request.

## What remains unproven

The limits below describe this original matrix. A later, separate [reasoning-continuation gate](2026-09-29-reasoning-continuation-acceptance.md) exercises actual encrypted acquisition and continuation; it does not retroactively turn these ordinary tool turns into encrypted-replay tests.

No media, other model/backend matrix, server tools/plugins, malformed live upstream transport, load, long-run stability, multi-user isolation or production deployment was exercised. The binary live gate covers text and tool continuation, not the library's JSON-object case through the binary. Model output adherence and latency are limited to these small synthetic cases. No total invoice/credit charge was independently audited; request/token bounds are not a billing guarantee. Live encrypted-token continuation, replay validity across different issuers and future backend routing choices are not established by the successful ordinary tool continuations. No additional calls were made beyond the 40-request bound to force those scenarios.
