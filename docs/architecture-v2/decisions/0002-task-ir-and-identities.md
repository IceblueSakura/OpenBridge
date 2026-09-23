# ADR-v2-0002: Task IR, Identity, Presence and Extensions

## Status

Accepted design direction; extension wire syntax and remaining task types are not yet implemented.

## Decision

Generation follows Responses ordered item/content semantics. Other tasks retain task-specific request/response/event contracts; a Chat-shaped speech endpoint is not necessarily conversation generation.

The overall internal representation separates standard task semantics, typed context, delivery intent, scoped extensions and bounded fidelity. Session/context extensions are part of this representation without entering message content or acquiring runtime authority. Attachment, schema, origin, lifecycle, visibility and target requirements follow [semantic-ir.md](../semantic-ir.md).

Stable local identities are distinct from wire IDs and indexes. Presence follows each field's semantics; contradictory outer absence and inner values are invalid. Standardized fields have standard owners rather than duplicate extension copies.

Opaque replay retains a typed owner and trusted source scope. Representation-only preservation stays in fidelity; modeled special capability lives in a typed extension, not arbitrary raw JSON. Reasoning finality and dependency checks follow [reasoning ownership](0006-reasoning-ownership.md).

## Consequences

Transforms preserve or explicitly replace identities and invalidate dependent metadata. Schema key ordering, media source semantics and context lifetime cannot be erased by generic normalization. Old source layout does not constrain the implementation; current scope and acceptance are explicit per slice.
