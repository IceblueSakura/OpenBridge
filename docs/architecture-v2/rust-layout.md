# Rust Module Boundaries

This page defines dependency and decomposition constraints, not a proposed file inventory. Actual ownership and data flow belong to [current architecture](../architecture.md); semantic concepts belong to the [Agent-first design](semantic-ir.md). Current pre-release paths may change within an authorized slice. Do not mirror SDK directories or pre-create modules for unimplemented tasks.

## Dependency direction

- `semantic` owns task values, validation, requirements and reducers, without protocol/provider/topology/execution dependencies.
- `protocol` maps shared semantics to declared wire profiles. Source records and scoped carriers are not another task authority.
- `lowering` checks immutable final semantics against a supplied representation contract; it does not look up topology, credentials or network targets.
- `adapter` composes protocol rules and lowering, including context projection, without runtime discovery or I/O.
- `topology` binds canonical/public models, fixed Routes and trusted Endpoints. Model support vocabulary comes from semantic contracts, not copied protocol flags.
- `execution` consumes compiled bindings and adapters. Pure candidate/fallback policy stays separate from bounded attempt intake and delivery state.
- `transport` handles prepared trusted HTTP parts and bounded framing, not Task IR.
- `credential` separates manager, authorization drivers and concurrent file storage; runtime access is a short-lived binding, not semantic data.
- `gateway` separates explicit private-file bootstrap, authenticated admission, trusted policy, upstream intake, downstream body ownership and observation. It does not introduce ambient credential fallback or a general plugin framework.

For direct owner links, use [architecture](../architecture.md#模块所有权). Keep these boundaries when splitting modules by responsibility, not line count; preserve intended public paths through explicit re-exports.

## Types to avoid

- Generic `ApiRequest { protocol, Bytes }` as the semantic pipeline carrier.
- Request analyzers that independently reconstruct semantic facts from JSON.
- `Native` / `Bridge` plan enums.
- Provider body hooks accepting arbitrary mutable JSON.
- Universal capabilities mixing semantic, representation and execution flags.
- Raw source envelopes stored inside semantic request/response objects.

## Crate split

Keep one crate while semantic APIs are unstable. Split only when enforceable dependency boundaries or reuse justify the iteration cost; a pure semantic crate is a possible boundary, not a scheduled implementation.
