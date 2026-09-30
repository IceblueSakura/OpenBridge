# OpenBridge Agent Instructions

These instructions apply to the repository and all subdirectories. More specific guidance may narrow them, but must not weaken authorization, security, evidence, or change-control boundaries.

## Scope and Authorization

- OpenBridge's current checkout is the experimental v2 Rust semantic library and minimal loopback text gateway, not a production-ready service. The predecessor runtime, templates, corpus and dedicated tests are archived at the fixed Git ref in [archive.md](docs/archive.md). Ground work in current source/tests and v2 contracts; archived behavior is migration evidence, not a current feature or instruction to restore legacy code.
- Inspect the branch, `git status`, and the target-file diff before editing. Preserve unrelated work; do not overwrite, revert, stage, or commit it. Stop on overlapping external edits.
- Reviews, diagnosis, status, and planning are read-only. Implement explicitly requested changes within scope; a design-first step is not an extra approval gate when implementation is already authorized. Stop for unresolved material design choices, not an arbitrary phase boundary.
- Commit, push, external publication, service/production changes, and paid Provider requests require explicit authorization for the relevant action and target. A commit request does not authorize a push.
- ADRs record accepted decisions; [next-goal](docs/implementation-plans/next-goal.md) records direction and [current-focus](docs/implementation-plans/current-focus.md) records the approved implementation slice. None grants authority, proves implementation, or turns a design gap into a completed feature.

## Read the Relevant Context

Before non-trivial work, read the root [README](README.md) and [documentation index](docs/README.md), then follow the task-specific route:

| Task | Required context |
|---|---|
| Product behavior or compatibility | [v2 migration boundaries](docs/architecture-v2/migration.md), [Responses text admission](docs/architecture-v2/responses-text-profile.md), affected source/tests and fixed reference material |
| Cross-module ownership or data flow | [Current architecture](docs/architecture.md), [v2 design](docs/architecture-v2/README.md), affected module docs and callers |
| Task IR, media semantics, or encode/decode | [Semantic IR](docs/architecture-v2/semantic-ir.md), [thematic research](docs/references/semantic-baseline.md), [fixed upstream baseline](docs/references/upstream-sync.md), relevant standard/extension/media leaves, [protocol/lowering](docs/architecture-v2/protocol-and-lowering.md), and actual `src/semantic/`, `src/protocol/`, `src/lowering/` types/callers |
| Implementation, dependency, or test changes | [Development guide](docs/development.md), affected tests and manifests |
| Provider onboarding or protocol changes | Use the current-information lookup below, affected adapters/code comments/tests, and the relevant official API pages fetched when needed; fixed standard/SDK references remain separate from live Provider behavior |
| Corpus or semantic testing | [Development guide](docs/development.md), independent `tests/semantic/` and `tests/support/` fixtures and affected Rust contracts; archived corpus is evidence only, not an active test dependency |
| Documentation or instruction maintenance | Documentation responsibilities, canonical sources, incoming links, and affected guidance |

Read only relevant leaves, not every document. Product contracts state intended behavior, source/tests describe implementation, and executed evidence describes observations. Investigate conflicts rather than silently changing a contract to match code.

## Current Provider, Model and Compatibility Information

Do not maintain Provider/model inventories, per-model support tables, live-test results, account status or temporary failures in Markdown (including this file), source comments or memory as a substitute for a fresh lookup. Prefer code, nearby rationale comments and independent executable regressions. Documentation explains how to investigate, not what a changing catalog currently contains.

1. **Registered definitions:** read [`src/provider/catalog.rs`](src/provider/catalog.rs) for trusted origins, protocol paths and auth schemes; follow [`src/topology/catalog.rs`](src/topology/catalog.rs) through `default_topology`, Public Model → Route → Endpoint, upstream model IDs, representation contracts and execution limits. A declared Provider or `Dialect` alone is not a usable model binding.
2. **Startup admission:** read [`src/gateway/bootstrap.rs`](src/gateway/bootstrap.rs) and [`src/gateway/config.rs`](src/gateway/config.rs). Cross-check explicit entries, protocol family and credential activation with the compiled topology. Do not infer that every catalog model/protocol is exposed by a running instance, or that Chat compatibility implies Responses.
3. **Wire behavior:** read [`src/adapter/mod.rs`](src/adapter/mod.rs), [`src/protocol/adaptation.rs`](src/protocol/adaptation.rs), the owning request/response/event codecs and `src/lowering/`. Follow their comments and independent tests in `tests/semantic/` and `tests/transport/`; do not turn test names or prior pass counts into capability claims.
4. **Probe choices:** inspect [`examples/probe_support/catalog.py`](examples/probe_support/catalog.py), scenario code and each entry point before creating a plan. The probe's selectable subset/defaults are not the product catalog. See [probe operations](docs/probes.md) for dry-run, shared budgets and result interpretation.
5. **Running instance / upstream availability:** verify the actual binary revision and operator-approved non-secret activation metadata. Inspect the current router before assuming a discovery API exists: this checkout has no `GET /v1/models`. If safe instance metadata is unavailable, report activation as unknown; do not inspect private files or dump environment values to fill the gap. Official Models APIs list upstream offerings, not OpenBridge admission or successful inference. Re-read relevant official pages on demand; the [source index](docs/references/providers/README.md) is navigation, not a support list. Credential-bearing discovery, live/paid requests and service operations still need scoped authorization and bounded execution.

Safe, offline starting points (run from the repository root, then read the enclosing definitions and callers):

```sh
git status --short
rg -n 'pub fn|ProviderDefinition|EndpointPath|TrustedOrigin' src/provider/catalog.rs
rg -n 'pub fn|PublicModel|Route|Endpoint|API_KEY_BINDINGS|upstream|protocols' src/topology/catalog.rs
rg -n 'OPENBRIDGE_|catalog::|Entry|binding.protocols' src/gateway/bootstrap.rs src/gateway/config.rs
rg -n '\.route\(' src/gateway/http.rs
```

Report findings in the current conversation with source revision, exact scope and unverified layers. Keep authorized probe artifacts only in ignored local run directories; do not copy result matrices, payloads or transient incident status into docs/comments. When a finding justifies a code change, preserve the invariant, rationale and essential source URL near the owning code and write a minimal independent synthetic regression—not a narrative of the Provider session. Historical Git content is available for explicit historical questions, never as current support evidence.

## IR and Codec Work

- Generation is Responses-first: the fixed OpenAI standard supplies the semantic backbone, with scoped typed extensions for special capabilities. Do not reduce IR to the Chat/common subset or treat SDK DTOs as the IR. Standard fields are not generic extensions merely because local codecs have gaps.
- The overall representation includes task semantics, typed context, delivery, scoped extensions and fidelity. Session/context extensions are allowed, but selected runtime endpoints, credentials, sockets and retry/commit state are not semantic data. Follow the canonical design for attachment, provenance, lifecycle and target checks rather than inventing a second schema here.
- Check the affected task's supported semantics, IR ownership, request/response codecs, and failure boundaries before choosing a field-by-field migration. Distinguish type expressiveness, codec mapping, and production wiring; a type or passing test alone proves none of the other layers. Establish semantic ownership and design admission before expanding the conformance fixtures.
- Keep task, wire protocol, and modality separate. Follow the task-family design rather than forcing Embedding, Images, or dedicated Speech semantics into GenerationRequest. Shared resource values must retain task-specific meaning; Chat wire does not imply conversation history.
- Follow the accepted target ordering: resolve the fixed Public Model task contract before semantic decode, process task IR before Provider selection, and keep registry/credentials/network outside pure codecs. Verify the current call path before changing it; the ADR is not a description of already completed wiring.
- Treat typed semantics as authoritative. Source metadata may preserve equivalent representation, not restore deleted values or reattach metadata to the wrong item. Check omission/null/empty/default distinctions, media metadata, numeric precision, and source-bound extensions against the affected contract. Do not mistake a narrower v2 codec for full preservation of archived Native behavior.
- Validate request/response closure and Static/Event consistency, not only request encoding. Derive requirements after semantic changes; each candidate must project independently from immutable input without changing fixed Route order or capability intersections.
- Do not introduce speculative hooks, a generic plugin framework, new task support, or arbitrary cross-Provider conversion while closing IR ownership gaps. Detailed task semantics and design choices belong in the ADRs, task contracts, and source docs, not a second schema in this file.

## Change Discipline

- Keep changes focused on the requested result. Split modules by responsibility or independent protocol domain, not line count; keep facades small and preserve intended public crate paths through explicit re-exports.
- Keep startup registration separate from request-time planning, analyzers separate from registry resolution, and pure response policy separate from body I/O, observation, and downstream commit. Detailed module ownership belongs in the architecture and source docs.
- Before an approved behavior change, record the observable result, requirement, failing test, non-goals, and validation boundary in the current focus; then use TDD. Pure instruction/comment/documentation maintenance does not require a manufactured behavior focus.
- A breaking change must update implementation, parsing/serialization, OpenAPI, examples, fixtures, docs, and tests together. Unpublished prototype APIs may be replaced within approved scope without legacy aliases, compatibility shims, or meaningless schema bumps.
- Keep dependencies intentional. Update `Cargo.lock` with `Cargo.toml` and repeat locked validation.
- Follow the development guide for code-comment conventions. The minimal binary uses the documented environment bootstrap, not archived service configuration templates; private files left in `config/` are not a source of test data. Rust comments/docs and Python docstrings use concise English; document non-obvious protocol, security, concurrency, cleanup, and failure boundaries.

## Security and Resource Boundaries

- Never print, copy, commit, or place real keys, passwords, bearer tokens, private configuration, sensitive production bodies, or credential values in code, fixtures, comments, logs, docs, or tool output. Synthetic non-sensitive fixtures are allowed.
- Treat `.env`, `config/users.toml`, `config/upstream-credentials.toml`, and OAuth auth files as private. Use only explicitly authorized synthetic examples for shape; old tracked configuration templates are archived. Do not discover or import third-party applications' auth caches.
- Keep the listener loopback-only and egress statically trusted. Business requests must not select upstream URLs, credentials, authentication/proxy headers, or transformation scripts. Preserve the separation of safe and sensitive headers; do not expose credential locators or trusted origins downstream.
- Preserve fail-closed authentication, credential ownership, bounded allocation/capture, protocol terminal, retry/fallback/cooldown, cancellation, and resource-lifetime behavior. Do not buffer without bounds, replay after downstream commit, or fabricate a successful terminal.
- Content logging starts only after downstream authentication, observes the final downstream boundary, and always redacts sensitive headers. It is not an upstream wire dump. Bounded content snapshots remain in the dedicated local JSONL sink, absent from stdout and reviewed OTLP traces; sink failure must not change business responses.
- Checked-in logging profiles are for controlled development. Do not run them against sensitive traffic merely to verify logging; production owners must reduce or disable content capture first.
- The corpus/testkit must not load OpenBridge credentials, call a real Provider, implicitly start OpenBridge, or implement automatic retry/fallback. Explicit Rust bootstrap tests may own a disposable loopback binary with synthetic keys, an isolated environment and a rejecting loopback egress proxy. Live, paid, ignored network, and external-dependency checks require explicit approval; paid probes also require an agreed target, exact request matrix, output limits, and sanitized report boundary.
- Do not inspect or manually edit `target/`, `tools/corpus/.venv/`, `tools/corpus/.pytest_cache/`, or Python `__pycache__/` unless explicitly targeted. Normal build/test tools may populate their own caches; do not treat generated output as source.
- Do not commit derived output under `testdata/generated/`, `testdata/reports/`, `testdata/dist/`, or `testdata/runtime/`. Canonical `testdata/` files are contracts; change wire data only for requested behavior and do not add comments to formats that forbid them.

## Tests and Verification

- A new test must protect a distinct client-visible semantic result, wire behavior, or security/resource failure boundary. A new Model, Route, Provider instance, or catalog value alone does not justify a test.
- Test each mechanism at its lowest owning layer; add at most one production-Router smoke test when it adds independent value. Avoid complete inventories, capability snapshots, incidental DTO snapshots, Route/candidate inventory assertions, and repeated per-model matrices. Assertions on intentional IR semantics and routing mechanisms are valid; do not confuse them with implementation snapshots.
- For codecs, verify wire-to-IR and IR-to-wire against independent expected results, then test insertion/replacement/deletion and applicable stream/terminal failures. Round trips alone can hide symmetric loss. Provider-independent means offline and account-independent, not protocol/profile-independent.
- Reuse canonical fixtures and admit external samples only after version, license, sensitivity, and independent-oracle review. Prefer small deterministic media and vectors over model-quality datasets. Keep Rust runtime contracts distinct from Python corpus/tooling; do not force all tasks into the current semantic-case schema or build a speculative test framework.
- Run focused checks first, then the proportionate baseline in [development.md](docs/development.md). Rust changes use `cargo fmt -- --check`, `cargo test --locked --offline`, and `cargo clippy --locked --offline --all-targets -- -D warnings`; SDK loopback is a separate explicit gate. Prose-only edits need structural/link checks and `git diff --check`, not a manufactured runtime run. Compilation and test existence do not prove changed behavior.
- Parallelize only independent scenarios with isolated paths, ports, and outputs. Keep ordered retry/fallback/cancellation scenarios serial; use readiness/events and bounded timeouts, not sleeps to hide races.
- Deterministic tests do not prove live Provider, SDK/Agent, TLS/network, load, long-run, or production compatibility. State each unexecuted layer explicitly.

## Documentation and Completion

- Prefer code and comments as implementation documentation. Follow [docs/README.md](docs/README.md) for the remaining stable material: architecture explains cross-module ownership, ADRs explain accepted decisions, implementation status tracks semantic/design gaps, and next-goal gives direction. Keep exact mappings, defaults and failure invariants beside their owning code/tests; do not mirror registrations or maintain a second schema in Markdown.
- Maintain stable research by semantic topic under [references](docs/references/README.md), preserving necessary source URLs, fixed standard/SDK versions and licenses. Do not add or retain Provider acceptance reports, model inventories, per-Provider API snapshots or live-test completion diaries, even when dated. Query volatile upstream facts when needed; dated standard/design research is not a current compatibility claim. Keep test outcomes in the task response and authorized ignored artifacts, not a new evidence page. Do not refresh external verification dates during local cleanup.
- Update affected architecture, decisions, goals and concrete limitations, not completion diaries. Restore the current focus to empty after completing the approved behavior slice; update the next goal separately rather than erasing an unfinished direction.
- Inspect the final diff and verify relative links/anchors and `git diff --check` for documentation changes. The old OpenAPI and Swagger runtime assets are archived with their service; introducing new HTTP schema assets requires an authorized interface contract, not copying predecessor claims.
- Report what changed, the files involved, exact checks and outcomes, skipped external layers, and remaining acceptance gaps. Do not claim runtime success from documentation edits or improved agent behavior from static instruction review alone.
