# Execution Model

## Principle

Execution consumes a compiled plan. It does not interpret model semantics.

```text
Validated Task IR
  -> Requirements
  -> Public Contract Check
  -> Fixed Route
  -> Candidate Lowering
  -> Encoded Candidate
  -> Credential Binding
  -> Transport Attempt
  -> Response Decode
  -> Delivery
```

## Compiled topology

Startup compilation produces an immutable topology:

```text
PublicContract
  task contracts
  route ids

Route
  ordered EndpointId[]

Endpoint
  ProviderId
  trusted target
  TaskKind
  ProtocolProfile
  upstream model binding
  RepresentationContract
  ExecutionContract
  CredentialBindingId
```

The runtime request cannot create or modify these relations.

## Plan types

Planning should produce immutable data rather than executable closures.

```text
ExecutionPlan {
  task,
  delivery,
  candidates: [CandidatePlan]
}

CandidatePlan {
  endpoint_id,
  representation_contract,
  execution_contract
}
```

The semantic request itself remains shared and immutable. Candidate-specific encoded bytes are created lazily per attempt from that request.

## Attempt lifecycle

For each fixed candidate:

1. lower immutable final IR to the endpoint representation;
2. encode it using the endpoint protocol profile;
3. bind upstream model identity and trusted execution metadata;
4. acquire the endpoint's credential binding;
5. send through transport;
6. classify HTTP/framing failures;
7. decode response with the same endpoint task/profile;
8. validate response/event IR;
9. lower to the downstream protocol/profile;
10. commit only valid downstream semantic output.

Retry/fallback cannot change semantic IR or expand/reorder the route.

## Commit boundary

Execution tracks a monotonic delivery state:

```text
Uncommitted -> Committed -> Terminal
```

Only Uncommitted requests may advance under an explicit bounded Route policy; the current gateway never repeats a candidate. First downstream-frame publication additionally freezes advancement conservatively before the HTTP response escapes, closing the receive/timeout race without claiming delivery commit. See [ADR 0010](decisions/0010-canonical-model-fixed-fallback.md).

For streaming, `Attempt::push` consumes at most one upstream frame and returns semantic events without retaining a replay log. `ResponseDelivery::encode_events` projects these incrementally. Only the terminal is withheld until `Attempt::finish` validates strict EOF; `finish_stream` then emits it. The I/O caller retains unconsumed suffixes and controls readiness/backpressure.

Encoding bytes does not commit. The I/O owner calls `ResponseDelivery::commit` at the external visibility boundary and `complete` only after upstream closure and final downstream delivery. After commit, fallback is forbidden. Late errors poison the chain; cancellation releases owned decoder/encoder state. `StrictComplete` may reject at terminal after partial delivery; complete-result consumers must choose bounded non-streaming delivery if they require pre-delivery completeness.

## Credential and transport isolation

Credentials are referenced by opaque binding identity in compiled endpoint topology but secret material is acquired only at execution.

Transport receives:

- trusted target;
- relative path;
- method;
- safe headers;
- sensitive auth headers;
- encoded bounded body or streaming body;
- timeout/resource policy.

Transport never receives Task IR and cannot make semantic decisions.

## Response symmetry

Response processing uses the selected endpoint contract; it does not infer provider/task from response body.

```text
upstream bytes/events
 -> endpoint protocol codec
 -> Response/Event IR
 -> semantic validation
 -> downstream representability
 -> downstream codec
 -> delivery
```

This symmetry is required even when upstream and downstream protocols are identical.
