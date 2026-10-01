# ADR-v2-0006: Reasoning and Replay Ownership

## Status

Accepted.

## Decision

- Reasoning controls belong to task semantics. Readable reasoning and summary are typed parts, never assistant text; their identities are distinct from wire indexes.
- Opaque replay belongs to bounded fidelity attached to a surviving reasoning owner. Encoding requires matching trusted origin and valid owner dependencies; protocol equality alone is insufficient. Unknown origin permits intake, not replay.
- Item-start replay may be partial. Item-finish owns the final token and replaces the partial value, including removal. Event encoding cannot recover a token from stale fidelity.
- One ordered reducer owns item/part/value/response lifecycle. Static and event materialization must agree; codecs validate snapshots rather than repair missing payload from terminal JSON.

## Rationale and consequences

Responses output is an ordered heterogeneous log. Flattening reasoning or maintaining unrelated accumulators loses identity, order and finality. Owner edits invalidate replay until it is explicitly removed or replaced by a trusted transform; deleted owners cannot be recreated by fidelity.

Target carriers must preserve owner completion and token finality or reject projection. A later response outcome does not retroactively alter an already completed owner. Invalid reduction poisons the stream; resource bounds apply to both semantic state and replay. Transport closure remains outside the reducer and cannot be inferred from EOF alone.

Owners: [reasoning types](../../../src/semantic/task/generation/reasoning.rs), [reducer](../../../src/semantic/task/generation/event.rs), [fidelity](../../../src/protocol/fidelity.rs), [Chat carrier](../../../src/protocol/openai/chat_reasoning.rs). Field admission and local compatibility forms belong to the [Responses](../responses-text-profile.md) and [Chat](../chat-text-profile.md) profiles, not this decision.
