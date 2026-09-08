# OpenBridge Agent Instructions

These instructions apply to the repository and all subdirectories. More specific guidance may narrow them, but must not weaken authorization, security, evidence, or change-control boundaries.

## Scope and Authorization

- OpenBridge is an experimental Rust/Axum, headless, OpenAI-compatible multi-provider gateway. Ground work in the live checkout, applicable contracts, source, and tests; history and summaries are navigation aids, not proof.
- Inspect the branch, `git status`, and the target-file diff before editing. Preserve unrelated work; do not overwrite, revert, stage, or commit it. Stop on overlapping external edits.
- Reviews, diagnosis, status, and planning are read-only. Implement explicitly requested changes without expanding scope. Commit, push, external publication, service/production changes, and paid Provider requests require explicit authorization for the relevant action and target.
- `docs/implementation-plans/current-focus.md` records user-approved behavior scope; it does not grant authority. Requirements, gaps, references, historical plans, and agent-written entries never independently authorize work.

## Read the Relevant Context

Before non-trivial work, read the root [README](README.md) and [documentation index](docs/README.md), then follow the task-specific route:

| Task | Required context |
|---|---|
| Product behavior or compatibility | Relevant `docs/functional-requirements/` leaves, affected source/tests, and current status boundaries |
| Cross-module ownership or data flow | [Current architecture](docs/architecture.md), affected module docs and callers |
| Implementation, dependency, or test changes | [Development guide](docs/development.md), affected tests and manifests |
| Provider onboarding or protocol changes | Relevant `docs/references/` source snapshots, registration code, Provider status and evidence |
| Corpus or semantic testing | `testdata/README.md`, `testdata/semantic-testing.md`, and relevant `tools/corpus/` source/tests |
| Documentation or instruction maintenance | Documentation responsibilities, canonical sources, incoming links, and affected guidance |

Read only relevant leaves, not every document. Product contracts state intended behavior, source/tests describe implementation, and executed evidence describes observations. Investigate conflicts rather than silently changing a contract to match code.

## Change Discipline

- Keep changes focused on the requested result. Split modules by responsibility or independent protocol domain, not line count; keep facades small and preserve intended public crate paths through explicit re-exports.
- Keep startup registration separate from request-time planning, analyzers separate from registry resolution, and pure response policy separate from body I/O, observation, and downstream commit. Detailed module ownership belongs in the architecture and source docs.
- Before an approved behavior change, record the observable result, requirement, failing test, non-goals, and validation boundary in the current focus; then use TDD. Pure instruction/comment/documentation maintenance does not require a manufactured behavior focus.
- A breaking change must update implementation, parsing/serialization, OpenAPI, examples, fixtures, docs, and tests together. Unpublished prototype APIs may be replaced within approved scope without legacy aliases, compatibility shims, or meaningless schema bumps.
- Keep dependencies intentional. Update `Cargo.lock` with `Cargo.toml` and repeat locked validation.
- Follow the development guide for configuration-template and code-comment conventions. Rust comments/docs and Python docstrings use concise English; document non-obvious protocol, security, concurrency, cleanup, and failure boundaries.

## Security and Resource Boundaries

- Never print, copy, commit, or place real keys, passwords, bearer tokens, private configuration, sensitive production bodies, or credential values in code, fixtures, comments, logs, docs, or tool output. Synthetic non-sensitive fixtures are allowed.
- Treat `.env`, `config/users.toml`, `config/upstream-credentials.toml`, and OAuth auth files as private. Use checked-in examples for shape. Do not discover or import third-party applications' auth caches.
- Keep the listener loopback-only and egress statically trusted. Business requests must not select upstream URLs, credentials, authentication/proxy headers, or transformation scripts. Preserve the separation of safe and sensitive headers; do not expose credential locators or trusted origins downstream.
- Preserve fail-closed authentication, credential ownership, bounded allocation/capture, protocol terminal, retry/fallback/cooldown, cancellation, and resource-lifetime behavior. Do not buffer without bounds, replay after downstream commit, or fabricate a successful terminal.
- Content logging starts only after downstream authentication, observes the final downstream boundary, and always redacts sensitive headers. It is not an upstream wire dump. Bounded content snapshots remain in the dedicated local JSONL sink, absent from stdout and reviewed OTLP traces; sink failure must not change business responses.
- Checked-in logging profiles are for controlled development. Do not run them against sensitive traffic merely to verify logging; production owners must reduce or disable content capture first.
- The corpus/testkit must not load OpenBridge credentials, call a real Provider, implicitly start OpenBridge, or implement automatic retry/fallback. Live, paid, ignored network, and external-dependency checks require explicit approval; paid probes also require an agreed target, exact request matrix, output limits, and sanitized report boundary.
- Do not inspect or edit `target/`, `tools/corpus/.venv/`, `tools/corpus/.pytest_cache/`, or Python `__pycache__/` unless explicitly targeted.
- Do not commit derived output under `testdata/generated/`, `testdata/reports/`, `testdata/dist/`, or `testdata/runtime/`. Canonical `testdata/` files are contracts; change wire data only for requested behavior and do not add comments to formats that forbid them.

## Tests and Verification

- A new test must protect a distinct client-visible result, Provider wire behavior, or security/resource failure boundary. A new Model, Route, Provider instance, or catalog value alone does not justify a test.
- Test each mechanism at its lowest owning layer; add at most one production-Router smoke test when it adds independent value. Avoid complete inventories, capability snapshots, Route/candidate count/order assertions, intermediate DTO snapshots, and repeated per-model matrices. Prefer removing duplicate coverage.
- Use canonical fixtures across layers and independent expected results. Keep Rust runtime contracts and Python corpus/tooling responsibilities distinct; do not grow the testkit into a speculative framework.
- Run focused checks first, then the proportionate baseline in [development.md](docs/development.md). Exercise changed behavior: syntax, compilation, and test existence are not execution evidence.
- Parallelize only independent scenarios with isolated paths, ports, and outputs. Keep ordered retry/fallback/cancellation scenarios serial; use readiness/events and bounded timeouts, not sleeps to hide races.
- Deterministic tests do not prove live Provider, SDK/Agent, TLS/network, load, long-run, or production compatibility. State each unexecuted layer explicitly.

## Documentation and Completion

- Follow [docs/README.md](docs/README.md) for fact ownership. Keep current architecture and necessary reasons, not speculative roadmaps or decision-history documents. Local implementation details belong in module/API docs and tests.
- Do not duplicate full model metadata or dynamic Provider directories. Keep official source identity, URL, snapshot/recheck boundaries; registration relationships may remain in implementation status.
- Preserve independently valuable external acceptance and observed discrepancies with dated evidence boundaries. Only executed contradictions support discrepancy claims; source-directory disagreements and untested inferences do not. Routine local test runs do not require historical documents.
- Update affected current facts and evidence pointers, not completion diaries. Restore the current focus to empty after completing the approved behavior slice.
- Inspect the final diff and verify relative links/anchors and `git diff --check` for documentation changes. Runtime assets such as `docs/openapi.yaml` and `docs/swagger-ui.html` are not ordinary movable documentation.
- Report what changed, the files involved, exact checks and outcomes, skipped external layers, and remaining acceptance gaps. Do not claim runtime success from documentation edits or improved agent behavior from static instruction review alone.
