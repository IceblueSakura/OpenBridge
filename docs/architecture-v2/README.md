# OpenBridge Semantic Architecture v2

Status: foundation design for the pre-release breaking rewrite.

This directory defines the target architecture for the `semantic-v2` epoch. It is intentionally not constrained by source compatibility with the current implementation. Existing code, ADRs, tests, provider evidence, protocol samples, credential logic, transport constraints, and observability requirements are inputs to the redesign, not architectural authorities.

## Goal

OpenBridge is a semantic gateway for model inference. Protocols and providers are boundary representations; they are not the domain model.

The architectural center is:

```text
downstream wire
  -> protocol decode
  -> task semantic IR
  -> semantic policy + validation
  -> requirements
  -> route planning
  -> target lowering
  -> protocol encode
  -> execution

upstream wire/event
  -> protocol decode
  -> task response/event IR
  -> validation / semantic processing
  -> target lowering
  -> downstream protocol encode
  -> delivery
```

## Documents

- [domain-model.md](domain-model.md): ontology and ownership of Model, Task, Protocol, Provider, Endpoint, Public Model, Route, Capability and IR.
- [invariants.md](invariants.md): architectural invariants that implementation must preserve.
- [migration.md](migration.md): rewrite strategy and classification of legacy assets.
- [decisions/0001-semantic-core.md](decisions/0001-semantic-core.md): first v2 architecture decision.

## Epoch rule

The existing `docs/decisions/0001-0005` remain valuable design history. For the v2 rewrite they are treated as predecessor decisions. If a v2 decision conflicts with a predecessor, v2 owns the target architecture on `semantic-v2`.

Do not preserve a legacy abstraction solely to reduce migration work. Preserve observable requirements and proven invariants; redesign their representation when the semantic model calls for it.
