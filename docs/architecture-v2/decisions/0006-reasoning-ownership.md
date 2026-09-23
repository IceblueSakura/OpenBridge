# ADR-v2-0006: Reasoning and Replay Ownership

## Status

Accepted for the `semantic-v2` rewrite epoch. This defines the offline core. The predecessor production runtime is [archived](../../archive.md); no v2 production interface is implied.

## Context

A Responses output is an ordered heterogeneous item log, not a sequence of Chat messages. Readable reasoning and provider-owned continuation must not be flattened into assistant text. Independent text/call/reasoning accumulators also lose global indexes, part identities and terminal coherence.

## Decision

1. **Request controls** belong to Task IR. Absent reasoning, a present empty object, omitted effort, explicit `none`, and disabled summary are distinct. The encrypted-output include is a typed output request, not an opaque token. Standard effort labels, including `max`, are represented independently from target model capability. Unsupported target projections fail explicitly; Chat support does not define the Responses value domain. Standard `configuration_update` items require ordered ownership of changes affecting subsequent responses, not an untyped settings patch.
2. **Readable reasoning** belongs to `ReasoningItem`. Summary and reasoning text are separately typed parts with stable identities, never assistant message text. Summary and content have separate wire index spaces but share the semantic part identity allocator.
3. **Opaque replay** belongs to a bounded representation sidecar. `ReasoningReplay` contains an `EncryptedReasoning` phase and an optional trusted `ReplayOrigin`; the origin is an opaque scope label, not a provider URL or credential locator. Unknown origin permits decoding, not encoding. Every target encoding requires an exact nonempty origin match. A trusted decode boundary may bind unbound records; conflicting rebinding fails.
4. **Event authority** is explicit. Item start may carry a partial encrypted snapshot; only item finish carries a final replay token. The final event replaces the partial value, including removing it when absent. Event encoders must use this event value, never the decoder's old fidelity token. Item-done and terminal output encode the same final token.
5. **Owner dependency** is checked for static replay. Fidelity binds each token to its surviving reasoning item with a digest of typed part identities, kinds, contents and lifecycle. Editing that owner invalidates replay until a trusted transform explicitly removes or replaces the record. Deleted/non-reasoning owners are irrelevant to target checks. Fidelity cannot create an item.
6. **Lifecycle** uses one ordered reducer: response start → item start → part start → deltas → value finish → part finish → item finish → response terminal. Function arguments have value/part closure independent of call closure. Protocol codecs translate wire indexes to identities and validate snapshots; they do not repair missing semantic payload from terminal JSON. A reasoning-text profile may omit generic part-close events after its explicit text done; the decoder closes that part at item done. Missing value done cannot be synthesized into successful closure.

## Standard target and current subset

The fixed public baseline contains `none` / `minimal` / `low` / `medium` / `high` / `xhigh` / `max` effort, string summary controls, context and mode. Per-model support and effective response settings remain separate. Current code also accepts `summary:false`; this is a compatibility form, not a standard enum in the refreshed baseline. Its profile disposition is tracked in [migration](../migration.md).

Current encrypted-output include and empty-include normalization do not imply that replay depends exclusively on the request flag: the refreshed official guide describes default encrypted reasoning in stateless mode. Finality, surviving owner and trusted origin still govern replay. Explicit `none` with an enabled summary remains a current local validation rule, not proof of every Provider's behavior.

Static and event Responses codecs support reasoning summary/text, opaque replay, assistant text/refusal, function calls, usage, completed/incomplete/failed/cancelled outcomes and bounded typed terminal details. Message, call and reasoning item status is independent from response outcome. Empty output and empty text remain meaningful.

Current context/mode, custom tools and text metadata have partial implementations; configuration updates, hosted tools, active state and media are still gaps. The standard target is broader than this subset. Unknown fields and unsupported mappings must fail closed. Current admission belongs to [responses-text-profile](../responses-text-profile.md); external standard facts belong to [the refreshed baseline](../../references/responses-standard.md), not this implementation summary.

## Consequences and acceptance

- Same-protocol and cross-protocol lowering use the same immutable IR authority. No Native bypass or post-encode body hook is introduced.
- Codecs share the semantic reducer; SSE framing, Chat `[DONE]` detection, delivery/commit, routing and network remain outside it. Chat finish reason precedes optional usage; only a real framing-level `[DONE]` closes the decoder. Transport EOF is not that marker.
- Static and Event materialization must agree on supported text, reasoning, tool, usage and terminal semantics. Unsupported Chat grouping, per-item partial history, failure details or replay is rejected rather than silently joined or dropped.
- Bounds cover part size, total semantic state, part/item counts, replay records and encoded payloads. Invalid reduction or decoding poisons the stream rather than permitting continuation after partial mutation.
- Independent conformance checks must protect mixed item order/indexes, global part identity, value/part/item/response closure, origin mismatch, stale owner/deletion, token replacement, partial results, budgets and two-turn parallel tool replay. Round trips alone are insufficient.
