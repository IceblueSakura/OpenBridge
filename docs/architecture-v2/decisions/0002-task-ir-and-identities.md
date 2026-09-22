# ADR-v2-0002: Task IR, Identity and Presence

## Status

Accepted.

## Decision

OpenBridge uses a closed task family with task-specific request/response/event types. There is no universal inference request.

Semantic entities use explicit scoped identities rather than array position. Presence is modeled according to semantics, not uniformly collapsed into `Option<T>`.

Protocol/provider-specific opaque information is kept in bounded fidelity records outside task request/response item semantics. Lifecycle events can carry typed sidecar updates without making opaque state a readable content part; origin, finality and ownership follow [ADR-v2-0006](0006-reasoning-ownership.md).

## Rationale

Task-specific types make illegal combinations difficult to represent and prevent endpoint shape from becoming the domain model. Stable identity is required for transformations, annotations, streaming deltas and source-record validity. Explicit presence prevents a codec from accidentally equating omission, null, empty and explicit defaults.

## Consequences

Existing `src/ir/generation` is design input, not the required v2 module/API. Its useful semantic types may be ported selectively after the new ownership rules are applied.
