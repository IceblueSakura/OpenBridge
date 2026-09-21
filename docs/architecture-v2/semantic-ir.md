# Semantic IR Model

## Purpose

The semantic IR is the internal language of OpenBridge. It represents supported inference meaning independently of downstream protocol, upstream protocol, provider and transport.

IR is not a normalized JSON document and is not a compatibility DTO.

## Layering

```text
semantic/
  value/       protocol-neutral bounded values
  resource/    media/resource identity and payload references
  task/        closed task family
    generation/
    embedding/
    image/
    speech/
  requirements/ pure projections from validated task IR
```

The outer task union is tagged:

```text
TaskRequest =
  Generation(GenerationRequest)
  Embedding(EmbeddingRequest)
  ImageGeneration(ImageGenerationRequest)
  SpeechRecognition(SpeechRecognitionRequest)
  SpeechSynthesis(SpeechSynthesisRequest)
  VoiceDesign(VoiceDesignRequest)
  VoiceClone(VoiceCloneRequest)
```

Responses follow the same task boundary. Streaming is a property of tasks that define event semantics, not a second task family.

## Generation aggregate

Generation is modeled as an ordered semantic graph rather than protocol messages.

```text
GenerationRequest
  identity-space
  instructions[]
  conversation[]
    Message
    ToolCall
    ToolResult
    ReasoningReplay
  tools[]
  tool-policy
  output-contract
  reasoning-policy
  generation-controls
  state-intent
  extensions[]
```

### Identity

Semantic identity is explicit and scoped. Array index is never identity.

Recommended opaque local newtypes:

```text
ItemId
PartId
ToolCallId
ResourceId
AnnotationId
CandidateId
```

IDs are stable across transformations. A transform may preserve, remove or create identity; it may not silently reassign identity based on new array positions.

### Presence

IR must distinguish presence where it changes meaning:

```text
Presence<T> = Absent | Present(T)
Nullable<T> = Null | Value(T)
```

Do not globally model every field as `Option<T>`. Use semantic enums when omission, explicit false, empty, null and default have different contracts.

### Extensions

Extensions are typed, bounded and namespaced:

```text
Extension {
  namespace,
  kind,
  payload,
  portability
}
```

Portability is explicit:

- SemanticPortable
- SameProfileOnly
- SameProviderOnly
- NonReplayable

An extension cannot override a modeled semantic field.

## Validation phases

1. Decode validation: wire is structurally valid for the declared protocol/profile and task.
2. Semantic validation: IR invariants hold independent of any endpoint.
3. Policy transformation: trusted gateway policies may return a new IR.
4. Revalidation: all semantic invariants are checked again.
5. Requirement projection: pure function of final IR plus delivery intent.
6. Candidate representability: endpoint can faithfully lower the final IR.

Validation never selects a provider.

## Transformation contract

Semantic transforms are pure:

```text
transform(TaskRequest) -> Result<TaskRequest, SemanticError>
```

They receive no credentials, network client, route or mutable source JSON.

When content changes, dependent annotations/source records are either preserved by stable identity and revalidated or discarded. They are never repaired by positional guessing.

## Response and event IR

A task response describes completed semantic output. Event IR describes incremental facts and transitions.

Events are not raw SSE wrappers. Transport framing is outside IR.

Generation event examples:

```text
GenerationStarted
ItemStarted(ItemId, ...)
PartDelta(ItemId, PartId, ...)
ToolArgumentsDelta(ToolCallId, ...)
UsageUpdated(...)
ItemCompleted(ItemId)
GenerationCompleted(...)
GenerationFailed(...)
```

A reducer consumes valid events into a task response state. Protocol event codecs must emit/consume typed events. Source SSE payload may be retained only as bounded fidelity metadata.

## Requirements

Requirements are derived values, never independently parsed request facts:

```text
requirements = derive(final_ir, delivery_intent)
```

They may describe semantic needs, resource budgets and delivery needs, but contain no candidate choice.

Changing IR and re-deriving requirements must be sufficient to change planning behavior.
