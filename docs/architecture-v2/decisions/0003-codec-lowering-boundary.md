# ADR-v2-0003: Separate Protocol Codec from Endpoint Lowering

## Status

Accepted.

## Decision

Protocol/profile codecs own wire syntax, standard field admission and declared extension mappings. Generation semantics use the Responses-first baseline; endpoint lowering owns representability, explicit mappings and conversion policy for a fixed endpoint. A narrow target cannot redefine the IR's standard expressiveness.

Encoding cannot silently drop unsupported Task IR. Provider code cannot mutate modeled semantic JSON after protocol encoding.

## Consequences

The predecessor `bridge` abstraction and runtime are archived. Same-protocol and cross-protocol paths use identical semantic stages. Any future Provider adaptation that affects modeled values must use typed contracts/lowering or an explicit profile, never a post-encode body hook. Complete responses, request shorthand and SDK-derived views require distinct validation boundaries.
