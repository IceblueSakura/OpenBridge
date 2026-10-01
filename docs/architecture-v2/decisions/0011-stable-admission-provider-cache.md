# ADR 0011: Stable Admission and Provider-owned Cache Affinity

## Status

Accepted. Refines [ADR 0007](0007-stateless-cache-affinity-and-extensions.md) and [ADR 0010](0010-canonical-model-fixed-fallback.md). No gateway load balancing, answer cache, session manager or sticky routing is introduced.

## Ownership

- Generation semantic support lives beside task requirements, with no dependency on protocol, topology or execution. Representation contracts contain this vocabulary rather than a second set of flags. Public context admission, identity hints, cache projection, media representation limits and execution policy have separate owners.
- Canonical models are explicitly registered once. Public aliases and upstream spellings reference that identity; neither creates it by inference. Provider bindings explicitly declare the adapter and protocol members. Bootstrap consumes these declarations instead of maintaining a second Provider/model switch.
- The public label, semantic admission and client response contract remain independent of upstream aliases. Endpoint replacements must satisfy the declared admission strategy; adding an Endpoint never edits the public contract. Provider-specific restrictions do not redefine canonical support.
- One pure bounded selector handles both library plans and activated Gateway members. Rejections use closed categories, never raw codec messages, request values or credential locators. Whole-request projection stays independent per candidate and does not union capabilities.
- `gateway::exchange` owns candidate advancement and attempt budgets; `gateway::intake` owns one upstream body's decoding/projection; `gateway::body` owns publication, acknowledgement and cancellation. The chain still owns one permit and one absolute deadline. Redirects are explicitly classified and do not authorize fallback.

## Provider cache projection

Cache affinity means using the Provider's automatic cache and documented cache-key/session carriers, not storing conversation or answers in OpenBridge. Every request still supplies its required history. Fixed candidate order is unchanged across requests.

- Preserve deterministic target projection, tool/Schema order and surviving history prefixes. Do not sort arbitrary JSON, inject changing prompt prefixes or claim semantic equivalence proves cache equivalence.
- `CacheHints` owns standard cache hints; `ClientIdentityHints` independently owns `user` and `safety_identifier`. Omitting advisory cache hints must not erase identity. Undeclared identity fields fail target projection rather than silently disappearing.
- Target cache projection explicitly admits individual fields. An unsupported advisory key/retention can be omitted without disabling Provider automatic caching. Supplied cache options are behavioral controls and are rejected when unsupported rather than silently discarded.
- The OpenBridge client profile admits a bounded nonempty `session_id` body extension as Provider cache/observability grouping. It is represented outside task IR by a typed cache-session carrier. No value is derived from another identifier; `session_id` and `prompt_cache_key` remain independent. The standard codec profile does not gain this extension.
- A target must explicitly admit the session carrier or reject the request. Business requests cannot choose arbitrary header names, destinations or credentials. Incoming session headers are not forwarded; this slice uses declared body projection only. Provider-private header projection requires its own documented target mapping, not a generic passthrough.
- Response cache usage and echoes remain upstream-reported facts, never copied from the request. Unknown cache effectiveness stays unknown. Synthetic prefix stability is not a live cache-hit or cost claim.

## Diagnostics and verification

Diagnostics keep bounded per-candidate ordinals, stages, byte counts, statuses, durations and closed failure codes. No raw model/session/cache keys, bodies or credential locators enter the record. Sink failure does not affect business responses.

Independent tests protect semantic admission, multiple public aliases, shared candidate selection, identity/cache separation, extension target rejection, IR edits, stable multi-turn prefixes and tool/Schema order. Synthetic Router checks cover unchanged cache carriers across explicitly ordered candidates. SDK and lifecycle regressions remain separate gates. Real Provider acceptance and cache-hit measurement need a separately authorized bounded experiment.
