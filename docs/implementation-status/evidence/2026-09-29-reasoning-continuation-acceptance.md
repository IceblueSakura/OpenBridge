# Reasoning ownership and encrypted continuation acceptance

- Date: 2026-09-29; `semantic-v2`, base `dee4c05` plus the accompanying OpenRouter/reasoning working-tree changes, not the unchanged base commit.
- Target: existing `gpt-6-luna` → OpenRouter `openai/gpt-6-luna`; fixed OpenAI SDK `3.19.0` → disposable loopback binary → real Provider.
- Scope: Chat/Responses × JSON/SSE × actual encrypted-state acquisition followed by one same-target continuation. This closes the specifically unexecuted live encrypted-continuation layer in the [earlier Luna matrix](2026-09-29-openrouter-luna-acceptance.md), not every reasoning format or backend.
- Bounds: **8 requests executed**, from a 16-request ceiling; maximum 2048 output tokens/request. No SDK/OpenBridge retries or fallback, hosted tools or additional models; OpenRouter-internal routing remains outside this gate's control. Credentials were read in-process from the existing workspace binding and were not modified.

## Live matrix

Every first turn requests medium reasoning and a small arithmetic JSON result. Responses also requests `reasoning.encrypted_content`. A first turn counts as success only if its completed output contains a nonempty encrypted token with issuer item identity **and** the JSON answer passes an independent arithmetic oracle. Missing ciphertext does not count as encrypted-replay acceptance.

The second turn appends the complete actual assistant/output history and a synthetic follow-up arithmetic question. Before sending, the probe checks the SDK's serialized JSON body for the same ordered `(issuer identity, ciphertext)` records in memory; omission, replacement or reattachment fails before HTTP I/O. The continuation must then complete and pass the second arithmetic oracle.

| Protocol | Delivery | First-turn encrypted records / bytes | Acquisition | Serialized replay equality | Continuation |
|---|---|---:|---|---|---|
| Chat | JSON | 1 / 1536 | passed | passed | passed |
| Chat | SSE | 1 / 1536 | passed | passed | passed |
| Responses | JSON | 1 / 1556 | passed | passed | passed |
| Responses | SSE | 1 / 1556 | passed | passed | passed |

All **8/8** requests passed through the real gateway binary. Ciphertext, issuer IDs and reasoning bodies were not printed or saved. The sanitized result file is retained locally at `testdata/runtime/gateway-luna-reasoning-1790694336485148181/calls.json`; this document is the portable evidence entry. The binary was shut down after the gate.

The equality observation is at SDK → gateway ingress. A separate independent Rust assertion verifies unchanged opaque values in `execution::prepare`'s actual upstream request body and rejection for a mismatched trusted origin. This is not an egress packet capture, decryption or proof of the Provider's internal use of the token rather than the accompanying readable history. Successful acceptance does not establish cache hits, cost savings or portability across accounts/issuers/models.

## Executed offline failures and fixes

The lifecycle probes use independent synthetic values, not captured real ciphertext. They exposed local codec/lowering defects, not malformed behavior by the live Provider:

1. A Chat `length` response changed an already completed encrypted reasoning owner to incomplete, invalidating its dependency fingerprint and disagreeing with event materialization. Complete encrypted details now keep the reasoning owner's completion; the carrier message and response can still be incomplete.
2. A Partial token on an incomplete reasoning item could disappear from static Chat projection or be emitted in a final-only encrypted detail by event projection. Chat now rejects that representation before emitting an opaque detail; partial values can never be promoted to final tokens.
3. Final token bytes alone were insufficient: an incomplete reasoning owner also has no faithful representation in the Chat final-detail carrier. Static and event lowering reject that combination rather than upgrading owner lifecycle.

`tests/semantic/reasoning_boundary.rs` protects readable-only output and continuation without any encrypted-origin requirement; typed text/summary ownership; replacement/deletion; static/event agreement for completed reasoning plus an incomplete answer; partial/final and owner status; source mismatch; and event-authoritative token replacement/removal despite stale fidelity. No assistant answer is fabricated from readable thinking text.

`tests/transport/chain.rs` protects the actual prepared-wire opaque value and origin check. `tests/sdk/test_live_probe_helpers.py` proves that the live probe's replay checker rejects missing/rebound records and request-budget exhaustion without invoking any network or credential loading. Existing Responses lifecycle, EOF, resource and replay regressions and the fixed SDK loopback gates were run separately.

The core types remain unchanged: readable text/summary belongs to ReasoningItem, opaque state to the existing owner-bound ReasoningReplay. Item completion is independent of response completion. A readable-only model does not need an encrypted token or replay scope, and absence of encryption is not a missing readable-reasoning capability.

## Reproduction

These commands make **paid calls** and require fresh scoped authorization; a flag or an existing key is not authorization by itself:

```sh
cargo build --locked --offline --bin openbridge
OPENBRIDGE_GATEWAY_PROBE=1 OPENBRIDGE_GATEWAY_REASONING=1 \
  uv run --project tests/sdk --locked --offline python examples/live_gateway_probe.py
```

The reasoning flag replaces the default text/tool matrix with the fixed 8-call reasoning matrix. Each run uses a new ephemeral listener and unique sanitized report directory. It retains ciphertext only in memory and does not reuse the earlier raw-capture probe.

Offline gates need no real keys or Provider network:

```sh
cargo test --locked --offline --test semantic reasoning_boundary
cargo test --locked --offline --test transport opaque_replay_reaches
uv run --project tests/sdk --locked --offline python -m unittest discover \
  -s tests/sdk -p test_live_probe_helpers.py
```

## Limits

This proves four same-target two-turn encrypted continuations, not decryption correctness, arbitrary signatures/formats, other models, long conversations, tool turns that actually contain encrypted reasoning, cross-protocol/cross-account/cross-backend replay, live failure injection or production behavior. Validation of arbitrary client-supplied ciphertext remains the issuer's responsibility; internal trusted scope is not an attestation of arbitrary submitted ciphertext. Readable-only behavior is covered by independent offline contracts here; no new readable-only model was selected or called in this batch. New models need their own fixed wire/capability evidence and explicit live matrix, not copies of this model's inventory or assumptions about encryption.
