# ADR-v2-0002: Task IR, Identity, Presence and Extensions

## Status

Accepted.

## Decision

Generation follows ordered item/content semantics; other tasks retain their own request/response/event contracts. Wire family does not determine task identity.

Separate task semantics, standard context, delivery intent, scoped extensions and bounded fidelity. Standard fields have standard owners, not extension copies. Local identities are distinct from wire IDs and indexes; presence is field-specific, and contradictory outer absence/inner values are invalid.

Extensions require explicit attachment, schema, trusted origin, lifecycle, visibility and target requirements. Representation-only preservation stays in fidelity; runtime authority never comes from an extension.

## Rationale and consequences

Stable ownership prevents reordering or transforms from attaching metadata to the wrong value. Replacing/deleting an owner invalidates dependent metadata; normalization must not erase Schema ordering, resource meaning or context lifetime. Opaque replay follows [ADR 0006](0006-reasoning-ownership.md).

Design: [semantic IR](../semantic-ir.md). Implemented types: [Generation](../../../src/semantic/task/generation/mod.rs), [context](../../../src/semantic/context.rs), [fidelity](../../../src/protocol/fidelity.rs), [extension carriers](../../../src/protocol/extensions.rs). These entry points do not imply that every designed task or carrier is wired into the gateway.
