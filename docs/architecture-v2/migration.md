# Rewrite and Migration Strategy

## Current status

Architecture exploration is complete enough to begin controlled migration. The active phase is **Generation Semantic Migration & Validation**.

This remains a repository-preserving rewrite. The `main` branch and Git history retain the predecessor implementation while `semantic-v2` is free to make breaking internal changes.

## Migration rule

Do not port predecessor modules mechanically.

For each behavior:

```text
legacy implementation / evidence
 -> extract observable semantic invariant
 -> assign one v2 owner
 -> implement in semantic / protocol / lowering
 -> add independent conformance test
 -> only then mark predecessor responsibility replaceable
```

Source compatibility is not an acceptance criterion.

## Active scope: Generation only

Migrate and validate:

- instructions and messages;
- text and stable item/part identity;
- image, audio and file resources;
- function tools, tool choice, tool calls and tool results;
- structured output and JSON Schema;
- reasoning controls and replay semantics where supported;
- generation controls;
- semantic presence/default distinctions;
- bounded fidelity/source metadata.

Do not start another Task family until Generation passes the phase gates.

## Milestones

### M1 — Generation IR

Complete a protocol-neutral Generation request/response/event model for the supported product surface.

Acceptance:
- no modeled semantic value requires original JSON to remain meaningful;
- identity and presence rules are explicit;
- requirements are pure projections from validated final IR.

### M2 — Chat / Responses codecs

Migrate proven protocol behavior from predecessor codecs and evidence.

Acceptance:
- supported Chat request/response semantics decode and encode through IR;
- supported Responses request/response semantics decode and encode through IR;
- same-protocol and cross-protocol paths use the same semantic authority;
- unsupported mappings fail explicitly.

### M3 — Semantic conformance suite

Prefer semantic properties over round-trip self-validation.

Required test classes:

1. **Convergence:** equivalent Chat and Responses wires decode to equivalent semantics.
2. **Authority:** semantic mutation/deletion changes all applicable encoded outputs.
3. **Fidelity isolation:** source metadata cannot restore deleted or replaced semantics.
4. **Representability:** unsupported endpoint semantics fail deterministically before encoding.
5. **Independent expected fixtures:** decoder and encoder correctness are checked against independently authored expectations.

### M4 — Legacy replacement assessment

Audit:

- `src/ir/generation/`;
- `src/bridge/static_codec/`;
- `src/bridge/event_codec/`;
- Generation parts of `src/pipeline/`;
- relevant `testdata/` and `docs/implementation-status/evidence/`.

Classify each responsibility as:

- migrated to v2;
- retained outside semantic core;
- intentionally unsupported;
- still blocking replacement.

M4 does not require deleting the legacy path. It establishes whether deletion is safe.

## Function-tool slice: implementation boundary

The offline v2 slice owns ordinary function definitions, optional description/schema, explicit strictness and source-default strictness, optional tool choice and parallel controls, bounded argument/result strings, call/result association, and Chat assistant-message ownership. `GenerationRequest::with_items` revalidates identity and association after insertion, replacement, deletion or reordering.

| Layer | Current owner and behavior | Remaining boundary |
|---|---|---|
| Semantic request | `src/semantic/task/generation/{request,tool,validate,requirements}.rs`; final IR owns function policy and history | Media, structured output, reasoning replay and broader presence coverage are not migrated codec features |
| Protocol request | `src/protocol/openai/{chat,responses,function_tools}.rs`; both protocols decode and encode function history | Inputs are task payloads; model binding, delivery/stream controls, top-level Responses instructions and other unmodeled fields are rejected by this slice |
| Lowering | `src/lowering/generation.rs`; validated immutable representations are the only public encoder inputs | Omitted strict cannot cross profiles with different defaults; unimplemented domains fail even when the endpoint advertises support |
| Static response | `src/semantic/task/generation/response.rs` and `src/protocol/openai/static_response.rs`; completed single-candidate text/function output and replay into request history | Partial/failed responses, refusal, annotations, multiple candidates and Event/SSE are explicit unsupported cases |
| Fidelity | `src/protocol/fidelity.rs`; bounded Responses item IDs keyed by surviving semantic identity | No raw semantic envelope or generic unknown-field passthrough; source records cannot create deleted output |
| Conformance | `tests/semantic_v2_tools.rs` and `tests/semantic_v2_generation.rs`; independent wire/IR expectations and mutation/failure cases | No production routing, provider, SDK or live-network acceptance |

Arguments remain exact untrusted strings, including incomplete or invalid JSON; the gateway neither repairs nor executes them. Tool results are text in this slice. Missing/null Chat content on a tool-only assistant message normalizes to null; empty text remains an explicit text part. Omitted function strictness retains its default meaning (`NonStrict` or `NormalizeSchema`), while explicit false/true remains explicit. Null strict and unknown tool/control fields are rejected.

Lowering explicitly normalizes a contiguous run of independent function calls into one Chat assistant call message. Existing Chat text/call groups retain their stable owner; independent Responses text messages are never merged into a neighboring call run. Multiple independent response messages cannot fit the supported single Chat candidate and are rejected. Chat multi-part text lowering also rejects instead of joining part boundaries.

Static response usage is bounded protocol-specific envelope data in this slice. Same-profile encoding preserves it; cross-profile encoding with usage fails until a separate, evidence-backed usage projection is implemented. Completed response envelope IDs, model labels and timestamps stay outside task semantics. Wire item IDs are distinct from call IDs and cannot override them.

The fixed offline admission limits are 4 MiB serialized task payload, 1 MiB per text/argument/result or schema value, 1,024 semantic items and 128 function definitions. Fidelity IDs are bounded to 256 bytes each and 1,024 records. Runtime-configurable budgets and source-delivery facts remain later integration work.

### Legacy replacement assessment for this slice

| Predecessor responsibility | Classification |
|---|---|
| Ordinary function values and request mappings in `src/ir/generation/tool.rs` and `src/bridge/static_codec/request.rs` | Migrated subset has independent v2 owners; broader legacy semantics remain |
| Static function response mapping | Completed single-candidate subset migrated; partial results, usage projection and other output domains still block replacement |
| Tool argument events, reducers and materialization in `src/bridge/event_codec/` and `src/ir/generation/` | Still blocking replacement; v2 Event/SSE has no implementation |
| Model binding, provider/route selection, credential and attempt lifecycle | Retained outside semantic core; not modified by this slice |
| Production Generation pipeline | Still uses predecessor path; offline v2 conformance does not authorize deletion |

M1-M4 are therefore not complete. Next, migrate the function-call Event IR, argument deltas, identity validation, terminal failures and materialization against this static contract before expanding another semantic domain. Do not delete the predecessor tool path or begin topology/execution migration yet.

## Deferred work

Until M1-M4 pass, defer:

- Embedding, Image Generation and Speech IR;
- provider/route redesign implementation;
- credential migration;
- retry/fallback and execution rewrite;
- MCP and observability redesign;
- general hooks/plugins;
- broad architecture-document expansion.

Existing security, credential, transport, cancellation, commit and observability evidence remains valid input for later phases.

## Phase exit gate

Generation migration is complete only when its supported semantics can, without provider accounts or network I/O:

```text
Chat / Responses wire
 -> decode
 -> Generation IR
 -> semantic transform
 -> requirements
 -> representability check
 -> encode
 -> Chat / Responses wire
```

and the semantic conformance suite proves IR authority.

Only then begin Endpoint/Topology/Execution migration.
