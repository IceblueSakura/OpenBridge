# Domain Model

## Dimensions

OpenBridge separates concepts that were historically coupled by wire shape or routing implementation.

### Model

A Model is canonical model identity plus model-level facts. It does not identify a provider endpoint, credential, route, or protocol.

A model may offer one or more Tasks.

### Task

A Task defines the semantic contract of one inference activity.

Initial task family:

- Generation
- Embedding
- ImageGeneration
- SpeechRecognition
- SpeechSynthesis
- VoiceDesign
- VoiceClone

Task identity is determined from the trusted public contract before semantic decoding. It is never guessed from arbitrary request content.

### Task IR

Each Task owns request, response, and where applicable event semantics. Generation follows the OpenAI Responses standard semantic surface, not the intersection of supported wire protocols; other tasks retain their own contracts.

The overall internal representation includes task semantics, typed request/response context, delivery intent, scoped extensions and fidelity as defined in [semantic-ir.md](semantic-ir.md). Task content does not own sockets, selected routes/endpoints, credentials, retry state or downstream commit state. Context extensions may own session/thread/turn facts without becoming runtime handles.

Shared value types are allowed only where they preserve task invariants: bounded text, resources, media descriptors, stable identities, schema values, usage and extensions.

There is no universal request struct with optional fields for every task.

### Protocol

A Protocol defines a wire language and its codec/profile rules. Examples include OpenAI Chat Completions and OpenAI Responses.

Protocol codecs translate between a known Task contract and Task IR. A protocol is not a provider and does not select a route.

### Provider

A Provider is a trusted upstream service implementation/domain. It owns service-specific authentication, error classification, trusted origins and provider-specific representation constraints.

A selected Provider/endpoint or credential must not become task semantics. A typed extension namespace or trusted opaque origin may identify provenance and constrain representability; it cannot choose or authorize an upstream target.

### Endpoint

An Endpoint is an executable upstream realization:

```text
Endpoint =
    Provider
  + trusted target
  + Protocol/Profile
  + Model Task realization
  + representation/execution constraints
```

An endpoint is the concrete unit to which a route candidate can bind.

### Public Model

A Public Model is the downstream gateway contract exposed to clients. It names the tasks and semantic contract accepted by OpenBridge and references a fixed route plan.

It is not an alias for an upstream model name.

### Route

A Route is an ordered, trusted mapping from a Public Model task contract to executable Endpoint candidates.

The request cannot add, reorder, or expand route candidates.

### Capability

Capability is not one undifferentiated bitset. v2 separates:

1. Semantic capability: what the task/model can mean or produce.
2. Representation capability: what a protocol/profile/endpoint can encode.
3. Execution capability: operational properties such as streaming policy or state affinity.
4. Public contract capability: what OpenBridge promises downstream.

Requirements are derived from final Task IR and delivery requirements. They are checked against the public contract and then against each fixed candidate's representability.

## Core relation

```text
Canonical Model
    |
    +-- offers --> Task semantic contract

Public Model
    |
    +-- exposes --> Task contract
    |
    +-- uses --> Route
                  |
                  +-- candidate --> Endpoint
                                      |
                                      +-- Provider
                                      +-- Protocol/Profile
                                      +-- upstream Model/Task realization
                                      +-- representation constraints
```

## Ownership rule

For every meaningful datum, the architecture must be able to answer exactly one of:

- standard task semantics own it;
- typed request/response context or an owner-bound extension owns it;
- source/fidelity metadata owns its representation-only preservation;
- target lowering owns target-specific representation;
- execution owns runtime resources and operational state.

No datum may have two independent authorities.
