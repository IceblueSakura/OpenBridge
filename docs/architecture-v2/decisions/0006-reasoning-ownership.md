# ADR-v2-0006: Reasoning Ownership

## Status

Accepted for the `semantic-v2` rewrite epoch. This decision does not implement a codec or authorize the next code slice.

## Context

Text and function calls are the active Generation path. The current v2 `ReasoningRequest` is only a placeholder: codecs reject reasoning fields, and `Item` has no reasoning replay. The predecessor implementation already separates request controls, readable reasoning output, and provider-owned replay state. Putting those into assistant text would make later deletion and cross-profile encoding ambiguous.

## Decision

Reasoning has three owners:

1. **Request controls** are portable request semantics, not response text. An absent reasoning object, an omitted child field, and an explicit `none` or disabled summary are distinct. Only the closed labels both Chat and Responses can represent may cross profiles. Provider-specific labels fail before encoding.
2. **Readable reasoning output** is a separate response item. Visible text and summary are parts of that item. They are never assistant message content and are never joined into visible text.
3. **Opaque replay** is same-provider state, not portable task semantics. It requires an origin. Cross-provider encoding fails. Deleting the item removes it from every encoding; source records cannot restore it.

Reasoning tokens belong to usage details, not to reasoning content. Events, production wiring, and provider-specific reasoning channels stay outside this decision.

## Independent expectations

A later codec slice must prove these without using the current placeholder enum as the oracle:

- omitted effort encodes as an absent field, not as `none`;
- explicit `none` remains present and distinct from omission;
- deleting a reasoning item removes it from both Chat and Responses encodings;
- opaque replay never appears as assistant text;
- opaque replay without a matching provider origin fails before encoding.

## Consequences

The current `ReasoningEffort` and `ReasoningSummary` types are not this contract. Do not expose them through Chat or Responses codecs until they are replaced by the presence model above and the expectations have failing tests.
