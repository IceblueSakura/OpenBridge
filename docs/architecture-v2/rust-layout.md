# Proposed Rust Layout

This layout is intentionally allowed to break predecessor crate paths. It describes ownership direction, not a finalized file inventory. Generation follows [Responses-first IR and scoped extensions](semantic-ir.md); typed request context and attachment-specific extensions must have explicit homes before implementation. Do not mirror the SDK source tree or create speculative empty modules.

```text
src/
  semantic/
    mod.rs
    value/
    context.rs
    resource/
    task/
      mod.rs
      generation/
        request.rs
        response.rs
        event.rs
        tool.rs
        reasoning.rs
        requirements.rs
        validate.rs
      embedding/
      image/
      speech/

  adapter/
    mod.rs
    request.rs

  protocol/
    mod.rs
    adaptation.rs
    fidelity.rs
    openai/
      chat/
        request.rs
        response.rs
        event.rs
      responses/
        request.rs
        response.rs
        event.rs

  topology/
    model.rs
    public_contract.rs
    endpoint.rs
    route.rs
    compile.rs

  lowering/
    mod.rs
    generation.rs
    embedding.rs
    image.rs
    speech.rs

  provider/
    mod.rs
    definition.rs
    auth.rs
    errors.rs

  execution/
    plan.rs
    attempt.rs
    lifecycle.rs
    delivery.rs

  gateway/
    config.rs
    admission.rs
    http.rs
    body.rs
    bootstrap.rs
  bin/
    openbridge.rs
  transport/
    http.rs
    sse.rs
  credential/
  observability/
  ingress/
```

## Dependency direction

```text
semantic
   ^
protocol     topology
   ^          ^
   +---- lowering
            ^
         planning
            ^
         execution
            ^
 ingress -> transport/credential
```

More precisely:

- `semantic` depends on no protocol/provider/topology/execution module.
- `protocol` depends on semantic values and protocol-local DTOs.
- Model support vocabulary lives in `semantic::task::generation::contract`; topology references it directly. Endpoint bindings carry explicit adapter/representation contracts, never inferred upstream aliases.
- `lowering` depends on semantic + protocol profile contracts + compiled endpoint contracts.
- `adapter` composes protocol rules and target lowering, and owns context projection; it does not access topology, credentials or network.
- `execution` consumes plans and adapters, owns bounded intake/delivery state and exposes commit acknowledgement to the I/O caller. It does not interpret context fields.
- `transport` is semantically blind.
- `gateway` owns startup entry/credential resolution, authenticated public-task admission and trusted budget policy. `exchange` coordinates the fixed chain, `intake` owns one upstream decode/projection, and `body` owns publication/handoff/cancellation. Library and Gateway reuse one pure candidate selector. Its minimal environment bootstrap is not a general configuration or plugin framework.

## Types to avoid

Do not recreate predecessor concepts under new names:

- generic `ApiRequest { protocol, Bytes }` as the semantic pipeline carrier;
- request analyzers that independently reconstruct semantic facts from JSON;
- `Native` / `Bridge` plan enums;
- provider body hooks accepting arbitrary mutable JSON;
- universal capabilities with protocol, semantic and execution flags mixed together;
- raw source envelopes stored inside semantic request/response objects.

## Crate split

Do not split into multiple crates initially. Module boundaries are sufficient while the semantic API is unstable.

A later split is justified only if dependency enforcement or reuse benefits outweigh iteration cost. A likely future boundary is a pure `openbridge-semantic` crate containing semantic IR, validation and requirement projection.
