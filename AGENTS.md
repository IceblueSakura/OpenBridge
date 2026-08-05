# OpenBridge Agent Instructions

This file applies to the repository root and every directory below it. A more specific `AGENTS.md` in a subdirectory may
add or narrow rules for that subtree, but it must not weaken repository-wide security, credential, or evidence
boundaries.

## Project Scope

OpenBridge is an experimental Rust/Axum, headless, OpenAI-compatible multi-provider gateway. Treat the current checkout
as the authority for implementation facts; do not infer current behavior from historical plans, previous task summaries,
or the mere existence of an unfinished prototype.

Before starting work, inspect the live source, relevant tests, documentation, and `git status`. Preserve unrelated
worktree changes and keep the requested change surface narrow.

The main repository areas are:

- `src/models/`: canonical, provider-independent model facts.
- `src/providers/`: compiled provider adapters, upstream targets, and upstream API registration.
- `src/registry/`: typed definitions, validation, and immutable registry snapshots.
- `src/ingress/`, `src/pipeline/`, `src/provider/`, and `src/transport/`: request admission, planning, provider
  contracts, HTTP transport, and SSE lifecycle handling.
- `tests/`: deterministic Rust contract and compatibility fixtures.
- `testdata/`: canonical protocol corpus and schemas.
- `tools/corpus/`: the independent Python mock/testkit tooling for the protocol corpus.

## Sources of Truth

Read [README.md](README.md) and [docs/README.md](docs/README.md) before making non-trivial behavioral or architectural
changes. Route documentation updates according to their meaning:

- Product behavior, compatibility promises, boundaries, and non-goals belong in `docs/functional-requirements/`.
- Current implementation facts and completed validation evidence belong in `docs/implementation-status/`.
- The single approved short-cycle behavior belongs in `docs/implementation-plans/current-focus.md`.
- External protocol, SDK, client, and reference-project facts belong in `docs/references/`.
- Local implementation rationale belongs in module or API documentation unless it affects a cross-cutting product or
  architectural contract.

Do not create standalone future-design documents, phase roadmaps, decision-history files, or speculative plans. Follow
the maintenance rules in `docs/README.md`.

## Development Workflow

- Treat read-only review, diagnosis, and planning requests as read-only. Do not turn them into implementation,
  documentation mutation, or commits without explicit authorization.
- Before an approved behavior change, follow `docs/implementation-plans/README.md`: define one observable behavior, its
  requirement, a failing test, explicit non-goals, and the validation boundary in `current-focus.md`.
- Use TDD for behavior changes: write or identify a failing test, implement the smallest coherent change that makes it
  pass, and then run proportionate regression checks.
- After a completed behavior change, update the implementation-status documentation with confirmed facts and actual
  validation evidence, then return `current-focus.md` to its empty state.
- Do not broaden a narrow fix into adjacent refactoring, lifecycle changes, provider expansion, configuration migration,
  or documentation work unless the user requested that scope.
- Do not overwrite, revert, stage, or commit unrelated worktree changes. Do not commit, push, or open a pull request
  unless explicitly requested.
- Keep dependency changes intentional. When `Cargo.toml` changes, update `Cargo.lock` consistently and repeat locked
  validation.

Pure documentation maintenance, comment-only work, and read-only analysis do not require manufacturing an implementation
focus unless they change a product behavior or compatibility commitment.

## Validation

Run checks in proportion to the changed surface. The default Rust validation baseline is:

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

When changing `testdata/` or `tools/corpus/`, also run the relevant corpus checks from the repository root:

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

Apply these evidence rules:

- A documentation-only or instruction-only change normally requires link/content inspection and `git diff --check`, not
  a full runtime test suite.
- 新的客户端可见测试优先使用 OpenAI SDK、独立 Python 脚本或 curl 等协议级入口；除非当前行为明确以某个 Agent
  客户端为兼容目标，否则不得把默认验收绑定到 Codex、Hermes 或其他客户端 runtime。
- Deterministic Rust tests and corpus loopback tests do not prove real Provider, current external SDK, load, long-run,
  or production compatibility.
- The ignored SDK compatibility test, real Provider calls, external dependency installation, and network-dependent
  verification are not part of the default baseline. Run them only when the task explicitly requires them or when the
  user approves the expanded boundary.
- Report exactly which checks ran, which passed, and which were skipped. Never describe an unrun acceptance layer as
  validated.

### Rust/Python test ownership

- Keep `testdata/` and `tools/corpus/` inside this repository as a runtime-independent `uv + Python` project. Do not
  extract, publish, or add cross-project compatibility layers until there is a real second consumer or the user
  explicitly requests that scope.
- Rust tests own OpenBridge runtime behavior: registry and routing, Provider contracts, retry/fallback/cooldown,
  cancellation, Protocol Bridge state, and other in-process invariants. When a Rust state machine can consume a
  canonical fixture directly, prefer `include_bytes!` or an equivalent read-only fixture replay over invoking Python.
- Python tests own corpus integrity and tooling behavior: schema/provenance/secret validation, deterministic
  generation/report/pack, byte-level SSE fragmentation, standalone HTTP Mock Server/Client behavior, redacted
  observations, and canonical observation comparison. Do not reimplement OpenBridge routing or retry/fallback policy in
  Python.
- `testdata/` is the shared protocol contract between the two layers. A fixture existing in the corpus is not runtime
  evidence; a Rust replay proves only the directly exercised in-process behavior; a Python loopback proves only the
  standalone testkit behavior.
- When Rust and Python tests overlap, keep one canonical wire fixture and place each assertion in the closest owning
  layer. Remove a duplicate helper or mock only after the replacement covers the same observable behavior and failure
  boundary.
- The Python testkit must expose stable CLI/data contracts for this repository, not a speculative general-purpose
  framework. Avoid plugin systems, dynamic control planes, generic adapters, or independent release machinery without a
  demonstrated current requirement.

### Test duration and parallelism

- Keep the default developer feedback path small: run the focused Rust test first, then the Rust baseline; add the
  Python baseline only when `testdata/` or `tools/corpus/` changes or when the focused behavior explicitly crosses that
  boundary.
- Independent Python cases may run in parallel only when each worker owns its temporary directory, Mock Server, random
  port, and runtime outputs. Never let parallel workers share or rewrite `testdata/generated/`, `testdata/reports/`,
  `testdata/dist/`, or `testdata/runtime/`.
- Ordered server suites, multi-attempt retry/fallback scenarios, cancellation timing, and other causally ordered tests
  must remain deterministic and serial within the scenario. Parallelize independent scenarios, not exchanges inside one
  ordered suite.
- Do not add sleeps to make concurrency tests pass. Use explicit readiness, events, bounded timeouts, and deterministic
  observations. Treat load, throughput, fairness, and long-run concurrency as separate acceptance layers, not default
  contract tests.

## Security and Trust Boundaries

- Never print, copy, commit, or place real API keys, passwords, bearer tokens, private user configuration, credential
  values, or sensitive production request bodies in code, comments, fixtures, logs, documentation, or tool output.
  Synthetic protocol fixtures are allowed when they contain no real or sensitive data.
- Treat `.env` and `config/users.toml` as private local files. Prefer `.env.example` and `config/users.example.toml`
  when inspecting or documenting configuration shape.
- Preserve the static trusted-egress design: business requests must not select an arbitrary upstream URL, credential,
  authentication header, proxy header, or transformation script.
- Keep safe headers and sensitive headers separated. Preserve redaction and avoid exposing credential locators, trusted
  origins, or internal routing details through downstream APIs or diagnostics.
- The protocol corpus and testkit must remain runtime-independent: they must not load OpenBridge credentials, call a
  real Provider, start the OpenBridge runtime implicitly, or perform automatic retry/fallback.

## Generated, Derived, and Private Files

Do not inspect or edit generated environments, build output, or caches unless the task explicitly targets them:

- `target/`
- `tools/corpus/.venv/`
- `tools/corpus/.pytest_cache/`
- Python `__pycache__/` directories

The following corpus outputs are derived and must not be committed:

- `testdata/generated/`
- `testdata/reports/`
- `testdata/dist/`
- `testdata/runtime/`

Canonical files under `testdata/` are contract source files, not general scratch space. Modify them only when the
requested protocol behavior or corpus case requires it. Do not add comments to JSON, SSE, or other wire fixtures unless
their format and contract explicitly permit comments.

## Code Comment Guidelines

### Language

- Write source-code comments, API documentation, and Python docstrings in concise English. English is the
  repository-wide comment language.
- Keep code identifiers, protocol field names, type names, and established technical terms such as `Provider`, `SSE`,
  and `fallback` in their original English form.
- Use complete, descriptive sentences. Keep comments short and current.

### Required comment coverage

- Add a `//!` module-level comment to each non-trivial Rust module. Describe the module's responsibility, important
  boundaries, and any security or protocol constraints.
- Add `///` Rustdoc comments to public types, functions, methods, constants, fields, and enum variants. Document purpose
  and, when relevant, inputs, outputs, errors, side effects, ownership, security constraints, and protocol limitations.
- Add English docstrings to non-trivial Python modules and public functions under `tools/corpus/`. Document
  responsibilities, inputs, outputs, exceptions, and observable side effects when relevant.
- Apply the same standard to private helpers that implement non-obvious protocol rules, boundary validation, resource
  cleanup, concurrency coordination, or error propagation.
- New or materially modified functions must comply fully. Update comments within the directly changed function or
  module, but do not perform repository-wide or unrelated comment-only rewrites unless explicitly requested.

### Function-body stage comments

- Every non-trivial function with two or more logical stages must have a single-line descriptive comment immediately
  before each stage.
- A stage is a coherent responsibility boundary, not every branch, loop, or group of adjacent statements.
- Treat request parsing, input validation, data loading, transformation, business decisions, external side effects,
  result construction, and response delivery as separate stages when they occur separately.
- Start stage comments with an action verb and describe the intent of the stage, not the syntax of the following
  statement.
- Keep each stage comment on one line. Do not combine unrelated stages under one comment.
- A trivial one-stage getter, constructor, conversion, or delegation function does not require a redundant body comment
  when its API documentation and code are already self-explanatory.

Example:

```rust
/// Validates a login request and returns the authentication result.
pub fn login(request: LoginRequest, database: &Database) -> Result<Response, AuthError> {
    // Parse the request and extract the account credentials.
    let credentials = parse_credentials(request)?;

    // Load the account record and password digest.
    let account = database.find_account(&credentials.account)?;

    // Verify the password and normalize authentication failures.
    verify_password(&credentials.password, &account.password_hash)?;

    // Build and return the successful response.
    Ok(build_login_response(account))
}
```

### Complex logic

- Prefer module documentation, API documentation, or the source-of-truth document category defined above for complex
  protocols, state machines, retry and fallback rules, security reasoning, and multi-step data transformations.
- Keep function bodies focused on concise stage comments and critical invariants. Link to the relevant document when the
  implementation depends on a larger behavioral contract.
- Explain why a branch, loop, retry, cancellation path, or resource boundary exists when that reason is not evident from
  the code.
- Prioritize comments around retry, fallback, cancellation propagation, sensitive-data handling, SSE terminal behavior,
  state affinity, and capability gates.

Example:

```rust
// The downstream body has started; do not append a second upstream response.
if response_started {
    return forward_stream_error(error);
}
```

### Avoid

- Do not translate obvious assignments, returns, or simple control flow into comments.
- Do not narrate code line by line or place long design discussions inside function bodies.
- Do not leave speculative future designs or untracked "fix later" comments in source code.
- Do not include real API keys, passwords, tokens, request bodies, or other sensitive values in comments, examples, or
  debug text.
- Do not preserve stale comments. A change is incomplete if its comments contradict the current implementation.

### Comment review checklist

Before completing a code change, verify that:

- Module responsibilities and boundaries are documented in English.
- Public APIs document their purpose and relevant errors, results, side effects, and constraints.
- Every logical stage in each non-trivial function has a concise single-line English comment.
- Non-obvious branches and invariants explain why the boundary exists.
- Complex reasoning is documented outside the function body when appropriate.
- Comments contain no sensitive data and match the current implementation.

## Handoff Requirements

In the final handoff, state:

- The behavior or documentation changed and the files involved.
- The validation commands actually run and their results.
- Any relevant checks, SDK tests, real Provider tests, load tests, or long-run tests that were not run.
- Any remaining user action or acceptance boundary.

Keep confirmed implementation facts, plans, static checks, deterministic tests, and external acceptance results
explicitly separated.
