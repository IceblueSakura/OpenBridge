# ADR-v2-0008: One Semantic Core and Explicit Boundary Adapters

## Status

Accepted. Field-specific compatibility defaults are allowed under explicit rules; there is no single permissive global Chat/Responses profile.

## Decision

- Keep one Responses-first semantic core. A different wire spelling changes an adapter; a genuinely missing capability may extend shared semantics. Do not create Provider-specific Generation or Usage types.
- Trusted client and Provider adapters compose shared protocol codecs. Business JSON cannot select dialect, origin or target; no generic body hooks or dynamic plugin framework are introduced.
- Named rules may map aliases, validate/discard derived views, exactly derive values or supply field-specific compatibility defaults. Defaults are not reported observations: preserve valid reports, reject malformed values, retain absence unless the named rule explicitly permits normalization, and record provenance outside task semantics.
- Encoding consumes final semantics without reapplying intake defaults. Never fabricate call identities, arguments, a whole missing usage object or a successful terminal.
- Fidelity is bounded, source/profile/owner-dependent representation, not a second task payload. Classified extras use the authoritative terminal snapshot; incompatible or stale extras cannot restore edited semantics. Opaque replay keeps its stricter rejection contract.
- SSE intake/projection remains incremental and bounded. One delivery binds to one execution attempt, not a reusable wire response ID. Encoding is not commit; late failures abort rather than splice another attempt or synthesize success.

## Rationale and consequences

Explicit adapters absorb real wire differences without weakening the standard codec or splitting semantic authority. Each normalization has a narrow contract and independent rejection cases. Target completeness checks may fail at terminal after earlier output; pre-first-byte completeness requires bounded complete-result delivery instead of incremental streaming.

Exact rule selection, defaults and allow-lists live in [Adapter](../../../src/adapter/mod.rs), [WireRules](../../../src/protocol/adaptation.rs) and owning codecs. [Fidelity](../../../src/protocol/fidelity.rs), [Attempt](../../../src/execution/attempt.rs) and [ResponseDelivery](../../../src/execution/delivery.rs) own dependency and lifecycle enforcement. Do not duplicate their field tables or test results here.
