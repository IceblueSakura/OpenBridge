# Protocol Codec and Target Lowering

## Two distinct projections

OpenBridge separates protocol translation from endpoint-specific lowering. [Generation IR](semantic-ir.md) is Agent-first and protocol-neutral, with typed domains and scoped extensions; it is neither a wire DTO, a field union nor a lowest common denominator. The [source index](../references/README.md) provides protocol references; the [fixed OpenAI/Codex baseline](../references/upstream-sync.md) distinguishes their public standard, SDK consumer and product profile, not a universal IR schema.

### Protocol codec

A codec owns syntax and protocol semantics:

```text
decode(profile, task, wire) -> Decoded<TaskIR>
encode(profile, task, representation) -> wire
```

A codec knows wire shapes, field names, event grammar and profile-level presence rules. `adapter::Adapter` composes a protocol family with trusted `Adaptation` rules and scope; Standard, OpenBridge-client and the fixed Provider profiles (selected in [`src/adapter/mod.rs`](../../src/adapter/mod.rs)) are distinct contracts, not one global vendor superset. Shared concepts have typed owners; extensions require a declared schema, attachment and scope. Wire containers do not determine semantic identity: a tool result remains a result even inside a user message. Known unsupported protocol branches must not become arbitrary JSON passthrough. Existing codec admission remains governed by its own profile. A codec does not access credentials, routes or provider selection.

Raw Responses JSON enters through `envelope::decode_request_bytes` / `decode_response_bytes`; SSE data uses the same bounded, duplicate-rejecting parser before event decoding. Existing Value APIs validate envelope/task semantics, not the lost raw JSON representation. Caller-side body collection remains independently bounded. Exact admission and limits are in the [text profile](responses-text-profile.md#raw-json-admission).

The single-candidate Chat boundary is `chat_envelope::decode_request_bytes` / `decode_response_bytes` with `chat_sse::ChatSseDecoder` / `ChatSseEncoder`. It shares JSON parsing, SSE framing and Generation event/reducer machinery, but has its own delivery options and `[DONE]` terminal contract. See [Chat admission](chat-text-profile.md).

### Target lowering

Lowering answers whether a final semantic request can be represented by one fixed endpoint:

```text
lower(final_ir, source_records, endpoint_contract)
  -> TargetRepresentation
  | RepresentationError
```

It owns explicit target mappings such as supported reasoning-level mapping, approved omission of semantically inactive hints, and endpoint-specific representation restrictions.

It cannot mutate the shared final IR. Representability covers ownership/grouping, continuation and source-bound dependencies as well as field shapes. A target that accepts a first request but cannot deliver the required replay carrier through the client and back into history is not a closed Agent mapping. `adapter::Request` carries shared task semantics, `semantic::context` hints/delivery and separate source records; target context projection lives in the adapter rather than execution. Context/extension mappings use the same rule: a candidate may project an admitted session fact or source-bound resource only where its profile permits, not create a second semantic authority. Pure codecs must validate complete envelopes separately from permissive input abbreviations and low-level snapshots.

## Why encode does not consume raw IR blindly

A protocol may have multiple profiles and an endpoint may expose only a subset. Therefore target lowering first proves representability and produces a typed representation accepted by the codec.

```text
Task IR
   |
   +-- EndpointContract
   v
TargetRepresentation
   |
   +-- ProtocolProfile
   v
Wire
```

This prevents the codec from silently dropping unsupported semantics.

## Source records

Decode returns:

```text
Decoded<T> {
  semantic: T,
  fidelity: FidelityRecords,
  delivery: SourceDeliveryFacts
}
```

Fidelity records may preserve unknown fields, exact spelling/form choices or opaque same-origin values only when bounded and classified.

They are keyed to stable semantic identities where applicable. Response-wide classified extras also bind protocol, adapter contract, trusted scope and a digest of the final typed response. Partial extras are validated but not captured; final extras project only at a compatible terminal and invalidate after semantic edits. Normalization audit records identify intake defaults without restoring or overriding semantic values.

During lowering, fidelity may be reused only if:

1. the semantic owner still exists;
2. the record is valid for the target profile/provider according to portability;
3. it does not override a modeled field;
4. any content dependency still validates.

Otherwise it is dropped or causes a deterministic representability error according to policy. Classified nested billing/router facts use this same lifecycle and dependency boundary without changing typed Usage; their current allow-list and mapping live in [`adapter_shapes.rs`](../../src/protocol/openai/adapter_shapes.rs).

The [replay design](decisions/0006-reasoning-ownership.md) separates typed opaque values from their source/dependency proofs; current reasoning-only fidelity does not implement all attachment types. Known behavior, cache policy and control transfer need typed owners, not classified-extra shortcuts.

For admitted Responses encrypted reasoning, static decode returns unbound replay records. A trusted caller binds their source scope with `FidelityRecords::bind_replay_origin`; event decoding receives the same scope through `EventDecoder::with_replay_origin`. `GenerationRepresentationContract::replay_origin` identifies the fixed target's compatible scope. Missing or mismatched scope, partial replay, or changed reasoning-owner dependencies fail lowering. These labels must not be supplied by business JSON or contain credential/endpoint locators. Scope construction is an execution-boundary responsibility, not provider discovery inside a codec.

Event codecs use `src/lowering/events.rs` for incremental representability and the same semantic reducer for lifecycle validation. `EventEncoder` checks a fixed target contract before rendering each event and validates the complete static projection at the terminal. A semantic event has authority over its final replay value; passed source fidelity supplies wire identities, not an alternative token. Chat output returns payloads only; framing owns `[DONE]`. A Chat decoder requires `done()` after finish and optional usage, and `finish()` rejects EOF without that terminal.

## Provider boundary

Provider code may contribute:

- trusted origin and relative paths;
- authentication binding;
- fixed/safe headers;
- status/error classification;
- endpoint contracts;
- provider-specific protocol profile declarations.

Provider code may not perform arbitrary semantic JSON mutation after encoding.

A provider quirk that changes meaning must be modeled as endpoint lowering or an explicit adapter rule, not a body hook. Mapping, validated derived views, scoped fidelity and field-specific compatibility defaults are defined in [ADR 0008](decisions/0008-stable-core-and-vendor-adapters.md). DeepSeek may default an unreported cache-write detail to zero in otherwise valid usage; that does not invent whole usage, mask malformed input or claim measured billing. Client and Provider adapters share these boundaries.

## Native and cross-protocol

There is no semantic distinction between Native and Bridge. The currently implemented protocol pairs illustrate the rule, not its architectural limit:

```text
Chat -> IR -> Chat
Chat -> IR -> Responses
Responses -> IR -> Responses
Responses -> IR -> Chat
```

All four paths use the same authority rules. Same-protocol paths may reuse fidelity records more often, but fidelity is never authority.
