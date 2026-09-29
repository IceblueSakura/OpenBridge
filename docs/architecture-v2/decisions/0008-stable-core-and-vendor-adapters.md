# ADR-v2-0008: One Semantic Core and Explicit Boundary Adapters

## Status

Accepted for the unpublished `semantic-v2` rewrite. Internal APIs may be replaced without compatibility aliases. This decision supersedes the unconditional prohibition on compatibility defaults and the treatment of all vendor shapes as one global Chat/Responses profile. It does not authorize production deployment or Provider calls.

## Decision

### One semantic authority

Generation remains Responses-first, not a lowest-common-denominator model. Task semantics, standard context, delivery intent, reported response facts and typed extensions have explicit owners. Provider/client identity, credentials, sockets and retry/commit state never become task semantics. A new spelling changes an adapter; a genuinely missing capability may extend the shared semantic model. There are no provider-specific Usage or Generation variants.

### Protocol families and adapters

Chat and Responses supply shared syntax, structural validation and event grammar. Trusted adapter profiles separately declare standard, OpenBridge-client, DeepSeek, Xiaomi and OpenRouter wire rules. Business JSON cannot choose a dialect, origin or target. Client adapters decode requests and encode responses/events; Provider adapters encode requests and decode responses/events. Both reuse protocol implementations, without generic body hooks or a dynamic plugin framework.

A target projection reads immutable final semantics and context, checks representability and produces wire through the selected adapter. Execution binds targets/authentication and coordinates I/O; it does not own field-by-field context conversion. A same-protocol path uses the same semantic authority as a cross-protocol path.

### Explicit normalization, not invented observations

Adapters may map aliases, validate and discard derived views, exactly derive values, or supply a field-specific compatibility default. These rules are part of the public compatibility contract, not claims that the Provider reported the resulting values.

The admitted default is **DeepSeek cache-write usage**: when a valid usage object exists and the cache-write detail is absent or null, normalize it to `0`. Preserve any reported integer, including nonzero values. Reject malformed/negative/overflowing values; never mask them with a default. Missing/null usage remains absent. This rule applies to JSON and SSE and to both admitted protocol entries. It does not infer physical cache activity, price or billing. Xiaomi and the standard/client profiles have no such default.

Bounded normalization records identify compatibility-derived values outside task semantics. They do not restore deleted values or override transformations. Encoding consumes final semantic values; it does not repeatedly apply intake defaults.

Other unreported facts remain absent unless a separate named rule is admitted. No request copy may impersonate response settings. Call identities, arguments, successful terminals and whole missing usage objects cannot be fabricated. Presence normalization is field-specific, not a global null/zero rule.

### Fidelity and source isolation

Fidelity records are not a second task payload. Classified response extras need an explicit allow-list, bounded payload, trusted origin, protocol/dialect, response identity and a dependency on the response semantics they describe. Same protocol alone does not authorize reuse. Missing/different target scope, incompatible dialect or changed semantic dependencies drops nonportable extras; opaque replay tokens retain their stricter rejection contract.

Response extras are authoritative only on the terminal snapshot. Nonterminal occurrences are validated and budgeted but cannot bind a later result to an initial null. Static and event encoding use the same terminal projection. The first admitted extras are `content_filters`, `frequency_penalty` and `presence_penalty` on the DeepSeek Responses adapter. Xiaomi `output_text` is a derived view: type-check partial snapshots, check equality at terminal, then discard. The [OpenRouter text adapter](../openrouter-text-profile.md) applies the same classified-fidelity boundary to router/billing fields, including nested usage fields; standard token counts remain typed and are never restored by fidelity. Its content-free repeated finish/usage tail and optional Responses post-terminal DONE are named wire normalizations, not new semantic terminals or a relaxed standard profile. Its single-part `openai-responses-v1` Chat reasoning details map to existing typed summary/text and owner/origin-bound replay; the explicit OpenBridge client extension uses the same mapping. Other formats do not inherit those replay permissions. Zero-only modality counters may normalize to no modality usage in this text-only adapter; positive, null or malformed values are rejected rather than hidden.

### Incremental delivery

Provider SSE decodes incrementally to the shared event IR/reducer; a downstream adapter projects events incrementally. Execution must not accumulate a whole event log merely to offer streaming. Consume at most one upstream frame per step and retain unconsumed bytes at the I/O caller for backpressure.

Encoding bytes is not downstream commit. The I/O owner explicitly marks delivery committed when output becomes externally visible, and marks completion only after validated upstream closure and successful delivery. One delivery binds to an execution-owned attempt identity, not a potentially repeated wire response ID; it cannot splice a second attempt into the stream. Errors poison the chain; cancellation releases owned state; no fallback crosses commit and no error becomes a successful terminal.

`ReportedFactPolicy::StrictComplete` is a target completeness check, not a defaulting policy. It may fail at the terminal of an incremental stream, after earlier output; it does not promise pre-first-byte completeness. Consumers requiring that stronger guarantee must use bounded complete-result delivery rather than the incremental entry. Structural requirements, duplicate-key rejection, budgets, call identity and protocol terminals remain strict in every profile.

## Acceptance and non-goals

Independent wire-to-IR and IR-to-wire expectations cover profile isolation, default/report/invalid/missing usage, normalized-value provenance, mutation/deletion, source isolation and JSON/SSE closure. Delivery tests cover first output before upstream completion, bounded intake, explicit commit, late failures and cancellation. Auth-bearing request Debug output is redacted.

No service listener, credential pool, automatic retry/fallback, new task family, model-quality claim or paid/live acceptance is introduced by this refactor. Historical live evidence describes its original implementation only; offline validation does not refresh it.
