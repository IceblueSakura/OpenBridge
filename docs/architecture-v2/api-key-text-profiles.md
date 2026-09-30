# Additional API-key text adapters

The additional LongCat, NVIDIA hosted NIM, Alibaba Cloud Model Studio (Beijing), Kimi CN and Zhipu CN bindings use the same Generation IR and shared JSON/SSE codecs. The fixed model and target declarations live in [`src/topology/catalog.rs`](../../src/topology/catalog.rs) and [`src/provider/catalog.rs`](../../src/provider/catalog.rs); startup variables belong to the [HTTP guide](../http-gateway.md). These are selected text bindings, not the providers' complete model or protocol inventories.

## Native protocol admission

LongCat exposes Chat and native Responses at `/openai/v1/chat/completions` and `/openai/v1/responses`, using the fixed `LongCat-2.5-Preview` target and the same credential binding. The official Codex configuration establishes the Responses base URL; it does not prove all wire branches or live compatibility. NVIDIA, Bailian, Kimi and Zhipu remain Chat-only. `ProviderDefinition.responses` is optional and topology compilation rejects a Responses endpoint without a declared native entry. There is no endpoint probing, dynamic selection, fallback or arbitrary URL supplied by a business request.

Xiaomi exposes both `mimo-v2.6-pro` and `mimo-v2.6-flash` on Chat/Responses. They share the Xiaomi adapter and credential binding, but use separate fixed model/Route/Endpoint identities. Adding Flash does not replace Pro or enable media. Its historical library-only acceptance is separate from current binary verification. DeepSeek and OpenRouter retain their declared dual-protocol entries.

## Named mappings

| Rule | Scope and ownership |
|---|---|
| `max_tokens` | The five additional provider profiles project the final IR output budget to this Chat spelling. Provider request decode accepts it with the same integer constraints and rejects simultaneous `max_completion_tokens`. Standard/OpenBridge ingress still uses `max_completion_tokens`; no client alias is silently enabled. |
| Readable reasoning | Each profile admits assistant `reasoning_content` into the existing typed ReasoningItem immediately ahead of its carrier message. Tool continuation preserves actual returned history. Text is not an encrypted token and does not need an opaque replay origin. |
| LongCat chunk `created` | Bind the first timestamp and normalize later per-chunk timestamp drift. Response ID, model and fingerprint consistency remain strict. |
| LongCat `lastOne` | Boolean advisory frame metadata, validated then discarded. It cannot substitute for `finish_reason`, actual usage or `[DONE]`; an empty early final-looking marker remains an error. |
| LongCat `matched_stop` | Nullable unsigned stop diagnostic under the single choice. Bounded classified fidelity binds it to protocol/profile/trusted origin/response identity/final semantic dependency; only the terminal record is authoritative. No stop reason is inferred from its numeric value. |
| LongCat static `delta:null` | Redundant absent delta alongside the complete message is removed. A non-null static delta is not silently discarded. |
| LongCat Responses usage detail view | `usage.prompt_tokens_details` is admitted only as an object exactly equal to the existing standard `input_tokens_details`. It is validated then discarded, not stored in fidelity or re-emitted. Missing canonical details, conflicts, null and malformed aliases fail. Standard/client profiles do not inherit this rule. |
| Bailian text-only usage view | Input/output `text_tokens` must be unsigned integers exactly equal to `prompt_tokens` / `completion_tokens` respectively. The official contract includes reasoning in output text counts; it is not added again. Only these redundant views are discarded; independent/non-equal modality breakdowns still need typed mapping and reject. Missing fields do not create defaults. JSON and SSE share validation. |
| LongCat zero usage details | Only explicit integer-zero `audio_tokens`, `image_tokens`, `video_tokens`, `text_tokens` details normalize to no supported detail. Positive, null or malformed values require an actual mapping and are rejected; standard totals, cached and reasoning counts stay typed. No whole usage or cache-write count is invented. |
| Bailian inactive message fields | Officially documented `audio:null` and `function_call:null` placeholders normalize to absent. Any non-null value still rejects; this does not enable media or legacy function calls. Source-based synthetic evidence and live acceptance are separate; credential status and executed scenarios belong to the linked evidence. |
| Zhipu `request_id` | Nullable/nonempty string diagnostic with the same bounded source/response dependency rules, not public response identity or task semantics. No arbitrary custom field passthrough. |

Nonportable diagnostics are absent from the OpenBridge client projection and from other origins/profiles or edited responses. Streaming metadata cannot create a successful terminal or restore a deleted semantic owner. Protocol-specific extras reuse the existing classified fidelity mechanism; there are no vendor Usage/Generation variants.

## Pi-facing text arrays

The shared [Chat request profile](chat-text-profile.md) admits nonempty arrays of plain `{ "type": "text", "text": "…" }` parts on user/assistant and system/developer messages. Parts keep their order and semantic identities. Encoding canonicalizes one text part to a string and multiple text parts to an array; it never concatenates independent parts or restores original text after an IR edit. Empty arrays, unknown part fields, media, mixed refusal arrays and malformed text reject. Tool results remain strings. Static Chat responses and event carrier grouping retain their previous single-part limits.

This closes the concrete input shape emitted by pi's Chat client, not all pi features. Client `store`, arbitrary sampling extensions, state, media and unsupported model controls remain outside the current profile. Set pi compatibility flags for the gateway rather than borrowing an upstream provider's identity-based defaults.

## Evidence and limits

Independent synthetic regression assertions are in `tests/semantic/provider_profiles.rs` and `tests/semantic/chat_wire.rs`. Optional native-entry rejection belongs to topology compilation; startup checks exercise explicit multi-model/dual-protocol admission and retain rejection for genuinely Chat-only entries. `examples/replay_chat.rs` is a bounded offline diagnostic for explicitly supplied synthetic captures, not a default test fixture dependency.

Historical onboarding and credential/entitlement blockers are recorded in [the onboarding evidence](../implementation-status/evidence/2026-09-29-api-key-provider-onboarding.md). [The Flash / LongCat / Bailian gate](../implementation-status/evidence/2026-09-30-flash-longcat-bailian-acceptance.md) separately records current binary acceptance, the fixed usage views and remaining timeout/truncation failures; LongCat Responses SSE is not yet live-accepted. Bailian's current normal Chat scenarios passed after the user updated its credential; the historical invalid-key result is not its current blanket status. A configured key, a successful model listing or a permissive SDK parse alone proves no model generation, tool continuation or protocol closure. OAuth/Codex and all other task families remain outside this slice.
