# OpenBridge Semantic Architecture v2

Status: **architecture baseline frozen; Generation semantic migration and validation in progress.**

This directory defines the architectural authority for the `semantic-v2` epoch. Internal source compatibility with the predecessor implementation is not a goal. Existing code, ADRs, tests, provider evidence and protocol samples are migration evidence, not module-layout authorities.

## Current phase

The project has moved from architecture exploration to **staged code migration and validation**.

The architectural baseline is now sufficiently defined. New architecture documents or ADRs are added only when implementation exposes a real semantic ownership problem that the current model cannot resolve.

The active scope is deliberately narrow:

> Migrate Generation semantics from the predecessor implementation into the v2 Semantic Core and prove the migration with offline semantic conformance tests.

Until this phase passes its acceptance gates, do not expand work into Embedding, Image Generation, Speech, provider routing, credential redesign, execution rewrite, MCP, observability redesign, or a general hook/plugin system.

## Frozen processing model

```text
Wire
 -> Protocol Codec
 -> Task IR
 -> Semantic Validation / Trusted Transform
 -> Requirements
 -> Representability / Lowering
 -> Protocol Codec
 -> Wire
```

Provider, route, credential and HTTP execution are intentionally outside the current migration slice.

There is no privileged Native path and no Bridge domain object. Same-protocol and cross-protocol requests pass through the same semantic authority.

## Current migration milestones

1. **M1 — Generation IR:** migrate the supported Generation semantic domains without copying predecessor module boundaries.
2. **M2 — Chat / Responses codecs:** establish complete supported `Chat <-> IR <-> Responses` mappings.
3. **M3 — Semantic conformance:** prove convergence, IR authority, fidelity isolation and deterministic representability failures.
4. **M4 — Legacy replacement assessment:** map predecessor Bridge/Pipeline/Generation-IR responsibilities to v2 and identify code that can be deleted or migrated.

Only after M1-M4 pass should topology, endpoint and execution migration begin.

## Core design documents

- [domain-model.md](domain-model.md) — ontology.
- [semantic-ir.md](semantic-ir.md) — task IR and semantic ownership.
- [protocol-and-lowering.md](protocol-and-lowering.md) — codec, fidelity and lowering boundaries.
- [capability-model.md](capability-model.md) — capability dimensions.
- [execution-model.md](execution-model.md) — later execution target model; not current implementation scope.
- [rust-layout.md](rust-layout.md) — target module direction.
- [invariants.md](invariants.md) — architecture gates.
- [migration.md](migration.md) — active staged migration plan.

## v2 decisions

ADR-v2-0001 through ADR-v2-0005 remain the accepted baseline. Do not add another ADR merely to describe implementation progress.

## Current acceptance principle

The migration is not validated by JSON round-trip alone. Tests must prove:

- semantically equivalent Chat and Responses inputs converge;
- changing final IR changes every applicable target encoding;
- deleting semantic IR cannot be undone by fidelity/source records;
- unsupported target semantics fail before encoding rather than being silently dropped;
- requirements are derived from final IR rather than independently reconstructed from source wire.

The function-tool request and completed static-response subset now has v2 codecs, validated lowering inputs and independent conformance cases. Scope and unresolved replacement gates are recorded in [migration.md](migration.md#function-tool-slice-implementation-boundary). The next slice is tool Event IR and static/event closure; this does not complete M1-M4.
