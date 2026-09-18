# OpenBridge Agent Instructions

These instructions apply to the repository and all subdirectories. More specific guidance may narrow them, but must not weaken authorization, security, evidence, or change-control boundaries.

## Scope and Authorization

- OpenBridge is an experimental Rust/Axum, headless, OpenAI-compatible multi-provider gateway. Ground work in the live checkout, applicable contracts, source, and tests; history and summaries are navigation aids, not proof.
- Inspect the branch, `git status`, and the target-file diff before editing. Preserve unrelated work; do not overwrite, revert, stage, or commit it. Stop on overlapping external edits.
- Reviews, diagnosis, status, and planning are read-only. Implement explicitly requested changes within scope; a design-first step is not an extra approval gate when implementation is already authorized. Stop for unresolved material design choices, not an arbitrary phase boundary.
- Commit, push, external publication, service/production changes, and paid Provider requests require explicit authorization for the relevant action and target. A commit request does not authorize a push.
- ADRs record accepted decisions; [next-goal](docs/implementation-plans/next-goal.md) records direction and [current-focus](docs/implementation-plans/current-focus.md) records the approved implementation slice. None grants authority, proves implementation, or turns a design gap into a completed feature.

## Read the Relevant Context

Before non-trivial work, read the root [README](README.md) and [documentation index](docs/README.md), then follow the task-specific route:

| Task | Required context |
|---|---|
| Product behavior or compatibility | Relevant `docs/functional-requirements/` leaves, affected source/tests, and current status boundaries |
| Cross-module ownership or data flow | [Current architecture](docs/architecture.md), relevant [ADRs](docs/decisions/README.md), affected module docs and callers |
| Task IR, media semantics, or encode/decode | [IR authority](docs/decisions/0001-generation-ir-authority.md), [task IR design](docs/decisions/0002-task-ir-and-semantic-ownership.md), relevant task contracts, and actual `src/ir/`, `src/bridge/`, `src/pipeline/` types/callers |
| Implementation, dependency, or test changes | [Development guide](docs/development.md), affected tests and manifests |
| Provider onboarding or protocol changes | Relevant `docs/references/` source snapshots, registration code, Provider status and evidence |
| Corpus or semantic testing | [Corpus contracts](testdata/README.md), [offline codec acceptance](testdata/semantic-testing.md#9-provider-无关的任务-ircodec-验收), and the affected Rust contracts or `tools/corpus/` source/tests |
| Documentation or instruction maintenance | Documentation responsibilities, canonical sources, incoming links, and affected guidance |

Read only relevant leaves, not every document. Product contracts state intended behavior, source/tests describe implementation, and executed evidence describes observations. Investigate conflicts rather than silently changing a contract to match code.

## IR and Codec Work

- Check the affected task's supported semantics, IR ownership, request/response codecs, and failure boundaries before choosing a field-by-field migration. Distinguish type expressiveness, codec mapping, and production wiring; a type or passing test alone proves none of the other layers. Use ADR-0002's design admission before formal corpus expansion.
- Keep task, wire protocol, and modality separate. Follow the task-family design rather than forcing Embedding, Images, or dedicated Speech semantics into GenerationRequest. Shared resource values must retain task-specific meaning; Chat wire does not imply conversation history.
- Follow the accepted target ordering: resolve the fixed Public Model task contract before semantic decode, process task IR before Provider selection, and keep registry/credentials/network outside pure codecs. Verify the current call path before changing it; the ADR is not a description of already completed wiring.
- Treat typed semantics as authoritative. Source metadata may preserve equivalent representation, not restore deleted values or reattach metadata to the wrong item. Check omission/null/empty/default distinctions, media metadata, numeric precision, and source-bound extensions against the affected contract. Do not replace a rich Native path with the narrower Bridge encoder.
- Validate request/response closure and Static/Event consistency, not only request encoding. Derive requirements after semantic changes; each candidate must project independently from immutable input without changing fixed Route order or capability intersections.
- Do not introduce speculative hooks, a generic plugin framework, new task support, or arbitrary cross-Provider conversion while closing IR ownership gaps. Detailed task semantics and design choices belong in the ADRs, task contracts, and source docs, not a second schema in this file.

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
- Do not inspect or manually edit `target/`, `tools/corpus/.venv/`, `tools/corpus/.pytest_cache/`, or Python `__pycache__/` unless explicitly targeted. Normal build/test tools may populate their own caches; do not treat generated output as source.
- Do not commit derived output under `testdata/generated/`, `testdata/reports/`, `testdata/dist/`, or `testdata/runtime/`. Canonical `testdata/` files are contracts; change wire data only for requested behavior and do not add comments to formats that forbid them.

## Tests and Verification

- A new test must protect a distinct client-visible semantic result, wire behavior, or security/resource failure boundary. A new Model, Route, Provider instance, or catalog value alone does not justify a test.
- Test each mechanism at its lowest owning layer; add at most one production-Router smoke test when it adds independent value. Avoid complete inventories, capability snapshots, incidental DTO snapshots, Route/candidate inventory assertions, and repeated per-model matrices. Assertions on intentional IR semantics and routing mechanisms are valid; do not confuse them with implementation snapshots.
- For codecs, verify wire-to-IR and IR-to-wire against independent expected results, then test insertion/replacement/deletion and applicable stream/terminal failures. Round trips alone can hide symmetric loss. Provider-independent means offline and account-independent, not protocol/profile-independent.
- Reuse canonical fixtures and admit external samples only after version, license, sensitivity, and independent-oracle review. Prefer small deterministic media and vectors over model-quality datasets. Keep Rust runtime contracts distinct from Python corpus/tooling; do not force all tasks into the current semantic-case schema or build a speculative test framework.
- Run focused checks first, then the proportionate baseline in [development.md](docs/development.md). Rust changes use `cargo fmt -- --check`, `cargo test --locked`, and `cargo clippy --locked -- -D warnings`; use the guide's separate corpus gates when those contracts change. Prose-only edits need structural/link checks and `git diff --check`, not a manufactured runtime run. Compilation and test existence do not prove changed behavior.
- Parallelize only independent scenarios with isolated paths, ports, and outputs. Keep ordered retry/fallback/cancellation scenarios serial; use readiness/events and bounded timeouts, not sleeps to hide races.
- Deterministic tests do not prove live Provider, SDK/Agent, TLS/network, load, long-run, or production compatibility. State each unexecuted layer explicitly.

## Documentation and Completion

- Follow [docs/README.md](docs/README.md) for fact ownership: current structure in architecture, decisions in ADRs, concrete gaps in implementation status, and direction in next-goal. Keep local details in source docs/tests and this file focused on agent workflow, not a duplicate architecture or schema.
- Keep essential attribution in references and independently valuable external acceptance in evidence. Do not repeat model metadata, Provider inventories, validation disclaimers, or completion diaries. Only executed contradictions support observed-discrepancy claims; routine local tests do not require historical reports.
- Update affected architecture, decisions, goals and concrete limitations, not completion diaries. Restore the current focus to empty after completing the approved behavior slice; update the next goal separately rather than erasing an unfinished direction.
- Inspect the final diff and verify relative links/anchors and `git diff --check` for documentation changes. Runtime assets such as `docs/openapi.yaml` and `docs/swagger-ui.html` are not ordinary movable documentation.
- Report what changed, the files involved, exact checks and outcomes, skipped external layers, and remaining acceptance gaps. Do not claim runtime success from documentation edits or improved agent behavior from static instruction review alone.
