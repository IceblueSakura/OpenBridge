# ADR-v2-0001: Agent-first, Protocol-neutral Semantic Authority

## Status

Accepted design direction. Current codec admission and implementation gaps remain separately documented.

## Decision

Generation has one Agent-first, protocol-neutral semantic authority for ordered content, actions/results, control transfer and continuation dependencies. OpenAI Responses, Google Gemini and Anthropic Messages are design references and projection targets; none defines the IR's expressiveness ceiling.

Keep ordered heterogeneous items, stable identities and explicit lifecycles where they express independent concepts. Do not clone a protocol union, SDK DTO, common denominator or mechanical field union. Shared concepts have one owner; scoped typed extensions preserve genuinely source-specific meaning without creating a Provider IR.

Same-protocol and cross-protocol paths decode, validate/transform, derive requirements, lower for a fixed target and encode. There is no Native bypass or post-encode semantic JSON mutation. Representability, executable capability and public admission remain separate.

## Rationale and consequences

OpenBridge is pre-release and in design exploration. Stability belongs to concepts, ownership and invariants, not current Rust shapes. New designs are evaluated against independent protocol contracts and Agent continuation semantics rather than incumbent implementation convenience.

A narrow target must reject or explicitly convert unsupported meaning, not redefine the IR. Protocol neutrality does not erase existing Responses semantics or promise arbitrary lossless translation. Other tasks keep their own contracts; Agent-first representation does not authorize orchestration, storage or tools. Runtime secrets, selected targets and retry/commit state stay outside IR.

Detailed design owner: [semantic IR](../semantic-ir.md). Current implementation: [Generation](../../../src/semantic/task/generation/mod.rs), [adapters](../../../src/adapter/mod.rs); [gaps](../../implementation-status/generation.md). Sources: [reference index](../../references/README.md). Design changes do not silently change existing wire profiles.
