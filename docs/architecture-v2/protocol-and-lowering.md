# Protocol Codec and Target Lowering

## Two distinct projections

OpenBridge separates protocol translation from endpoint-specific lowering.

### Protocol codec

A codec owns syntax and protocol semantics:

```text
decode(profile, task, wire) -> Decoded<TaskIR>
encode(profile, task, representation) -> wire
```

A codec knows Chat vs Responses shapes, field names, SSE event grammar and protocol-level presence rules. It does not know credentials, routes or provider selection.

### Target lowering

Lowering answers whether a final semantic request can be represented by one fixed endpoint:

```text
lower(final_ir, source_records, endpoint_contract)
  -> TargetRepresentation
  | RepresentationError
```

It owns explicit target mappings such as supported reasoning-level mapping, approved omission of semantically inactive hints, and endpoint-specific representation restrictions.

It cannot mutate the shared final IR.

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

They are keyed to stable semantic identities where applicable.

During lowering, fidelity may be reused only if:

1. the semantic owner still exists;
2. the record is valid for the target profile/provider according to portability;
3. it does not override a modeled field;
4. any content dependency still validates.

Otherwise it is dropped or causes a deterministic representability error according to policy.

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

A provider quirk that changes meaning must be modeled as endpoint lowering or a typed protocol profile, not a body hook.

## Native and cross-protocol

There is no semantic distinction between Native and Bridge.

```text
Chat -> IR -> Chat
Chat -> IR -> Responses
Responses -> IR -> Responses
Responses -> IR -> Chat
```

All four paths use the same authority rules. Same-protocol paths may reuse fidelity records more often, but fidelity is never authority.
