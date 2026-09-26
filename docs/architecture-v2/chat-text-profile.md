# Chat Completions: single-candidate text admission

This is a bounded offline profile over the same Generation IR and reducer as Responses, not a second IR, a running gateway, or full Chat API compatibility. The fixed `openai==3.19.0` consumer in `tests/sdk/` supplies the independently checked `ChatCompletion`, `ChatCompletionChunk`, `CompletionCreateParams` and `ChatCompletionStreamOptionsParam` shapes (Apache-2.0 SDK). Version/source baseline: [upstream sync](../references/upstream-sync.md). This is schema/loopback evidence, not live Provider acceptance.

## Entry points and ownership

- `chat_envelope::decode_request_bytes` / `decode_response_bytes`: strict raw JSON plus complete envelope validation. Value overloads cannot validate lost raw-byte/duplicate-key information.
- `chat_envelope::encode_request` / `encode_response`: validated target representations from lowering, not raw JSON passthrough.
- `chat::decode_generation` and static response codecs remain task-level mappings.
- `ChatSseDecoder` / `ChatSseEncoder`: data-only SSE over the shared framer, strict JSON, event codec and reducer. No socket, credential, retry or tool execution.

`RequestContext` owns the public model label, `n`, `stream` and `stream_options`; these are not message history or upstream routing inputs. Request model binding must be resolved by the trusted caller, not by the codec. Reported response identity/model/created are validated separately rather than copied from request hints.

## Current admission

| Domain | Supported behavior | Rejection / limitation |
|---|---|---|
| Request envelope | Required model/messages; n absent/null/1 means the single candidate; stream absent/null/false is static | Multiple/zero candidates, wrong types and unknown controls fail |
| Task controls | temperature, top_p, max_completion_tokens, function tools/choice/parallel_tool_calls, existing reasoning_effort mapping, response_format's text/json_object/json_schema mapped to the shared output constraint owner | Additional sampling controls, verbosity/logprobs/truncation and deprecated aliases are not admitted here; function output_schema has no Chat projection |
| History | system/developer text instructions, user/assistant text, assistant refusal, function calls and text tool results; replayed assistant `parsed`/`parsed_arguments` views and call `index` artifacts validated against the raw body | No media, custom/hosted tools, third-party reasoning text or state resources; Responses `phase` labels have no Chat projection |
| Complete response | Required id/model, integer created, chat.completion, one choice at index 0, assistant message, stop/tool_calls/length | No synthetic upstream headers; content_filter/deprecated function_call and multiple candidates are not yet mapped |
| Stream chunks | Required stable identity/model/integer created, chat.completion.chunk, delta/index; nullable content/refusal/tool_calls; function argument fragments | Additional standard response metadata such as service_tier/system_fingerprint/moderation and logprobs are not yet admitted; no generic pass-through |
| Message ownership | One candidate has one assistant owner, including tool-only output; static and event decode preserve the same grouping | Mixed text/refusal and unsupported target groupings fail rather than flattening |
| Usage | Optional reported usage and supported details; stream may have a choices=[] usage tail after finish_reason | Duplicate/early usage and business chunks after finish_reason fail; missing counts are not invented |

Unknown fields are explicit errors, including presently unsupported standard fields. This table defines a subset, not a claim that those fields are nonstandard. Existing Responses-only semantics stay in the IR; an unavailable Chat projection must reject them rather than narrow the Responses contract.

## Output constraint shells

Chat `response_format` and Responses `text.format` map to the same `TextOptions.format` owner (`Presence<OutputConstraint>`); the shells are parsed and encoded independently and are not byte-isomorphic. Chat nests the schema body under a `json_schema` object, Responses places `name`/`description`/`schema`/`strict` beside `type`. `text` and `json_object` accept only `type`; unknown keys, unknown `type` values and wrong value types fail in both shells.

Outer presence follows the shared owner's rule that admitted child nulls and explicit defaults remain visible: a missing `response_format` is `Absent`; an explicit `null` is `Null` (present, not absent) and re-emits `null`; an object is `Value`. An explicit `{"type": "text"}` is the visible default: wire-distinct from omission and never synthesized from a missing or empty `text` container or verbosity-only settings. The pinned `openai==3.19.0` create type marks `response_format` non-nullable; admitting explicit null as a visible distinction is a local choice matching the shared `text.format` contract, not a claim that every server accepts it.

Inside the schema body the two shells share one field contract: `name` is required and bounded; `schema` is required and authoritative and fails rather than defaulting; `description` missing or null equals no description and is never re-emitted as null; `strict` missing or null is the general-structural mode with no filled default while an explicit `false` stays visible. Structural, strict and local-reference admission stays in the [schema profile](schema-profile.md); nothing here rewrites the schema.

A request constraint is not a response fact: Chat responses and chunks never echo `response_format` or settings. The mapping opens no other boundary: target structured-output capability, the Chat non-strict function default, and the rejection of function `output_schema`, verbosity, logprobs and truncation all still apply.

## Derived view replay

The pinned SDK's `ParsedChatCompletionMessage.parsed` sits beside `content` on assistant messages and derives from the message body. History replay admits the key on assistant messages only: missing or null is the SDK's no-view state and is dropped; a non-null view must equal, as a JSON value, the naive JSON parse of `content`, and a missing or null body cannot carry a non-null view. User/system/developer/tool messages reject the key outright. Static responses and chunks never carry or echo it: the typed `parsed` lives in the SDK client, not on the wire.

The view is validated then dropped following the shared [derived replay rules](responses-text-profile.md#derived-replay-views): it never enters the IR, is never emitted, and a replaced body cannot resurrect it. Function `function.parsed_arguments` follows the same rule against the authoritative `arguments` string on replayed assistant `tool_calls` and is dropped the same way. The SDK's chat stream accumulation also leaks the chunk `index` into message-level calls: on replay a present index must equal the call's position (the same ordinal rule the chunk layer enforces) and is dropped afterwards; a missing or null index is the static shape. Static responses and chunks never carry or echo any of these: the typed views live in the SDK client, not on the wire.

## Delivery and terminal rules

`stream_options` objects require `stream=true`. Child options are optional booleans, not nullable: `include_usage` defaults false; `include_obfuscation` defaults true. The raw request presence is preserved independently of these effective defaults.

- With include_usage=true, the encoder requires an observed semantic Usage before terminal and emits the usage tail. With false, it omits the usage wire chunk without changing the semantic event/state. The decoder accepts a usage tail when reported; it does not invent one or infer the originating request's options.
- Padding is wire-only. Enabled encoding requires caller-supplied `Obfuscation::Seeded`; disabled delivery requires `Disabled`. The shared padding budget is charged before allocation. Decoding charges decoded UTF-8 padding before discarding it; exhausted budgets poison the stream. Synthetic seeds do not prove production entropy or side-channel resistance.
- finish_reason closes candidate output, not transport. Only a literal framed `data: [DONE]` closes the semantic stream. The encoder emits exactly one DONE after successful terminal encoding, never on cancellation/error or missing requested usage.
- SSE event names must be absent/empty or the default `message`. EOF requires a complete delimiter and DONE. Missing/early/duplicate DONE, malformed JSON, post-finish business data and post-DONE payloads cannot recover into success.
- HTTP admission requires 200 and text/event-stream with UTF-8 if a charset is specified. `SseLimits` independently limits single events, total wire, event count (including DONE) and aggregate padding. Shared JSON and semantic budgets additionally apply.

Callers retain unconsumed bytes from `consume`, finish framing before materializing, bound body collection and handle cancellation/backpressure. The adapters do not buffer an unbounded stream or implement production retry/commit behavior.

## Independent acceptance and remaining work

`tests/transport/chat.rs` covers independent static/event fixtures, fragmentation, request projection to Responses, output-constraint byte entry and dual-shell projection, final-event mutation, refusal/empty/length, failure closure and resource limits. `tests/sdk/chat_text_loop.py` exercises SDK `parse()`/stream and typed chunks through function-tool turns with a structured-output request, then replays the dumped derived views in later requests; the server synthesizes the final structured text from modified IR and asserts the replayed raw bodies stay authoritative. It does not claim model-output adherence or every parse branch. The existing Responses SDK gate remains separate; both use only synthetic loopback.

Function schemas use shared [structural/strict/reference validation](schema-profile.md); omitted Chat strict stays non-strict rather than adopting the Responses normalization default. Broader field/event admission and additional finish reasons remain future text slices. Media, multi-candidate output and production/Provider execution are outside this profile. See [next goal](../implementation-plans/next-goal.md) and [development commands](../development.md).
