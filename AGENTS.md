# OpenBridge Agent Instructions

These instructions apply throughout the repository. More specific guidance may narrow, not weaken, authorization, security or change-control boundaries.

## Scope and Authorization

- Develop against current product goals, contracts and source. Predecessor feature parity is not an objective; [archived code](docs/archive.md) is not a current dependency or acceptance requirement. Do not claim production readiness from the mainline designation.
- Before editing, inspect the branch, `git status` and target diff. Preserve unrelated work; stop on overlapping external edits. Do not discard, stage, commit or push without scoped authorization.
- Review, diagnosis and planning are read-only. An implementation request authorizes scoped edits and disposable checks; a design step is not another approval gate. Stop for unresolved material choices, not arbitrary phases.
- Commits, pushes, publication, service/production changes, credential operations and live/paid requests require explicit authorization for the target and effects. A commit request does not authorize a push.
- ADRs record decisions, [next-goal](docs/implementation-plans/next-goal.md) records direction, and [current-focus](docs/implementation-plans/current-focus.md) records the approved behavior slice. None grants authority or proves implementation.

## Read the Relevant Context

Read [README](README.md) and the [documentation index](docs/README.md) before non-trivial work, then follow only the relevant sources:

| Task | Starting points |
|---|---|
| Product behavior | [Generation gaps](docs/implementation-status/generation.md), affected codecs, callers and independent tests |
| Ownership / data flow | [Architecture](docs/architecture.md), [design and ADRs](docs/architecture-v2/README.md), affected module docs |
| IR / codec / media | [Semantic IR](docs/architecture-v2/semantic-ir.md), [protocol/lowering](docs/architecture-v2/protocol-and-lowering.md), [fixed sources](docs/references/upstream-sync.md), affected types and profile contracts |
| Implementation / tests / dependencies | [Development guide](docs/development.md), affected tests and manifests |
| Provider / compatibility | The lookup below, owning adapter rules and current official sources as needed |
| Documentation | Canonical owners, incoming links and affected instructions; do not refresh external verification dates during local editing |

Contracts describe intended behavior; source and tests describe implementation; executed checks describe observations. Investigate disagreements instead of silently changing the contract to match code.

## Current Provider, Model and Compatibility Information

Query current information; do not maintain Provider/model inventories, support matrices, account status or test results in Markdown, comments or memory.

1. **Registration:** [provider catalog](src/provider/catalog.rs) and [topology catalog](src/topology/catalog.rs), following Public Model → Route → Endpoint, upstream IDs, representation and execution contracts. A Provider or `Dialect` declaration alone is not a usable binding.
2. **Activation:** [bootstrap](src/gateway/bootstrap.rs) and [configuration](src/gateway/config.rs). Check explicit entries, protocol family and credential activation against compiled topology; Chat admission does not imply Responses admission.
3. **Wire behavior:** [adapters](src/adapter/mod.rs), [named rules](src/protocol/adaptation.rs), owning request/response/event codecs, lowering and independent tests. Test names and previous pass counts are not capability evidence.
4. **Probes:** [probe catalog](examples/probe_support/catalog.py), selected entry points and [operations](docs/probes.md). Probe choices/defaults are not the product catalog.
5. **Running instance:** verify binary revision and operator-approved non-secret activation metadata; inspect [router source](src/gateway/http.rs) before assuming discovery endpoints. If metadata is unavailable, report activation as unknown. Do not inspect private files or dump environment values. [Official sources](docs/references/providers/README.md) describe upstream offerings, not OpenBridge admission or successful inference.

Report revision, scope and unverified layers in the current conversation. Credential-bearing discovery and live requests still require authorization and bounded execution.

## Implementation Discipline

- Keep one Agent-first, protocol-neutral semantic authority, with typed capability domains and scoped extensions rather than a protocol/common subset, mechanical field union, raw SDK DTOs or a second Provider IR. OpenAI, Google and Anthropic are design references, not expressiveness ceilings. Shared concepts retain one owner even when codecs are incomplete; existing wire profiles remain binding until explicitly changed.
- Treat the project as pre-release design exploration: stabilize concepts and invariants before Rust shapes. Keep response closure, artifact completeness, logical-turn progress and continuation requirements distinct. Owner/group/prefix dependencies govern transforms and replay; representing control transfer does not authorize orchestration.
- Keep task, wire and modality separate. Pure codecs/lowering cannot access registry, credentials or network. Runtime target/retry/commit state is not semantic data. Check the actual call path and affected ADR before changing ownership.
- Final typed semantics govern encoding. Fidelity cannot restore deleted values or attach to another owner. Preserve field-specific absent/null/empty/default distinctions, numeric precision, resource meaning and source-bound replay.
- Validate request/response closure and Static/Event consistency. Derive requirements after transforms; project each fixed candidate independently from immutable input, without route reordering or capability unions.
- Keep changes focused. Split by responsibility, not line count; preserve intended public paths through explicit re-exports. Keep startup registration, request planning, pure policy, body I/O and observation separate. Do not add speculative frameworks or unrelated tasks.
- Before a behavior change, record observable result, requirement, failing case, non-goals and validation boundary in current-focus, then use TDD. Documentation/comment-only work does not need a behavior slice. Clear focus when the approved slice is complete; do not erase unfinished direction in next-goal.
- Breaking changes update implementation, serialization, OpenAPI, examples, fixtures and affected contracts together. Unpublished APIs may be replaced within scope without legacy aliases or meaningless schema bumps. Dependency changes update both manifests and locks, followed by locked validation.

## Security and Resources

- Never expose real credentials, private configuration, sensitive bodies or auth caches in files, tools, logs, fixtures or output. `.env`, private `config/` files and OAuth files are not investigation or test inputs. Do not discover third-party auth caches; use authorized synthetic examples.
- Preserve loopback ingress, trusted egress and fail-closed authentication. Business data cannot choose upstream URLs, credentials, auth/proxy headers or scripts. Do not expose credential locators or trusted origins downstream.
- Preserve bounded allocation/capture, protocol terminals, cancellation, cleanup and retry/fallback/commit boundaries. No unbounded buffering, post-commit replay or fabricated successful terminal.
- Any content logging starts after authentication, observes the final downstream boundary and redacts sensitive headers. Bounded snapshots belong only in a dedicated local JSONL sink, not stdout or reviewed OTLP traces; sink failure cannot change responses. Development logging profiles do not authorize sensitive traffic capture.
- Test tooling must not load real credentials, call Providers, implicitly start services or add automatic retry/fallback. Explicit bootstrap tests may own an isolated disposable loopback binary with synthetic keys and rejecting loopback egress. Live, ignored network and external-dependency gates require approval; paid probes also need exact targets, request matrix, budgets and sanitized output limits.
- Do not inspect or manually edit build/virtualenv/cache output (`target/`, `tools/corpus/.venv/`, `.pytest_cache/`, `__pycache__/`) unless targeted. Normal tools may populate their caches. Do not commit derived `testdata/{generated,reports,dist,runtime}/`; canonical fixtures are contracts, not cleanup targets.

## Verification

- Add tests for distinct semantic, wire or security/resource boundaries, not each catalog value. Test at the lowest owning layer; add at most one production-Router smoke when it adds independent value. Avoid inventory and incidental DTO snapshots.
- Use independent wire→IR and IR→wire expectations, then insertion/replacement/deletion and applicable stream/failure cases. Round trips alone can hide symmetric loss. Reuse small synthetic fixtures; review version, license, sensitivity and oracle independence before importing assets.
- Run focused checks, then the [development baseline](docs/development.md). Rust changes require `cargo fmt -- --check`, `cargo test --locked --offline` and `cargo clippy --locked --offline --all-targets -- -D warnings`. SDK loopback is a separate explicit gate. Prose-only changes require structure/link/anchor checks and `git diff --check`, not runtime tests.
- Parallelize only independent scenarios with isolated paths/ports/output. Use readiness/events and bounded timeouts; keep ordered retry/fallback/cancellation scenarios serial, without sleeps hiding races.
- Inspect the final diff. Report actual checks, failures and skipped layers in the task response. Compilation, test existence and synthetic execution do not prove live Provider, general SDK/Agent, TLS/network, load or production compatibility.

## Documentation and Comments

- Prefer executable types, validation and independent regressions. Put non-obvious protocol, compatibility, security, concurrency and cleanup rationale beside the owning code, with essential source URLs. Rust comments/docs and Python docstrings use concise English; comments explain why, not narrate a previous investigation.
- Keep Markdown for stable cross-module decisions, design gaps, source provenance and necessary operations. ADRs state the decision, rationale and consequences with owner links; do not duplicate field tables, defaults, file trees or test scenarios already owned by code/tests.
- Do not retain historical analysis, comparison surveys, audit reports, test results or completion diaries, including offline/SDK results. Do not move them into comments or memory. Outcomes belong in the current response and authorized ignored artifacts; Git history is sufficient for explicit historical questions.
- Preserve necessary attribution, fixed standard/SDK versions and licenses for retained material. References are source navigation, not another implementation schema or a changing Provider API snapshot. Do not add research pages when a nearby rationale comment or an existing ADR suffices.
- Update only affected owners and links. Do not weaken a design constraint merely because it is not implemented, or claim behavior improved from documentation review alone.
