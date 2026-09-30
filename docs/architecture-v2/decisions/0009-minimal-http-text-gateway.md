# ADR 0009: Minimal HTTP Text Gateway

## Status

Accepted for the first runnable text slice. Implementation and executed acceptance
remain separately tracked in the architecture, [Generation status](../../implementation-status/generation.md) and tests.

## Decision

- Expose authenticated `POST /v1/chat/completions` and `POST /v1/responses` over a
  loopback-only server. Share the current adapters and execution chain; no Native
  bypass, upstream JSON patching, or restoration of predecessor runtime code.
- Startup supplies compiled topology, credential bindings and explicit entries
  `(public model, downstream protocol, endpoint)`. An entry's endpoint must belong
  to the compiled model Route. This slice executes that one fixed candidate, with
  no retries/fallback. Default entries use matching wire families. Explicit
  cross-family entries retain normal representability failures.
- Parse bounded strict JSON only to resolve the public model/task before semantic
  decode. Authenticate before reading the body. Derive requirements after the
  trusted output-budget policy has filled an absent limit; reject explicit limits
  above the configured ceiling instead of silently clipping them.
- Keep HTTP transport semantically blind: it receives prepared trusted request
  parts and never forwards inbound headers or follows redirects/retries. Upstream
  auth/routing diagnostics are not reflected in downstream errors.
- Bind replay provenance to the fixed public entry and credential/auth ownership.
  Runtime scope labels stay outside task semantics and never come from business
  JSON. This is an internal ownership boundary, not attestation that an arbitrary
  client-supplied opaque token was originally issued in that scope; its issuer
  remains responsible for validating it.
- Stream with bounded framing state and one acknowledged outbound frame at a
  time. Encoding/queueing does not commit. A frame is acknowledged when the HTTP
  body hands it to the server transport; that is the conservative external
  visibility boundary, not a claim of peer receipt or TCP acknowledgement.
- Upstream EOF is validated before any terminal is released. Producer completion
  requires all frames to be handed off. Timeout, cancellation, shutdown and late
  decode/projection errors release upstream resources and terminate the body with
  an error, never a fabricated successful terminal. An unpolled/backpressured
  body does not suspend the upstream deadline.
- Gate resource-intensive work by bounded concurrency, request collection timeout,
  request/response byte budgets and an absolute exchange deadline. No payload
  logging is enabled by this slice. An explicit startup-only probe sink may record
  bounded, authenticated, content-free attempt metadata in a private operator
  file. Its queue is nonblocking, failure does not alter business responses, and
  neither upstream diagnostics nor caller correlation IDs are reflected downstream.
  This is not a general observability or retry subsystem.

## Bootstrap

Use environment variables, not the archived private configuration schema. A
single downstream Bearer identity and the enabled providers' API keys configure
startup; missing/invalid credentials fail closed. Preserve the existing fixed
catalog and require an explicit trusted proxy setting rather than inheriting
arbitrary process proxy state. Library tests use only synthetic keys and loopback.

The startup interface and HTTP error/body contract are documented in
[the HTTP gateway guide](../../http-gateway.md). This does not introduce a dynamic
registry, general configuration format, multi-user credential manager or plugin
framework.

## Acceptance

A real Router must use a synthetic HTTP Provider rather than manufacture client
answers. Verify native and admitted cross-family projection, IR budget changes,
first-frame-before-terminal, cancellation/backpressure, strict EOF, scope and
credential isolation. The pinned official SDK must exercise the same Router and
Provider chain. Pure codec tests, external Provider probes and production/load
acceptance remain separate evidence layers.
