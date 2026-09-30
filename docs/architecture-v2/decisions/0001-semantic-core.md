# ADR-v2-0001: Responses-first Semantic Authority

## Status

Accepted for the current OpenBridge mainline. Implementation scope and remaining gaps belong to [Generation capabilities and boundaries](../../implementation-status/generation.md); this decision does not establish full protocol compatibility or production readiness.

## Decision

Generation IR uses the fixed OpenAI Responses standard as its principal semantic reference, with scoped extensions for capabilities outside that standard. It is not a Chat/common-denominator model and is not a raw SDK DTO or JSON clone.

The authority boundary is:

```text
Wire + trusted admission context
 -> Codec -> typed semantics/context/extensions
 -> Validation / Trusted Transform -> Requirements
 -> Fixed Candidate Lowering -> Codec -> Wire
```

Same-protocol encoding has no Native bypass. Cross-protocol conversion is composition through the same final IR, not a separate Bridge object. Chat representability cannot reduce the Responses semantic model.

The authoritative structural contract is [semantic-ir.md](../semantic-ir.md); fixed sources and evidence limits are in [upstream-sync.md](../../references/upstream-sync.md). Provider code cannot perform semantic JSON mutation after encoding. Runtime secrets, endpoints, retry and commit state remain outside IR.

## Consequences

The predecessor runtime is [archived](../../archive.md). No compatibility wrapper or duplicate legacy path is required. Current implementation gaps are tracked in [capabilities and boundaries](../../implementation-status/generation.md), not converted into permanent model limitations or a predecessor feature-parity checklist. Internal Rust compatibility with the predecessor is not an objective.
