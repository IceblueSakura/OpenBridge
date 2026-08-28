# OpenBridge Agent Instructions

This file applies to the repository root and all subdirectories. A more specific `AGENTS.md` may narrow these rules but
must not weaken repository-wide security, credential, evidence, or change-control boundaries.

## Operating Principles

- OpenBridge is an experimental Rust/Axum, headless, OpenAI-compatible multi-provider gateway. Treat the live checkout,
  relevant tests, and maintained documentation as authority; historical plans and prior summaries are only routing aids.
- Before non-trivial work, inspect `README.md`, `docs/README.md`, affected source/tests/docs, and `git status`. Preserve
  unrelated worktree changes and keep the requested surface narrow.
- Review, diagnosis, status, and planning requests are read-only. Do not turn them into implementation, documentation
  changes, commits, pushes, or external actions without authorization.
- Do not overwrite, revert, stage, or commit unrelated work. Commit, push, and open pull requests only when explicitly
  requested; a commit request never implies a push.

## Architecture Boundaries

See `docs/implementation-status/current-architecture.md` for the current module map. Preserve these ownership rules:

- `src/config/` owns strict Bootstrap parsing and startup process policy. `src/models/`, `src/providers/`, and
  `src/registry/` own canonical Model facts, trusted Provider registration, validation, and immutable runtime snapshots.
- `src/ingress/`, `src/pipeline/`, `src/provider/`, and `src/transport/` own admission, request analysis/planning,
  Provider contracts, HTTP transport, body lifecycle, and SSE handling. Business requests must not select arbitrary
  upstream URLs, credentials, authentication headers, proxy headers, or transformation scripts.
- Split modules by ownership or independent protocol domain, not line count. Keep multi-responsibility roots as small
  facades and preserve public crate paths through explicit re-exports.
- `core/capability.rs` only combines domains at `ApiCapabilities`; generation rules belong in
  `core/capability/generation.rs`, Embeddings input/encoding/dimension/limit rules in
  `core/capability/embeddings.rs`, and Images generation rules in `core/capability/images.rs`.
- `pipeline/generation/`, `pipeline/embeddings/`, and `pipeline/images/` each own their operation analyzer, preflight,
  planner, and pure response policy behind `pipeline/mod.rs` re-exports. Analyzers extract request facts only; they must
  not resolve registry entities or select Routes. Response policy must not perform body I/O, observation, or downstream
  commit.
- `registry/public_model.rs` is the facade for downstream-safe Models DTOs and preflight accessors. Operation DTOs and
  media algebra live in `public_model/*`; private execution interfaces, startup compilation, contribution, aggregation,
  and Embeddings response-budget narrowing remain in their dedicated leaves. Never serialize execution topology or move
  request-time routing into compiler modules.
- `observability.rs` is a facade. `request.rs` owns downstream lifecycle, `request/content.rs` local snapshot policy,
  `provider.rs` attempt observation,
  `metrics.rs`/`otlp.rs` SDK export, and `http_jsonl/` sanitized local snapshots. Authentication/wiring remains in
  `ingress/router.rs`; bounded body capture remains in `ingress/lifecycle.rs`.
- Provider family roots aggregate trusted registration modules; developer roots aggregate explicit per-model leaves.

## Documentation and Change Workflow

Route maintained facts by meaning:

- Product behavior, compatibility, boundaries, and non-goals: `docs/functional-requirements/`.
- Confirmed implementation facts and actual validation evidence: `docs/implementation-status/`.
- The one approved short-cycle behavior: `docs/implementation-plans/current-focus.md`.
- External protocol, SDK, client, and reference-project facts: `docs/references/`.
- Local implementation rationale: module/API documentation unless it changes a cross-cutting contract.

For model information that is directly available from an official model page or OpenRouter, record the source URL,
source identity, and recheck boundary instead of copying the complete metadata payload or capability table. Current
Model-to-Provider/Target/Public Model relationships may be documented as implementation mappings, but capability facts
belong in `src/models/`, `src/providers/`, the runtime extended Models API, or the external source. Create a separate
dated evidence record only when an executed test contradicts the cited official/OpenRouter claim; preserve the exact
source claim, observed delta, endpoint, model ID, payload boundary, account/region/network boundary, and what the test
does not prove. Do not promote a directory disagreement or untested inference into a verified discrepancy.

Do not create speculative future-design, roadmap, or decision-history documents. Follow `docs/README.md` maintenance
rules.

- Before an approved behavior change, define its observable behavior, requirement, failing test, explicit non-goals,
  and validation boundary in `current-focus.md`; then use TDD.
- A breaking change must update every affected source of truth atomically: implementation, parsing/serialization,
  OpenAPI, examples, fixtures, docs, and tests. Leave private configuration and user data untouched.
- Within the approved focus, unpublished prototype APIs and bootstrap fields may be replaced rather than preserved with
  legacy aliases, compatibility shims, deprecation windows, or meaningless schema bumps.
- After completion, record confirmed facts and commands in implementation status, then restore `current-focus.md` to
  its empty state. Do not expand a narrow task into adjacent refactoring or provider/configuration work.
- Keep dependency changes intentional. Update `Cargo.lock` with `Cargo.toml` and repeat locked validation.
- Pure instruction, comment, or documentation maintenance does not require a manufactured implementation focus unless
  it changes product behavior or a compatibility commitment.

## Test Governance

- Every new test must protect a distinct client-visible result, Provider wire behavior, or security/resource failure
  boundary. A new Model, Route, Provider instance, or catalog-only capability value does not by itself justify a test.
- Do not maintain complete Model/Provider inventories, capability snapshots, Route IDs/count/order, candidate counts,
  compiler/planner intermediate DTOs, or repeated per-model acceptance matrices in tests.
- Test one mechanism at its lowest owning layer and add at most one production-Router smoke test when that boundary adds
  independent value. Prefer deleting duplicate coverage over replacing it with a more elaborate parameterized harness.
- Keep authentication, credential secrecy/ownership, bounded allocation, protocol terminal, retry/fallback/cooldown,
  cancellation, and resource-lifetime failures covered through their real fail-closed boundary.

## Bootstrap and Local HTTP Logging

- `config/bootstrap.toml` and `config/bootstrap.example.toml` are checked-in development profiles. They must parse to
  the same `BootstrapConfig`, and every assignment in both files must have an immediately preceding concise English
  comment describing its runtime effect.
- `[logging]` owns `http_jsonl_directory`, `request_headers`, `request_body`, `response_headers`, and `response_body`. Checked-in
  development profiles explicitly set all four to `true`; an omitted table or field parses as `false`. Keep this
  distinction explicit in README, requirements, status docs, and tests.
- Content logging starts only after downstream Bearer authentication and observes the final downstream client boundary;
  it excludes anonymous failures and is not a raw upstream Provider wire dump.
- Header snapshots must redact authentication, Cookie, token, key, secret, password, session, credential, and signature
  values before tracing. Never add a switch that disables redaction or exposes upstream credential headers.
- Request and response captures are bounded by `max_request_body` and `max_json_response_body`. Emit at most
  one terminal snapshot per direction with captured/observed bytes, completeness, and truncation; never buffer without
  bounds or log per SSE chunk.
- Content snapshots use a bounded dedicated JSONL writer and must remain absent from stdout and the reviewed OTLP trace layer.
  Writer failure may drop snapshots with diagnostics but must not change business responses.

## Security, Private Data, and Generated Files

- Never print, copy, commit, or place real API keys, passwords, bearer tokens, private user configuration, credential
  values, or sensitive production request bodies in code, comments, fixtures, logs, docs, or tool output. Synthetic
  fixtures are allowed only when they contain no real or sensitive data.
- Treat `.env`, `config/users.toml`, and `config/upstream-credentials.toml` as private. Prefer checked-in examples for
  shape; preserve the separation between safe and sensitive headers and do not expose credential locators or trusted
  origins.
- Keep the service listener loopback-only and preserve static trusted egress.
- The checked-in logging profile is for controlled development, not production. Do not start it against sensitive
  traffic merely to test logging; production owners must reduce or disable content logging before sensitive use.
- The protocol corpus/testkit must not load OpenBridge credentials, call a real Provider, implicitly start OpenBridge,
  or implement automatic retry/fallback.
- Do not inspect or edit `target/`, `tools/corpus/.venv/`, `tools/corpus/.pytest_cache/`, or Python `__pycache__/` unless
  explicitly targeted.
- Never commit derived corpus output under `testdata/generated/`, `testdata/reports/`, `testdata/dist/`, or
  `testdata/runtime/`. Canonical `testdata/` files are contracts; modify them only for requested protocol behavior, and
  do not add comments to wire formats that do not support them.

## Validation and Evidence

Run focused tests first, then the proportionate Rust baseline:

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

For Bootstrap logging schema/defaults, body lifecycle, redaction, or OTLP exclusion, start with:

```powershell
cargo test --locked --test config_contract
cargo test --locked --test example_config
cargo test --locked --test observability_contract
cargo test --locked --test otlp_trace_contract
```

When `testdata/` or `tools/corpus/` changes, also run:

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

- Instruction/documentation-only changes normally need content/link inspection and `git diff --check`, not runtime
  tests. Report exactly what ran and what was skipped.
- Prefer OpenAI SDK, independent Python, or curl protocol entry points for new client-visible acceptance tests. Do not
  bind default acceptance to Codex, Hermes, or another Agent runtime unless it is an explicit compatibility target.
- Rust tests own OpenBridge runtime behavior: registry/routing, Provider contracts, retry/fallback/cooldown,
  cancellation, Protocol Bridge, and in-process invariants. Python owns corpus integrity/tooling, deterministic
  generation/report/pack, byte-level SSE fragmentation, and standalone mock/client behavior.
- Keep one canonical wire fixture when layers overlap; place assertions in the closest owner. Do not turn the Python
  testkit into a speculative general framework.
- Deterministic Rust tests and Python loopback tests do not prove real Provider, current external SDK, load, long-run,
  production logging, or physical/external-system compatibility. Run ignored, network, external-dependency, or
  live-provider validation only when explicitly required or approved.
- Parallelize only independent scenarios with isolated temp directories, ports, servers, and outputs. Keep ordered
  retry/fallback/cancellation scenarios serial, use readiness/events and bounded timeouts, and never add sleeps to make
  concurrency tests pass.

## Code Comments

- Write Rust comments/docs and Python docstrings in concise English. Keep identifiers and established terms such as
  Provider, SSE, Route, and fallback unchanged.
- Add `//!` responsibility/boundary documentation to non-trivial Rust modules and `///` documentation to public APIs.
  Document non-obvious private helpers that enforce protocol, security, cleanup, concurrency, or error boundaries.
- A non-trivial function with multiple logical stages needs one concise action-led comment before each stage. Explain
  why non-obvious retry, fallback, cancellation, sensitive-data, SSE terminal, state-affinity, and capability branches
  exist.
- Do not narrate obvious code, leave speculative TODOs, embed long design discussions, expose sensitive values, or keep
  stale comments. Update comments in the directly changed surface without unrelated repository-wide rewrites.

## Handoff

State the behavior or documentation changed, files involved, commands actually run and their results, skipped SDK/live
Provider/load/long-run layers, and any remaining user action or acceptance boundary. Keep implementation facts, static
checks, deterministic tests, and external acceptance clearly separated.
