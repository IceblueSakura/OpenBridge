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
