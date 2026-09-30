# ADR-v2-0007: Stateless Cache Affinity and Extension Carrier Ownership

## Status

Accepted for the current OpenBridge semantic core. This defines cache and extension ownership, not a predecessor migration target. The predecessor remains a [historical reference](../../archive.md); current implementation and execution boundaries belong to [Generation status](../../implementation-status/generation.md).

## Context

Request hints (`prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `safety_identifier`, `user`) and reported response context are distinct facts on both Responses and Chat bodies. The Codex HTTP header baseline additionally shows provider-private execution context split across cache affinity, logical session, thread identity, context window, turn state, routing hints and agent lineage, projected into custom body sections (`client_metadata`) and custom HTTP headers. A universal session ID or a raw `extra_headers` passthrough would both collapse these owners and open credential injection.

## Decision

1. **Cache affinity is typed standard data.** `prompt_cache_key` is the standard cache-affinity carrier. `CacheHints` owns the cache family for both protocol bodies. A request hint and a reported echo are separate facts: the codec never synthesizes echoes, never converts `prompt_cache_retention` (deprecated maximum policy) into `prompt_cache_options.ttl` (minimum lifetime) and never aliases `user` onto `prompt_cache_key`. `prompt_cache_options.prewarm` preserves omitted, explicit `false` and `true`; `null` is not a bool. `prompt_cache_options` echoes describe options applied to the response, not a copy of the request.
2. **Cache affinity is not session identity.** A derived affinity key may later be projected by a ChatGPT provider profile into `prompt_cache_key` and the `session-id` header together, but the codec never derives either from the other and never merges logical session, thread, window or turn state into one ID. No identity or sticky token is fabricated without a real runtime owner.
3. **Provider-private context lives in a protocol-layer extension carrier, never Task IR.** `protocol::extensions` owns `CodexHeaders` (typed lifecycle fields: session/cache scope, thread identity shared by `thread-id` and `x-client-request-id`, context window, opaque turn state, typed routing hint, lineage, agent label, bounded opaque turn metadata) and `CustomSections` (admitted body sections such as `client_metadata`). Header encoding projects the documented compatibility forms; each value keeps one owner and conflicting duplicates fail.
4. **Opaque fallback is bounded and never credential-class.** Unmodeled safe headers and section contents round-trip verbatim within count, length and byte budgets, with control-byte and header-injection rejection. Credential, account-locator and transport headers (`Authorization`, `ChatGPT-Account-ID`, `X-OpenAI-Fedramp`, `x-oai-attestation`, cookies, API keys, framing headers) are never context and fail closed. Body section names stay on an explicit admit list; unknown body keys remain rejected.
5. **Turn state is opaque and codec-inert.** `x-codex-turn-state` only round-trips or drops. Nothing derives it, extends its lifetime or replays it across turns; that requires a future turn-management mode. Routing hints are typed `model`/`tier` syntax and never sticky state.

## Standard target and current subset

The fixed SDK `3.19.0` declares `prompt_cache_options` with `mode`, `ttl`, `prewarm` and `comparison_response_id`, and echoes `prompt_cache_key`, `prompt_cache_options`, `prompt_cache_retention`, `user`, `prompt_cache_diagnostics` and effective `service_tier` on responses. Current field admission belongs to the [Responses](../responses-text-profile.md) and [Chat](../chat-text-profile.md) profiles; remaining carrier wiring and execution gaps belong to [Generation status](../../implementation-status/generation.md). Carrier types and offline read/write checks alone do not establish Gateway admission or runtime lifecycle management.

The Codex header schema is a provider-private compatibility profile, not public OpenAI API. Current admission covers the documented request-side lifecycle headers and bounded opaque fallback; response-side typed observation headers (`x-request-id`, `openai-model`, rate-limit families) stay in the opaque fallback, and cross-position consistency between `client_metadata` values and header projections is not enforced because the canonical turn-metadata schema is not owned here.

## Consequences and acceptance

- `semantic::context` owns `CacheHints`; Responses and Chat context shells project that shared owner, and the two profiles may widen independently.
- Extension carriers are validated at decode and encode; hand-built contexts cannot emit forbidden or malformed headers.
- Privacy: session/thread/window/turn values are high-cardinality sensitive metadata and stay out of logs, metrics labels and fixtures.
- Stateful request fields (`previous_response_id`, `conversation`, `store`, `background`, prompt templates, compaction) remain explicitly rejected until their own slice; deferring them is scope, not a permanent exclusion.
- Acceptance is offline: `tests/semantic/extensions.rs` and the cache-hint contracts in `tests/semantic/text_profile.rs`. No live Provider, SDK loopback or network behavior is proven.
