# ADR-v2-0008: Stable Core IR and Vendor-Adapted Codecs (Dual Track)

## Status

Accepted for the `semantic-v2` rewrite epoch. This replaces the provider-conformance-request route for observed vendor wire divergence. The predecessor production runtime is [archived](../../archive.md); no v2 production interface is implied.

## Context

Controlled live probes of DeepSeek (`deepseek-flash`) and Xiaomi MiMo (`mimo-v2.6-pro`) ([2026-09-28 matrix](../../implementation-status/evidence/2026-09-28-deepseek-xiaomi-provider-live-matrix.md)) showed vendor wire divergence from the fixed standard shapes:

- both vendors omit `usage.input_tokens_details.cache_write_tokens` — expected, since neither has an explicit cache-write billing dimension, so the fact is semantically *absent*, not missing;
- Xiaomi MiMo Responses reports no settings echo at all (`tools`/`tool_choice`/`parallel_tool_calls`);
- vendor extras appear on response bodies: `content_filters`, `frequency_penalty`, `presence_penalty` (DeepSeek), `output_text` (Xiaomi), plus Chat-side `reasoning_content`, `system_fingerprint`, usage aliases and stream-shape quirks.

The pinned SDK's strict model (`model_validate`) declares several of these facts required and rejects absence, while the SDK's real parse path (`construct_type`, used by actual client calls) accepts absence as `None` (both verified against the pinned `openai` build). Demanding vendor conformance would push a billing-dimension-free field onto vendors and contradict the standard's own rule that request copies must never impersonate response facts.

## Decision

1. **Two tracks, one authority.** The typed IR is the stable core and the only semantic authority; vendors are adapted at the encode/decode layer. The core must be semantically complete: every real provider difference either has a deterministic semantic owner (缺省/回显/派生 view/typed fact) or an explicitly registered gap — a missing mapping is a codec gap, never a reason to gut or widen the core. The core never grows vendor-private fields and never chases vendor shapes; vendor wire divergence is resolved in codecs, not by schema expansion and not by consistency demands sent to providers.
2. **Reported facts are presence-preserving (三态).** A reported count (`0` included) stays distinct from "no reported count" through decode → IR → encode; absent and explicit null both mean no reported count and re-encode as absence (named form normalization). Wire admission and emission never fabricate a count, an echo or a default: 缺省 is encoded as absence, never as `0` or a synthesized value. A fabricated fact is as invalid as a silent drop.
3. **Structural requirements stay strict.** Snapshot `output` arrays (explicit empty allowed), stream `sequence_number`, event grammar, strict JSON, budgets, identity rules and usage-total consistency are structural and remain enforced. This ADR relaxes fact completeness only; it does not touch the complete-snapshot hardening for structure.
4. **Vendor adaptation uses exactly three mechanisms** (the only sanctioned forms; no body hooks, no opaque passthrough):
   - **scoped typed shapes** — a vendor field with standard semantic meaning maps to an existing typed owner: `reasoning_content` → readable reasoning items, `system_fingerprint` → Chat reported fact, DeepSeek usage hit/miss aliases → normalized `cached_input_tokens` after consistency checks;
   - **derived views validated then dropped** — convenience mirrors like Xiaomi `output_text` must equal the authoritative output text and are then discarded (`output_text` never enters the IR);
   - **classified bounded response fidelity** — named vendor extras (`content_filters`, `frequency_penalty`, `presence_penalty`) round-trip verbatim within budgets on the same origin/profile and drop under the documented portability policy otherwise. Names stay on an explicit admit list; unknown keys remain rejected.
5. **No provider-conformance track.** OpenBridge does not file vendor consistency requirements as a fix strategy. Provider reference pages record observed wire facts; divergence is absorbed by adapters or recorded as a typed gap.

## Revision: explicit policy, declared portability and target scope (2026-09-29)

A compliance review of the first adaptation slice found four properties that had been removed or left implicit instead of expressed in the codec/contract layer. They are now explicit:

- **The completeness choice is expressible.** `GenerationRepresentationContract::reported_facts` carries `ReportedFactPolicy::Faithful` (default) or `StrictComplete`. A strict-complete target fails lowering (`RepresentationError::ReportedFacts`) when the settings echo or the usage sub-details are absent instead of omitting them; both delivery entries check it before emitting anything, so a refusal never follows partial delivery.
- **The delivery contract owns the target replay scope.** `Attempt::render_json`/`render_stream` take the downstream target contract; the attempt's own `replay_origin` only binds intake. A target with a different or absent scope still refuses the replay token.
- **Classified extras portability is declared.** `fidelity::declared_response_extras(profile)` is the single table shared by capture and re-encode; a profile that declares no extras can neither record nor emit them.
- **Derived-view equality holds where the view claims finality.** `output_text` is type-checked everywhere and compared with the authoritative output text on terminal snapshots only; a partial non-terminal view is admitted and dropped.
- **`tool_calls: []` declares no calls.** A Chat response may report an explicit empty list while the model emits its attempt as text and hits its budget; it normalizes to absent, matching the streaming path, instead of failing as malformed.

## Consequences and acceptance

- Downstream consumers receive faithful facts. Strict-model validation may reject bodies whose facts are absent (a vendor billing-dimension fact, not a gateway defect); real SDK consumption tolerates absence. This is an explicit downstream contract choice: faithfulness over strict-model completeness, with no invented values.
- The Chat usage projection gap (`input_cache_write_tokens` → `prompt_tokens_details.cache_write_tokens`) keeps its three-state rule: projection must not lose counts or estimate.
- Cached-input observability keeps its meaning: absence of cache-write dimensions cannot corrupt cache hit/miss accounting because nothing is filled in.
- Acceptance: `tests/semantic/vendor_shapes.rs` (three-state usage, echo absence, derived-view validation, classified fidelity portability), the existing baseline and the probe matrix re-run are the evidence boundary. A vendor changing its wire shape requires a new observation, not a core change.
