# OpenBridge Semantic Architecture v2

Status: **architecture baseline frozen; Generation semantic migration and validation in progress.**

This directory defines the architectural authority for the `semantic-v2` epoch. Internal source compatibility with the predecessor implementation is not a goal. Existing code, ADRs, tests, provider evidence and protocol samples are migration evidence, not module-layout authorities.

## Current phase

The current workspace is the **v2 semantic library and offline validation suite**. The predecessor runtime and its dedicated tests/assets are [archived](../archive.md); there is no runnable gateway binary. Breaking retirement was explicitly accepted before feature parity and does not complete semantic migration.

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
4. **M4 — Archived responsibility assessment:** map archived Bridge/Pipeline/Generation-IR behavior to v2 owners, intentional omissions and remaining gaps. The old runtime is already retired; removal does not prove parity.

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
- [responses-text-profile.md](responses-text-profile.md) — offline Responses text admission and ownership map.

## v2 decisions

ADR-v2-0001 through ADR-v2-0005 remain the accepted baseline. [ADR-v2-0006](decisions/0006-reasoning-ownership.md) owns reasoning separation and its admitted Responses subset. Do not add another ADR merely to describe implementation progress.

## Current acceptance principle

The migration is not validated by JSON round-trip alone. Tests must prove:

- semantically equivalent Chat and Responses inputs converge;
- changing final IR changes every applicable target encoding;
- deleting semantic IR cannot be undone by fidelity/source records;
- unsupported target semantics fail before encoding rather than being silently dropped;
- requirements are derived from final IR rather than independently reconstructed from source wire.

The function-tool request, completed static response, function-call events, assistant-text events, and the admitted reasoning subset now have v2 codecs and independent conformance cases. Scope and unresolved semantic gates are recorded in [migration.md](migration.md#phase-gates). Media remains outside this slice. Structured output is under pure-text Responses acceptance, not production migration; this does not complete M1-M4.
