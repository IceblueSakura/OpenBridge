# ADR 0010: Canonical Models and Fixed Pre-publication Fallback

## Status

Accepted. Extends the single-entry execution slice in [ADR 0009](0009-minimal-http-text-gateway.md); no automatic activation of additional targets follows.

## Decision

- Canonical model identity and task semantics are independent of Provider aliases. Public admission may intentionally expose a narrower semantic slice. Every Endpoint binds a trusted canonical identity; compilation rejects unknown identities, cross-model Route members and public semantics exceeding canonical support.
- Provider/profile image constraints and configured Endpoint limits are typed and intersected. Format/detail/source/count/byte limits do not rewrite model support or final IR. Shared security budgets remain independently bounded.
- Routes specify fixed Endpoint order, candidate admission policy and a bounded attempt count. RequireAll keeps strict admission; SkipUnrepresentable evaluates whole final requests independently, not capability unions. Activation names only existing members and cannot reorder them. Different wire families are explicit candidates, not inferred compatibility.
- Default fallback is Disabled. BeforeCommit may advance after rate limits, server failures, connection failures or attempt timeout while total budget remains. Never repeat a candidate, hedge, reorder, expand a Route, change model semantics, drop fields or fabricate tool results. Protocol/projection/auth/permission failures, HTTP redirects and model refusal/incomplete are not retry permissions. Shared planning and I/O ownership are refined in [ADR 0011](0011-stable-admission-provider-cache.md).
- One chain owns its concurrency permit, absolute exchange deadline and bounded diagnostic trace. Each attempt owns fresh transport/decoder/encoder state and its own credential/adapter binding. Per-attempt timeout is bounded by the remaining total deadline.
- Publication of the first downstream frame conservatively freezes advancement, closing the receive/timeout race before HTTP response ownership escapes. Publication is not commit: acknowledged HTTP body handoff still owns `ResponseDelivery::commit`, and validated EOF plus final handoff owns completion. Late failures abort the body without another candidate or a fabricated terminal.
- Multi-member ingress currently rejects opaque continuation/encrypted-output requests. Canonical model equality never authorizes sharing an issuer/auth scope. Single-member replay keeps its existing boundary; explicit affinity support is a separate future slice.

## Consequences and validation

The bootstrap continues to activate its fixed single members. New real cross-Provider equivalence, route order and activation require trusted registration/operator choices, not request JSON or model-name guessing. Fallback may duplicate upstream computation/billing; cancellation does not prove that an upstream stopped processing.

Pure policy tests protect classification, visibility and attempt budgets. Independent synthetic planning protects whole-request compatibility and IR immutability. A real Router smoke protects fixed order, model/credential rebinding, unchanged image/control payloads, total/per-attempt deadlines and post-publication failure. Existing body lifecycle and fixed SDK gates continue to protect handoff, cancellation, backpressure and protocol closure. These are not live Provider, load or production acceptance.
