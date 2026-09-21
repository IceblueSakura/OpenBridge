# Rewrite and Migration Strategy

## Strategy

This is a repository-preserving rewrite, not an incremental compatibility migration.

The existing implementation remains available through Git history and the `main` branch while `semantic-v2` develops the replacement architecture. New v2 code may coexist temporarily with legacy modules, but v2 must not add compatibility layers whose only purpose is preserving internal Rust APIs.

## Asset classification

### Preserve directly

- provider probe evidence and integration observations;
- protocol fixtures and semantic test data;
- security boundaries around trusted targets and credentials;
- bounded body/SSE/transport behavior;
- retry-before-commit and cancellation requirements;
- observability privacy requirements;
- validated OAuth and credential lifecycle behavior;
- public product requirements that remain intentional.

### Preserve as knowledge, redesign implementation

- `src/ir/generation`;
- `src/bridge`;
- `src/pipeline`;
- registry/public-model compilation;
- provider adapters and capability structures.

For these areas, extract invariants and examples first. Do not mechanically rename old structs into v2 types.

### Delete when superseded

- transitional Native-vs-Bridge ownership paths;
- JSON normalization or post-codec surgery that owns modeled semantics;
- duplicate request-fact structures once final IR-derived requirements replace them;
- compatibility facades that exist only for the pre-v2 internal module layout.

## Implementation order

1. Freeze the v2 ontology and invariants.
2. Define shared semantic value primitives and the Task union.
3. Implement Generation request/response/event IR as the first vertical slice.
4. Define protocol codec traits around explicit Task + ProtocolProfile inputs.
5. Decode downstream request before requirements extraction.
6. Derive requirements exclusively from final IR.
7. Introduce Endpoint contracts and candidate lowering.
8. Encode without semantic post-processing.
9. Reconnect execution, credential, transport and commit lifecycle.
10. Port Embedding, ImageGeneration and Speech task families.
11. Remove superseded bridge/pipeline/IR modules.
12. Rewrite top-level architecture documentation and make v2 the default branch only after semantic acceptance gates pass.

## Acceptance gates for a migrated task

A task is v2-complete only when:

- meaningful supported input distinctions have typed semantic representation;
- request and response mappings are independently tested;
- streaming event semantics are covered when the task streams;
- requirements are derived from final IR;
- each candidate starts from the same immutable IR;
- mutations to IR are observable in encoded output;
- deleted semantics cannot reappear from source records;
- unsupported target representation fails deterministically;
- no provider/network dependency is required for codec conformance tests.

## First coding milestone

Do not begin by porting the current bridge.

Build a minimal vertical semantic path:

```text
Chat request fixture
 -> Chat codec
 -> GenerationRequest
 -> requirements
 -> Responses lowering
 -> Responses codec
 -> expected wire fixture
```

Then run the same IR through a Chat target codec. This proves that Native and cross-protocol paths share one semantic authority before reconnecting production routing.
