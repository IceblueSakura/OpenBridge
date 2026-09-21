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

Only Uncommitted requests may retry or advance to another candidate.

For streaming, a bounded precommit stage may validate enough upstream semantics to safely emit the first downstream event. After the first visible semantic event, fallback is forbidden.

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
