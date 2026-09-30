# ADR-v2-0008: One Semantic Core and Explicit Boundary Adapters

## Status

Accepted for the current unpublished OpenBridge semantic core. Internal APIs may be replaced without compatibility aliases. This decision supersedes the unconditional prohibition on compatibility defaults and the treatment of all vendor shapes as one global Chat/Responses profile. It does not authorize production deployment or Provider calls.

## Decision

### One semantic authority

Generation remains Responses-first, not a lowest-common-denominator model. Task semantics, standard context, delivery intent, reported response facts and typed extensions have explicit owners. Provider/client identity, credentials, sockets and retry/commit state never become task semantics. A new spelling changes an adapter; a genuinely missing capability may extend the shared semantic model. There are no provider-specific Usage or Generation variants.

### Protocol families and adapters

Chat and Responses supply shared syntax, structural validation and event grammar. Trusted adapter profiles separately declare standard, OpenBridge-client and fixed Provider wire rules; current selections live in [`Adapter`](../../../src/adapter/mod.rs) and reuse the same boundary mechanisms, rather than a documented Provider inventory. Business JSON cannot choose a dialect, origin or target. Client adapters decode requests and encode responses/events; Provider adapters encode requests and decode responses/events. Both reuse protocol implementations, without generic body hooks or a dynamic plugin framework.

A target projection reads immutable final semantics and context, checks representability and produces wire through the selected adapter. Execution binds targets/authentication and coordinates I/O; it does not own field-by-field context conversion. A same-protocol path uses the same semantic authority as a cross-protocol path.

### Explicit normalization, not invented observations

Adapters may map aliases, validate and discard derived views, exactly derive values, or supply a field-specific compatibility default. These rules are part of the public compatibility contract, not claims that the Provider reported the resulting values.

For the named cache-write default, a valid usage object with an absent/null detail may normalize that detail to `0`. Preserve any reported integer, including nonzero values; reject malformed/negative/overflowing values rather than masking them. Missing/null usage stays absent, and JSON/SSE must apply the same rule. This does not infer physical cache activity, price or billing. The exact profile selection belongs to `Adapter`; the field-specific contract belongs to `WireRules` and the usage codec, not a duplicated Provider table in this ADR.

Bounded normalization records identify compatibility-derived values outside task semantics. They do not restore deleted values or override transformations. Encoding consumes final semantic values; it does not repeatedly apply intake defaults.

Other unreported facts remain absent unless a separate named rule is admitted. No request copy may impersonate response settings. Call identities, arguments, successful terminals and whole missing usage objects cannot be fabricated. Presence normalization is field-specific, not a global null/zero rule.

### Fidelity and source isolation

Fidelity records are not a second task payload. Classified response extras need an explicit allow-list, bounded payload, trusted origin, protocol/dialect, response identity and a dependency on the response semantics they describe. Same protocol alone does not authorize reuse. Missing/different target scope, incompatible dialect or changed semantic dependencies drops nonportable extras; opaque replay tokens retain their stricter rejection contract.

Response extras are authoritative only on the terminal snapshot. Nonterminal occurrences are validated and budgeted but cannot bind a later result to an initial null. Static and event encoding use the same terminal projection. Derived views require type checks on partial snapshots and equality at terminal before discard. Nested routing/billing facts use the same classified-fidelity boundary; standard token counts stay typed and are never restored by fidelity. Named stream normalizations cannot create a semantic terminal or relax the standard profile. Structured reasoning carriers map to existing typed parts and owner/origin-bound replay; unknown formats do not inherit permissions. Current rule selections, field allow-lists and fail-closed conditions live beside the owning code and tests, not in this decision record.

### Incremental delivery

Provider SSE decodes incrementally to the shared event IR/reducer; a downstream adapter projects events incrementally. Execution must not accumulate a whole event log merely to offer streaming. Consume at most one upstream frame per step and retain unconsumed bytes at the I/O caller for backpressure.

Encoding bytes is not downstream commit. The I/O owner explicitly marks delivery committed when output becomes externally visible, and marks completion only after validated upstream closure and successful delivery. One delivery binds to an execution-owned attempt identity, not a potentially repeated wire response ID; it cannot splice a second attempt into the stream. Errors poison the chain; cancellation releases owned state; no fallback crosses commit and no error becomes a successful terminal.

`ReportedFactPolicy::StrictComplete` is a target completeness check, not a defaulting policy. It may fail at the terminal of an incremental stream, after earlier output; it does not promise pre-first-byte completeness. Consumers requiring that stronger guarantee must use bounded complete-result delivery rather than the incremental entry. Structural requirements, duplicate-key rejection, budgets, call identity and protocol terminals remain strict in every profile.

## Acceptance and non-goals

Independent wire-to-IR and IR-to-wire expectations cover profile isolation, default/report/invalid/missing usage, normalized-value provenance, mutation/deletion, source isolation and JSON/SSE closure. Delivery tests cover first output before upstream completion, bounded intake, explicit commit, late failures and cancellation. Auth-bearing request Debug output is redacted.

No service listener, credential pool, automatic retry/fallback, new task family, model-quality claim or paid/live acceptance is introduced by this refactor. Live results are reported for the current task and authorized local run only, not maintained as a compatibility report in Markdown.
