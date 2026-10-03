# ADR 0012: Profile-neutral authorization credential management

## Status

Accepted. Management and activation do not authorize live credential operations or inference.

## Decision

- The [manager](../../../src/credential/manager.rs) owns lifecycle, identity binding, operation tickets and publication. Registered [drivers](../../../src/credential/driver.rs) own authority/client contracts, wire encoding, verified identity, grant-field inheritance and revocation. Manager/store/transport do not branch on Provider identity. Shared OAuth, PKCE, callback and cryptographic mechanisms do not form a configurable workflow; JWT headers never select trusted issuers, algorithms or key URLs.
- Authorization profiles are distinct from inference Providers. A binding pins profile, client and verified principal, including workspace scope where applicable. Equal aliases, subjects or Bearer syntax do not make credentials interchangeable. Grok personal authorization and Codex product authorization are isolated; Codex is not public SIWC.
- The [Unix store](../../../src/credential/store.rs) publishes one protected, readable account document per profile/alias. Identity and credentials are atomic; revision, credential generation and login ticket have separate meanings. Session lifecycle and access freshness are distinct. Only this explicit owned-store format is supported, without auth-cache import or implicit migration.
- Account-operation locks exclude competing mutations; store transactions cover only bounded local I/O, never authority requests or human interaction. Publication checks the current ticket/client/principal and identity uniqueness. Failed or cancelled login preserves the usable session. Contention returns busy, not an implicit retry or account switch.
- Rotating refresh durably removes reusable secrets and supersedes pending login tickets before transmission. Cancellation, rejection or uncertain completion requires reauthorization. A durable pending marker precedes account replacement; interrupted or uncertain publication quarantines the account until explicit login or local logout. Confirmed document durability and marker cleanup are distinct; a stale marker never restores an older token.
- Logout clears the selected local session before optional remote revocation. Remote acknowledgement, uncertainty and local completion remain distinct. Secret values and identity bundles are never status or diagnostic output.
- [Access bindings](../../../src/credential/access.rs) pin identity and borrow a fresh access snapshot for each attempt. Rotation preserves private replay identity; expiry, logout, quarantine or identity change rejects later borrows. Unknown expiry remains unknown. No filesystem lock spans inference I/O; already-dispatched requests may finish with their snapshots.
- [Gateway credential sources](../../../src/gateway/credentials.rs) enforce kind/profile isolation. [Execution metadata](../../../src/provider/subscription.rs) supplies closed trusted headers. Business data cannot select accounts, authorities, proxies or auth headers. HTTP transport disables redirects, ambient proxies and automatic retries. Tokens, locators and refresh state never enter task IR; ordinary requests do not log in, refresh, switch accounts or replay automatically.

## Rationale and consequences

Authorization lifecycle is the reusable boundary, not the union of product wire fields. Profile-specific protocol and metadata stay in drivers; CLI and Gateway consume the manager rather than each owning a credential lifecycle. This is not an account scheduler, quota service or cross-account fallback mechanism.

The filesystem must support advisory locks, atomic rename and fsync. Owner-only permissions do not isolate malicious same-UID processes, provide multi-host coordination or guarantee secure erasure. Local logout and cancellation do not prove upstream computation or billing stopped.

Operations and recovery: [credential guide](../../credentials.md). Fixed protocol provenance and adoption boundaries: [Grok](../../references/grok-login.md), [ChatGPT/Codex](../../references/chatgpt-login.md). Exact layouts, budgets, client metadata and failure rules belong to their code owners.
