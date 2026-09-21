# ADR-v2-0003: Separate Protocol Codec from Endpoint Lowering

## Status

Accepted.

## Decision

Protocol codecs own wire syntax and protocol semantics. Endpoint lowering owns representability, explicit mappings and conversion policy for a fixed endpoint.

Encoding cannot silently drop unsupported Task IR. Provider code cannot mutate modeled semantic JSON after protocol encoding.

## Consequences

The predecessor `bridge` abstraction is superseded. Same-protocol and cross-protocol paths use identical semantic stages. Provider request-body hooks that affect modeled semantics must migrate into typed endpoint contracts/lowering or protocol profiles before the legacy path is removed.
