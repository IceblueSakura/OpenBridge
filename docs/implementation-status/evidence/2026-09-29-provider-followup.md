# Provider follow-up: classify failures without rewriting outputs

- Date: 2026-09-29 UTC; baseline `ded965e` plus the accompanying diagnostic helpers. No runtime codec or gateway authentication change was required by this batch.
- Scope: the eight previously selected API-key model bindings, fixed OpenAI SDK `3.19.0`, and pi `0.87.1` through the actual loopback binary. No OAuth, new models, credential changes or deployment.
- Bounds: **30 generation-request attempts**, under a 64-attempt ceiling; at most 2048 output tokens/request, no automatic retries. Synthetic text and a fixed-file read tool only. Failed authentication/quota attempts are included; this is not an invoice or token-usage claim.
- Prior evidence: [initial onboarding](2026-09-29-api-key-provider-onboarding.md). Earlier failures are not erased by later success.

## Findings

| Target | Executed observation | Conclusion / next boundary |
|---|---|---|
| Bailian / `qwen3.8-max` | The selected Beijing Chat endpoint returned 401 with `invalid_api_key` | Confirm the key and its region/workspace ownership. This does not establish that the key is invalid at every possible endpoint, and is not repaired by changing the codec. |
| Kimi / `kimi-k3` | Generation returned 429; the bounded error classifier found account and balance-related wording | Treat this as an account/balance-related refusal, not evidence of a transient rate limit. No balance amount, billing endpoint or precise quota state was inspected. |
| NVIDIA / `nemotron-3-super` | Python SDK→binary SSE text/auto-tool/continuation passed; pi→binary text/read/continuation also passed, with wire/consumer equality | Nine successful requests across the selected current conditions and the final repaired-relay check. The historical 502 did not reproduce; no runtime fix or general stability claim follows. |
| MiMo / `mimo-v2.6-pro`, off→`none` | HTTP 200, `response.completed`, four answer characters classified as a case variant of `pong`; accumulated deltas, terminal snapshot and pi answer agreed | Exact lowercase oracle still fails. No lowercasing or output rewriting was applied. Evidence locates the mismatch in the delivered content, not pi loss/duplication. This was not a same-response raw-upstream packet capture. |
| MiMo, explicit `minimal` | Same literal oracle passed, followed by an actual read and result continuation; wire/consumer agreement throughout | Two three-request groups passed, including the final repaired-relay check. This is an explicit operator option, not a silent default change or proof that minimal always fixes output adherence. |
| DeepSeek, OpenRouter, LongCat, Zhipu | One pi text/read-continuation group each passed with the new same-response observation guard | Twelve requests. OpenRouter/Zhipu tool-result answers included extra formatting; the existing value-presence oracle passed while exact formatting remained a separate false metric. |

The count is two account checks + three NVIDIA SDK requests + one MiMo none request + six MiMo minimal requests + six NVIDIA pi requests + twelve other pi requests. The final six requests validate the repaired relay and exact, untrimmed consumer comparison in both protocol families; they were not used to force literal answers or to declare the unreproduced NVIDIA issue fixed.

## Same-response observation, not a new codec

[`provider_probe_observation.mjs`](../../../examples/provider_probe_observation.mjs) observes the gateway's already-produced SSE while forwarding the original bytes unchanged. It compares the terminal answer with the consumer's result and, for Responses, separately compares accumulated text deltas with the terminal snapshot. Reports contain closed text-shape categories, lengths, terminal/EOF facts and equality booleans, not body text, reasoning, opaque values or headers.

Budgets are 2 MiB wire, 1 MiB frame and 65,536 UTF-16 text code units. UTF-8 and CRLF delimiter fragmentation, raw frame-byte accounting, ordered part coordinates, missing/premature terminal, truncation and allocation limits have independent synthetic tests. A diagnostic failure fails the probe; this observer is not a replacement for the Rust protocol decoder or a full SSE conformance validator.

The local pi probe now requires exact agreement between untrimmed typed answer blocks and wire text, in addition to its original scenario oracle. An independent whitespace counterexample exposed that an initial trim-based comparison could hide consumer whitespace changes; that comparison was corrected and both protocol families were rechecked through the repaired relay. A quoted, differently cased or punctuated `pong` is diagnosed, **not accepted as exact lowercase output**. The `minimal` comparison is explicit and saved defaults remain unchanged.

## Reproduced local defect and repair: test-relay authentication

The pi diagnostic relay previously inserted the harness's known-good gateway token, regardless of the credential actually sent by pi. This could mask a wrong client credential and let an unauthenticated local caller reach the test gateway. It was a defect in the test relay, not an authentication bypass in OpenBridge's production Router.

The red synthetic gate sent an explicitly wrong token from the real pi SDK. The old relay replaced it: the response became 502 from the rejecting egress proxy and **one upstream connection attempt occurred**, instead of a 401 and zero attempts. No real Provider was contacted.

The repaired relay uses [`probe_http_headers.mjs`](../../../examples/probe_http_headers.mjs): forward only the client's actual Authorization and Content-Type, never substitute a good token or repair a missing header, and reject duplicate selected headers. Proxy/auth/routing headers outside that whitelist are not forwarded. The real gateway remains responsible for authentication and media admission.

After repair, separate Chat and Responses synthetic runs both observed:

- Wrong pi token: 401 on text and tool-definition requests, **zero upstream connection attempts**.
- Correct pi token: requests reached the rejecting loopback proxy, proving normal authentication/admission was not broken; no real egress.
- Pure header tests additionally protect missing values, duplicate/case variants and exclusion of other headers.

Normal-path model/protocol observations from earlier relayed calls remain evidence of those calls; they did not independently prove rejection of incorrect pi credentials. That negative boundary is established by the new synthetic gate.

## Reproduction and limits

Offline, without credentials or Provider access:

```sh
node --test tests/sdk/provider_probe_observation.test.mjs tests/sdk/probe_http_headers.test.mjs
```

The independent local launcher at `~/.pi/openbridge-test/README.md` documents `--check --check-invalid-auth` for the actual pi/Router negative gate and `--thinking minimal` for an explicit comparison. `--check` uses synthetic credentials and a rejecting loopback proxy; `--probe` makes new paid calls and requires a new scoped budget. The actual authentication assertions are local-harness checks, not part of default Rust tests.

Sanitized account facts are retained in ignored `testdata/runtime/provider-followup-JzCbjz/account-results.json`; the NVIDIA SDK result is in `testdata/runtime/provider-matrix-1790719811856950511/calls.json`. Pi wire facts were reported without raw bodies. The portable findings and evidence limits are maintained here, not by promoting captures into regression fixtures.

This batch does not resolve invalid credentials, change account balances, prove that a different reasoning setting is always more reliable, or establish load/long-run/TUI compatibility. The runtime's reported text remains authoritative. No private key files were modified, no production diagnostics were enabled, and all owned test services were closed.
