# Capability and Requirement Model

## Problem

A single capability structure tends to mix four questions:

- can the model perform a semantic operation?
- can a wire protocol represent it?
- can this endpoint execute it?
- does the gateway promise it publicly?

v2 keeps these dimensions separate.

## Semantic contract

A task semantic contract describes meaningful supported behavior independent of wire syntax.

Generation follows the [Agent-first semantic design](semantic-ir.md), using multiple protocol contracts as evidence rather than a common subset or mechanical field union. Semantic expressiveness, local implementation and model support are separately tracked. This includes:

- input resource kinds;
- tool kinds and tool-choice semantics;
- structured output semantics;
- reasoning semantics;
- response completeness, turn progress and continuation dependencies;
- grouping, source-bound replay and state/resource semantics;
- generation controls;
- output modalities.

## Representation contract

Attached to an Endpoint protocol/profile. It answers whether semantic values can be encoded faithfully.

Examples:

- supports explicit serial tool control;
- supports strict function schema;
- can represent refusal separately from text;
- accepts reasoning summary;
- supports same-provider opaque reasoning replay;
- can encode image URL but not inline bytes.

Representation support may include explicit mappings and named conversion policies. It must cover request/history, response and applicable events, including required continuation carriers through downstream delivery and the next request; accepting the first prompt alone is insufficient.

## Execution contract

Operational properties:

- streaming optional/required/unsupported;
- bounded non-streaming conversion;
- state affinity;
- retry eligibility before commit;
- body/event limits;
- credential kind;
- timeout class.

Execution capability must not be used as semantic support.

## Public contract

A Public Model task exposes an intentional downstream contract. It is compiled from trusted configuration/catalog facts and must be satisfiable by its route according to the gateway's route policy.

Public contract is not simply the union of all candidates.

## Requirements

```text
TaskRequirements {
  semantic,
  resources,
  delivery
}
```

Semantic and resource requirements are pure projections of final IR, including admitted extension requirements. Delivery requirements come from the downstream interaction contract, e.g. requested streaming or a WebSocket lane. Typed context/state references may constrain execution affinity without selecting an endpoint. Standard tools, schema dialect and media source variants need value-sensitive checks; a broad `tools` or `media` bit is not full support.

Requirements contain no ProviderId, EndpointId or route choice.

## Candidate check

Each fixed route candidate is evaluated independently:

```text
check(requirements, endpoint_contract)
 -> Representable(conversions)
 | NotRepresentable(reason)
```

Whether an unrepresentable candidate may be skipped is a route policy decision compiled in advance. The request itself cannot reorder candidates.

## Conversion policy

Every lossy or normalizing conversion is named and typed. Examples include:

- OmitSemanticallyInactive
- MapReasoningLevel
- BufferRequiredStreaming
- EncodeEmbeddingFloat32Base64
- DefaultDeepSeekCacheWriteZero (valid usage exists; absent/null detail only; audit provenance outside task semantics)

There is no generic `best_effort=true`.

A conversion must declare its semantic precondition, dependency effects and observable consequence. Semantic representability does not authorize execution, and a continuation is not an implicit retry. Exact derivation of usage must declare accounting scope and prerequisites; unknown counts cannot be repaired to fit a target's arithmetic.
