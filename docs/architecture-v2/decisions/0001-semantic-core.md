# ADR-v2-0001: Responses-first Semantic Authority

## Status

Accepted design direction. Current implementation remains a partial offline Generation library, not a complete Responses gateway.

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

The predecessor runtime is [archived](../../archive.md). No compatibility wrapper or duplicate legacy path is required. Current implementation gaps are tracked in [migration.md](../migration.md), not converted into permanent model limitations. Internal Rust compatibility is not an objective.
