# ADR-v2-0001: Responses-first Semantic Authority

## Status

Accepted.

## Decision

Generation uses the fixed OpenAI Responses standard as its semantic backbone, with scoped typed extensions for capabilities outside it. It is neither a Chat/common-denominator model nor an SDK DTO clone.

Same-protocol and cross-protocol paths both decode into typed semantics, validate/transform, derive requirements, lower for a fixed target and encode. There is no Native bypass, separate Bridge authority or post-encode semantic JSON mutation. Runtime secrets, selected endpoints and retry/commit state remain outside IR.

## Rationale and consequences

One semantic authority makes transforms and rejection rules independent of the source protocol. A narrower target may reject semantics, but cannot redefine the IR. Predecessor API compatibility is not a requirement; incomplete implementation is not a permanent model limitation.

Structural design: [semantic IR](../semantic-ir.md). Implementation: [Generation](../../../src/semantic/task/generation/mod.rs), [adapters](../../../src/adapter/mod.rs), [lowering](../../../src/lowering/generation.rs). Standard provenance: [fixed sources](../../references/upstream-sync.md).
