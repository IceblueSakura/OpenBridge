# Proposed Rust Layout

This layout is intentionally allowed to break predecessor crate paths. It describes ownership direction, not a finalized file inventory. Generation follows [Responses-first IR and scoped extensions](semantic-ir.md); typed request context and attachment-specific extensions must have explicit homes before implementation. Do not mirror the SDK source tree or create speculative empty modules.

```text
src/
  semantic/
    mod.rs
    value/
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

  protocol/
    mod.rs
    profile.rs
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
    response.rs

  transport/
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
- `topology` depends on semantic task/capability vocabulary but not protocol implementation internals.
- `lowering` depends on semantic + protocol profile contracts + compiled endpoint contracts.
- `execution` consumes plans/encoded candidates and owns lifecycle.
- `transport` is semantically blind.

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
