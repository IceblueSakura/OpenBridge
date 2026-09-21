# ADR-v2-0001: Semantic Core Is the Architectural Center

## Status

Accepted for the `semantic-v2` rewrite epoch.

## Context

The predecessor architecture introduced a Generation IR while production behavior still evolved through wire analysis, JSON normalization, Native/Bridge distinctions, provider hooks and source-envelope preservation. This was a rational migration path, but it makes the implementation order visible in the architecture.

The project is pre-release and can accept breaking changes. Internal API continuity is therefore not a design objective.

## Decision

OpenBridge v2 is organized around typed task semantics.

The canonical processing model is:

```text
Wire -> Codec -> Task IR -> Policy/Validation -> Requirements
     -> Fixed Route -> Candidate Lowering -> Codec -> Wire -> Execution
```

The response direction follows the inverse semantic boundary.

The following consequences are intentional:

- `bridge` is not a domain concept. Cross-protocol conversion is composition of decode, semantic representation, lowering and encode.
- `native` does not bypass semantic processing.
- wire-fact analysis may exist for bounded admission, but it cannot become a parallel semantic model.
- requirements are projections of final IR plus delivery constraints, not independently accumulated interpretations of the original body.
- provider-specific code may define endpoint contracts and representation mapping but cannot own generic task semantics.
- internal Rust compatibility with predecessor modules is explicitly not required.

## Consequences

The rewrite will initially duplicate some implementation while legacy production paths remain present. This is preferable to forcing new semantics through predecessor abstractions.

Migration is complete when the legacy semantic path can be deleted rather than wrapped.

Existing evidence and tests remain authoritative evidence of observed behavior, but predecessor module boundaries and type names do not constrain v2.
