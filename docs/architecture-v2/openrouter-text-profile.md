# OpenRouter text adapter

This is the admitted `Dialect::OpenRouter` wire subset over the shared [Chat](chat-text-profile.md) and [Responses](responses-text-profile.md) codecs, not full OpenRouter compatibility. Live observations remain bounded by the [dated acceptance evidence](../implementation-status/evidence/2026-09-29-openrouter-luna-acceptance.md). Sources and their limits are recorded in the [OpenRouter reference](../references/providers/openrouter-api.md). No provider-specific semantic types are introduced.

## Request and binding

The fixed public model `gpt-6-luna` maps to `openai/gpt-6-luna`. Provider origin, paths and Bearer authentication live in `src/provider/catalog.rs`; endpoint/public representation contracts live in `src/topology/catalog.rs`. Bootstrap activation is documented in the [HTTP guide](../http-gateway.md).

The adapter encodes `provider: {"require_parameters": true}` as a trusted fixed policy so OpenRouter must select a backend declaring support for the request parameters. Client JSON cannot override this policy or select backend providers, URLs, keys, plugins or metadata headers. OpenRouter's own routing/account policies are external; this does not disable its internal fallback or account-default plugins.

Current text admission includes instructions, output limits, function calls/results, structured output and reasoning effort. The fixed public contract does not admit temperature/top_p, logprobs, verbosity, truncation or enabled parallel calls. These are local admission limits informed by the model directory, not a claim that the Responses protocol cannot represent them. Existing strict-tool and media restrictions remain; the upstream model advertises media inputs, but those codecs are outside this slice. Cache hints remain inactive target omissions, not a promise of cache affinity.

Chat readable reasoning is encoded as `reasoning` from the final typed reasoning item, including replay from the OpenBridge client's `reasoning_content` carrier. The adapter never restores source text after editing or deletion. Request model and routing policy come from the fixed target, not source wire metadata.

## Response mappings

| Wire shape | Ownership and admission |
|---|---|
| Chat `message.reasoning` / `delta.reasoning` | Same typed readable reasoning item as the shared Chat carrier; string/null only, conflicting `reasoning_content` fails |
| Chat `reasoning_details` | Null/empty means no details. The `openai-responses-v1` format admits one text/summary part followed by an optional encrypted detail, mapped to existing ReasoningItem/ReasoningReplay; unknown formats/kinds/groupings fail |
| Chat `service_tier` | Existing typed reported context, presence-preserving; first Chat report binds, conflicting later reports fail. Responses snapshots retain their separate mutable reported-settings lifecycle |
| Responses reasoning item `format` | Only `openai-responses-v1` is a validated redundant syntax marker; discard after validation, never use unknown formats as opaque replay |
| Responses argument-done `name` | Optional redundant view of the existing function name; if present it must equal the typed call owner, not replace it |
| `provider`, Chat `choices[0].native_finish_reason` | Classified router facts, nullable strings; normalized `finish_reason` remains the sole terminal authority |
| `usage.cost`, `usage.is_byok`, `usage.cost_details` | Classified billing facts; cost is null or finite nonnegative number, BYOK is null/bool, details permit `upstream_inference_cost`, Chat `upstream_inference_prompt_cost`/`upstream_inference_completions_cost`, and Responses `upstream_inference_input_cost`/`upstream_inference_output_cost`, with nullable nonnegative numeric values |
| Standard token counts/details | Shared typed Usage with no compatibility defaults; absent cache-write stays absent, invalid counts fail. In this text-only adapter, explicit integer zero Chat audio/video input and audio/image output counts normalize to no modality usage; nonzero/null/malformed values fail instead of being dropped |
| Responses penalty echoes | Reuse the classified `frequency_penalty`/`presence_penalty` response-extras contract; no request-to-response copying |

Classified facts share an aggregate 4096-byte budget (including field paths). They bind protocol, adapter contract, trusted scope, response identity and a digest of the final typed response. They re-encode only for that compatible source and unchanged semantics, never into standard/client profiles. They cannot restore deleted usage or override token counts. Unknown fields, server-tool billing branches and unmodeled standard usage details remain errors.

Only terminal facts are retained. Chat combines classified fields from the finish chunk and its optional usage tail; nonterminal fields are checked but not saved as final facts. Responses captures its terminal snapshot. No event log or raw response is retained for replay. Both static and event encoding use the same final fidelity projection.

## Structured reasoning ownership

OpenRouter and the OpenBridge client Chat adapters admit the structured carrier; standard, DeepSeek and Xiaomi do not inherit it. A readable summary remains `ReasoningContent::Summary`, not assistant text or generic opaque JSON. A simultaneous `reasoning`/`reasoning_content` view must equal the detail's readable fragment. The readable detail has index 0; the encrypted detail has index 0 alone or index 1 after that part. Unknown metadata, multiple readable parts and conflicting identities/order fail. A terminal empty readable-text part has no lossless plain Chat carrier and is rejected by lowering rather than causing an encoder panic; empty summary parts keep their explicit structured carrier.

The encrypted detail retains its issuer item ID and uses the existing replay token owner, semantic-dependency fingerprint and trusted origin. Replay requires a completed token and a matching endpoint/client scope. Semantic edits invalidate the old token; it is not source text that can be restored. Static and incremental paths map to the same IR. A final encrypted detail requires a completed reasoning owner, independent of the response/carrier outcome: a later `length` keeps that owner completed. Partial tokens and incomplete reasoning owners have no faithful Chat encrypted carrier and fail before emission rather than being dropped or promoted. In streams, the complete encrypted detail closes its reasoning owner before the carrier message; repeated/fragmented encrypted records and new reasoning owners arriving after message content fail. A late issuer ID is bound before Chat emits the encrypted detail; Responses must not rename an already emitted item.

The client emits `reasoning_details` for structured summaries/tokens and continues to use `reasoning_content` for the plain readable carrier. This is an explicit OpenBridge Chat extension, not a claim that every OpenAI Chat SDK stream accumulator preserves unknown fields. Tool/history replay must preserve the actual emitted details; the live SDK probe accumulates their indexed fragments explicitly.

## Streaming closure

The OpenRouter Chat usage tail may repeat the prior finish reason with one choice instead of the standard empty choices array. This rule is isolated to the adapter: the repeated choice must have index zero, the exact same normalized finish reason, null/absent logprobs, and a delta containing only absent/null/empty content and optional assistant role. It must carry valid usage. Additional text, calls, reasoning, a changed finish reason or duplicate usage fails and poisons the stream. `[DONE]` and strict EOF remain mandatory; repeated finish is not a second semantic terminal.

Responses keeps the standard complete snapshot/event grammar, including `sequence_number`, output arrays and item identities. Only this adapter additionally accepts one optional data-only `[DONE]` trailer **after** a validated semantic terminal; it cannot replace a terminal, repeat or allow later data. Strict EOF still gates delivery. Abbreviated guide examples do not authorize fabricated fields or translation of unknown events. Missing or invalid terminals never become success.

## Evidence and gaps

`tests/semantic/router_adapter.rs` supplies independent synthetic JSON/SSE expectations for reasoning, usage, source isolation, mutation and failures. `tests/transport/chain.rs` verifies trusted request policy and model admission; bootstrap owner tests verify explicit key activation and rejection of empty credentials. Those synthetic checks alone are not evidence of account access or backend acceptance. The separate [reasoning-continuation evidence](../implementation-status/evidence/2026-09-29-reasoning-continuation-acceptance.md) records actual encrypted acquisition/continuation through the SDK and binary, including a memory-only check of the SDK's serialized replay values; it does not prove internal Provider use of those values.

Other reasoning formats/groupings/signatures, media/nonzero modality counts, server tools/plugins and arbitrary routing controls remain unadmitted. They fail closed, not by stripping data. A selected live matrix proves only its model, account, payloads and implementation; it does not prove all backend routing choices, opaque replay portability across issuers, model quality, load or production behavior. Public model/endpoint GETs alone do not prove inference compatibility.
