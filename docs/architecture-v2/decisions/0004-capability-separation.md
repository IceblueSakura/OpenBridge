# ADR-v2-0004: Separate Semantic, Representation, Execution and Public Capabilities

## Status

Accepted.

## Decision

Capability is modeled in four dimensions: semantic contract, representation contract, execution contract and public contract. Requirements are derived from final Task IR plus delivery intent.

No generic capability bitset may become authoritative across these dimensions.

## Consequences

Registry compilation must validate relationships among the four contracts. Existing capability structures can be reused only after fields are assigned to a single dimension. Candidate compatibility is a typed representability check, not ad-hoc JSON filtering.
