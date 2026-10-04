# OpenBridge-client Responses scoped carrier

This is the explicitly selected `Dialect::OpenBridge` client contract over the existing Responses envelope, not a change to standard Chat/Responses or a native Interactions ingress. Low-level standard codecs and standard target lowering remain closed. Trusted adapter selection admits the extension; business JSON cannot select an adapter, origin, endpoint or credential. HTTP wiring belongs to the [gateway](../http-gateway.md), not this profile.

## Ownership and schema

A bounded `_openbridge` object with integer `version: 1` carries only the finite declarations owned by the [item codec](../../src/protocol/openai/client.rs) and [usage codec](../../src/protocol/openai/client/usage.rs). Unknown versions, fields, formats and misplaced attachments fail. The extension is decoded into existing typed semantics, then discarded; it is not raw fidelity payload or another Provider IR.

- A function call can declare structured JSON argument authority and an explicit preceding assistant-message wire-ID reference. No label means the standard raw-string authority. One base `arguments` string is the only body; the label determines strict typed recovery rather than storing a competing JSON value.
- A reasoning item can carry a format/phase-bound Google Interactions thought attachment, independently of readable summary. It does not impersonate standard `encrypted_content`; competing replay values are rejected. Native Responses encrypted content keeps its existing carrier.
- Function/custom results in request history can declare structured JSON output authority and a separate execution report. Failure is not a payload kind; the reported body remains available and must not be replaced by an error string.
- The response can report interaction progress and scoped token reports. Neither infers logical-turn completion, continuation readiness or execution permission. A report's scope refers to operation, session or a delivered item wire ID, not an internal allocation ID. Relationships and delta/cumulative/final basis are explicit; unknown counts stay omitted, never zero-filled.

Existing base fields remain their sole value authority. Standard-representable usage stays in the base usage object. Other admitted usage reports reside only in the root attachment, with base `usage: null`; competing non-null base usage is rejected. Counts, relations, report updates and owner checks use the [semantic usage owner](../../src/semantic/task/generation/usage.rs). Profile structural syntax does not expand billing or modality semantics.

Illustrative synthetic history item, not a model/Provider activation claim:

```json
{"type":"function_call","id":"call-wire-id","call_id":"call-logical-id","name":"lookup","arguments":"{\"n\":18446744073709551616001}","status":"completed","_openbridge":{"version":1,"arguments":"json","message":"preceding-assistant-wire-id"}}
```

The referenced assistant message must actually survive in the preceding history with that wire identity. Array adjacency, call names and local item counters are not membership proof. Removing or changing a typed owner cannot resurrect its old attachment through fidelity. Structured bodies use the shared exact, duplicate-rejecting, bounded [JSON parser](../../src/semantic/value/json.rs), not the discarded SDK parsed view.

## Static, events and client return

The semantic event opening declares argument format before a part/delta is published. Structured argument events use the existing function-argument wire event grammar, with partial authority in the opening and completed authority only after a valid complete value. Completion of an invalid or truncated JSON builder is rejected. Item-done and real terminal snapshots must agree with event-owned values, identity and membership; a terminal cannot fill a missing attachment or complete an unfinished part.

Response progress and scoped usage are reported in the real terminal snapshot. No custom event type is emitted: the pinned strict SDK event union does not admit it. Initial snapshots do not carry these root facts. This does not fabricate a terminal, widen the standard event grammar or promise a terminal after failure. Existing stream framing, budgets, poisoning and post-commit abort rules remain binding.

Clients preserve delivered item fields through SDK `model_dump`, actual JSON persistence and next-request history. They append independently reported tool results and retain each item's wire identity/attachment; they do not replay the root response object as request settings. Root facts remain response reports, not new execution directives. A client that drops unknown fields is incompatible with this scoped contract; standard profiles must reject rather than silently accept a reduced history.

Independent owners and checks: [JSON/event/edit oracles](../../tests/semantic/client_carrier.rs), [wire fixtures](../../tests/support/client_carrier.rs), [fixed SDK HTTP boundary](../../tests/sdk/client_carrier.rs) and [SDK persistence/return](../../tests/sdk/client_carrier_loop.py). Commands and isolation requirements belong to [development](../development.md#固定-openai-sdk-loopback); execution results do not belong here.

## Trust and remaining boundaries

A format label, client wire reference or ordinary dependency hash is not issuer authentication. The attachment carries no origin, credential locator, endpoint locator, account or execution grant. Trust scope is supplied separately by the adapter; local binding and finite dependency checks cannot prove cross-request integrity or real Google acceptance. Client-provided opaque bytes must never be treated as authenticated issuer data merely because they parse.

This carrier closes the selected syntactic client boundary for existing assistant/function membership and reasoning attachments. It does not provide general turn/group identity, authenticated group/prefix proofs, other attachment kinds, Google codec/activation, account management, a session service, media-resource lifecycle or orchestration. Standard target representability remains independently required on the next request; an admitted client history may still be rejected before upstream I/O. Broader directions remain in [Generation gaps](../implementation-status/generation.md).
