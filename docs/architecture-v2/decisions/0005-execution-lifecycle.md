# ADR-v2-0005: Execution Is Semantically Blind After Encoding

## Status

Accepted.

## Decision

Execution receives compiled candidate plans and encoded protocol representations. Credential binding, retry, fallback, cancellation, transport and commit are execution concerns.

Transport and attempt coordination cannot inspect or alter Task IR semantics. Retry/fallback is allowed only before downstream commit and cannot modify route order or semantic input.

Response bytes/events re-enter semantics through the selected endpoint codec before downstream delivery.

## Consequences

Existing bounded retry, credential isolation and commit invariants are retained. Semantic JSON mutation currently performed late in provider/execution paths must move earlier into lowering/profile logic.
