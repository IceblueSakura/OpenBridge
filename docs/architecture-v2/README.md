# OpenBridge Semantic Architecture v2

Status: target architecture for the pre-release breaking rewrite.

This directory defines the architectural authority for the `semantic-v2` epoch. It is intentionally not constrained by internal source compatibility with the predecessor implementation. Existing code, ADRs, tests, provider evidence, protocol samples, credential logic, transport constraints and observability requirements are design inputs, not module-layout authorities.

## Architectural thesis

OpenBridge is a semantic inference gateway. Protocols and providers are boundary representations; they are not the domain model.

```text
downstream wire
  -> protocol decode
  -> Task IR
  -> semantic validation / trusted transform / revalidation
  -> requirements
  -> public-contract check + fixed route
  -> candidate lowering
  -> target protocol encode
  -> execution

upstream wire/event
  -> selected endpoint protocol decode
  -> Response/Event IR
  -> semantic validation
  -> downstream lowering
  -> downstream protocol encode
  -> delivery
```

There is no privileged Native path and no Bridge domain object. Same-protocol and cross-protocol requests pass through the same semantic authority.

## Core design documents

- [domain-model.md](domain-model.md) — ontology: Model, Task, IR, Protocol, Provider, Endpoint, Public Model, Route and Capability.
- [semantic-ir.md](semantic-ir.md) — task type family, identity, presence, extensions, validation, response/event semantics and requirements.
- [protocol-and-lowering.md](protocol-and-lowering.md) — codec, fidelity records, endpoint lowering and provider boundary.
- [capability-model.md](capability-model.md) — semantic, representation, execution and public contracts.
- [execution-model.md](execution-model.md) — immutable topology, planning, attempts, commit and response symmetry.
- [rust-layout.md](rust-layout.md) — proposed breaking Rust module layout and dependency direction.
- [invariants.md](invariants.md) — non-negotiable architecture gates.
- [migration.md](migration.md) — repository-preserving rewrite strategy and acceptance gates.

## v2 decisions

- [ADR-v2-0001](decisions/0001-semantic-core.md) — Semantic Core is the architectural center.
- [ADR-v2-0002](decisions/0002-task-ir-and-identities.md) — task-specific IR, stable identity and semantic presence.
- [ADR-v2-0003](decisions/0003-codec-lowering-boundary.md) — protocol codec and endpoint lowering are separate.
- [ADR-v2-0004](decisions/0004-capability-separation.md) — capability dimensions are separate.
- [ADR-v2-0005](decisions/0005-execution-lifecycle.md) — execution is semantically blind after encoding.

## Epoch rule

The predecessor `docs/decisions/0001-0005` remain design history and evidence of earlier reasoning. v2 decisions own the target architecture on `semantic-v2` when the two epochs conflict.

Do not preserve a legacy abstraction solely to reduce migration work. Preserve observable requirements and proven invariants; redesign their representation when the semantic model calls for it.

## Immediate implementation gate

Before reconnecting real providers, implement one offline vertical slice:

```text
Chat fixture
 -> Chat codec
 -> GenerationRequest
 -> semantic transform
 -> requirements
 -> Responses lowering/codec
 -> expected Responses fixture
```

and independently:

```text
same GenerationRequest
 -> Chat lowering/codec
 -> expected Chat fixture
```

The slice passes only if changing/deleting IR semantics changes/deletes the encoded result and source fidelity cannot resurrect them.
