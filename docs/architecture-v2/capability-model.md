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

For Generation this may include:

- input resource kinds;
- tool kinds and tool-choice semantics;
- structured output semantics;
- reasoning semantics;
- state semantics;
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

Representation support may include explicit mappings and named conversion policies.

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

Semantic and resource requirements are pure projections of final IR. Delivery requirements come from the downstream interaction contract, e.g. requested streaming.

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

There is no generic `best_effort=true`.

A conversion must declare its semantic precondition and observable consequence.
