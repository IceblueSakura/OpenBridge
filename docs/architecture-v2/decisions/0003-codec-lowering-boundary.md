# ADR-v2-0003: Separate Protocol Codec from Endpoint Lowering

## Status

Accepted.

## Decision

Protocol/profile codecs own wire syntax, structural validation and declared extension mappings. Lowering owns representability and explicit conversion policy for a fixed target. Neither performs registry lookup, credential resolution or network I/O.

Unsupported task semantics must fail explicitly rather than disappear during encoding. Provider adaptation uses typed contracts or explicit profiles, never a post-encode body hook.

## Rationale and consequences

Separating syntax from target restrictions prevents a narrow Endpoint from redefining standard semantics. Same-protocol and cross-protocol paths share the same stages. Request shorthand, complete responses and SDK-derived views need distinct validation boundaries.

Owners: [protocol](../../../src/protocol/mod.rs), [lowering](../../../src/lowering/generation.rs), [adapters](../../../src/adapter/mod.rs). Cross-module contract: [protocol and lowering](../protocol-and-lowering.md).
