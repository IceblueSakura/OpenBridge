# Generation Migration and Validation

## Current state

The workspace contains only the v2 semantic core, protocol codecs, lowering, pure SSE framing and their independent tests. The predecessor runtime is retired at the explicit user-approved breaking boundary documented in [archive.md](../archive.md). Runtime availability and source compatibility are not acceptance criteria for this phase.

**Retirement is not migration completion.** There is no current service, Provider execution, topology/registry, credential loader, MCP or telemetry implementation. Synthetic test listeners are not a production Router.

## Semantic migration rule

```text
archived behavior / independent protocol evidence
 -> identify an observable invariant
 -> assign one v2 semantic/protocol/lowering owner
 -> implement within the admitted scope
 -> prove decode, encode, transformation and refusal independently
```

Do not restore predecessor modules, Native/Bridge dual authority or speculative gateway tool executors merely to obtain source compatibility. The fixed archive provides old source/tests without a second workspace implementation.

## Phase gates

| Gate | Required result | Remaining boundary |
|---|---|---|
| M1 — Generation IR | Supported meaning, identity and presence are typed; requirements derive from final validated IR | Media and broader task semantics are not completed by the current text slice |
| M2 — Chat / Responses codecs | Independent decoding and encoding, same-protocol authority and explicit unrepresentable projections | Responses pure-text acceptance is active; rich Responses controls/metadata are not all representable in Chat |
| M3 — Conformance | Convergence, mutation/deletion authority, fidelity isolation, terminal/resource failures and independent expected fixtures | Full field/event admission and negative-case coverage still require review |
| M4 — Archived responsibility assessment | Classify old responsibilities as adopted, intentionally absent, future execution concern or remaining semantic gap | Old code is already retired; this gate now assesses semantic coverage, not permission to delete it |

Generation remains the active task. Do not begin Embedding, Image Generation or dedicated Speech migration merely because the old runtime has been removed. Approved short-cycle scope is recorded in [current-focus](../implementation-plans/current-focus.md).

## Current owners

| Layer | Owner and admitted behavior |
|---|---|
| Task | `src/semantic/task/generation/`: instructions, ordered messages/calls/results/reasoning, text/refusal, output/generation controls, typed metadata, Static/Event and pure requirements |
| Protocol | `src/protocol/openai/`: Chat/Responses task payload codecs; complete Responses envelope separates model/delivery/execution hints from task semantics |
| Lowering | `src/lowering/`: immutable final IR representability; unsupported Chat grouping/controls/replay/metadata fail instead of silently dropping values |
| Fidelity | `src/protocol/fidelity.rs`: bounded wire identity and origin/owner-bound replay; source records cannot restore deleted values |
| SSE | `src/protocol/openai/sse.rs`, `src/transport/sse.rs`: one bounded frame at a time, HTTP media-type checks and explicit EOF/terminal failures |
| Tests | `tests/semantic_v2_*`, `tests/sse_contract.rs`: independent semantic fixtures, transformation/failure tests, test-only body lifecycle and explicitly pinned SDK loopback |

Precise text admission belongs to [responses-text-profile.md](responses-text-profile.md), not a second schema here. Function arguments remain untrusted exact strings; no tool is executed. A checked SDK `parsed_arguments` convenience value cannot become a second authority. Reported usage, including Responses cache-write counters, is never estimated; unrepresentable details fail lowering.

Reasoning request controls, readable reasoning and opaque replay have separate owners as defined in [ADR-v2-0006](decisions/0006-reasoning-ownership.md). Stable identities, owner-dependent metadata and Static/Event coherence remain mandatory even when a fixture passes the SDK.

## Archived responsibilities

- Text/function/reasoning semantics have v2 owners, but broader old behaviors need independent admission rather than mechanical porting.
- Media semantics, wider profiles and missing field/negative-case coverage remain semantic gaps.
- Model catalog, Provider contracts, topology, credentials, retry/fallback, cancellation/commit coordination, observability and ingress are future runtime work, absent from the current library.
- Gateway web-search loops and ToolPlan injection/stripping prototypes are intentionally absent. They are not prerequisites for completing pure codecs.
- Old corpus, SDK production-Router tests and runtime safety tests remain available in the archive as evidence; they are not part of the current test baseline.

## Exit condition

For the admitted Generation surface, without a Provider account:

```text
wire -> decode -> IR -> trusted transform -> requirements
     -> representability -> encode -> wire
```

Independent conformance must prove final IR authority and explicit failure boundaries. Only after Generation gates pass should a separately authorized topology/execution phase construct production wiring. No live Provider probe, paid call or deployment is implied.
