# ADR 0010: Canonical Models and Fixed Pre-publication Fallback

## Status

Accepted. Extends [ADR 0009](0009-minimal-http-text-gateway.md); registration does not automatically activate additional targets.

## Decision

- Canonical model identity and task semantics are independent of Provider aliases. Public admission may be narrower; topology compilation rejects unknown identities, cross-model Route members and public semantics exceeding canonical support.
- Intersect typed model, representation and execution constraints without rewriting final IR. Routes fix candidate order, admission strategy and bounded attempts. Project the whole immutable request independently for each candidate; never union capabilities or reorder activation.
- Fallback is disabled unless explicitly allowed. Eligible pre-publication failures may advance while total budget remains; never repeat, hedge, expand the Route, drop semantics or invent tool results. Protocol/projection/auth/permission errors, redirects and model refusal/incomplete do not authorize fallback.
- One chain owns its permit and absolute deadline. Each attempt has fresh transport/codec/delivery state and its own credential binding; attempt deadlines cannot exceed the remaining total budget.
- First downstream-frame publication freezes advancement before response ownership escapes. Publication is distinct from acknowledged body handoff/commit; validated EOF and final handoff govern completion. Late failure aborts the body.
- Canonical equality never authorizes replay across issuer/auth scopes. Until explicit affinity ownership is admitted, multi-member ingress rejects opaque continuation/encrypted-output requests rather than guessing an equivalent target.

## Rationale and consequences

Fixed selection preserves a stable public contract while allowing bounded recovery without dynamic routing. Publication must close the timeout/receive race earlier than I/O commit. Fallback may duplicate upstream computation and billing; cancellation does not prove upstream processing stopped.

Owners: [topology compilation](../../../src/topology/compile.rs), [candidate planning](../../../src/execution/plan.rs), [fallback policy](../../../src/execution/fallback.rs), [exchange](../../../src/gateway/exchange.rs), [body](../../../src/gateway/body.rs). Shared admission ownership is refined by [ADR 0011](0011-stable-admission-provider-cache.md).
