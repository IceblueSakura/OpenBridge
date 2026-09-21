# Architecture v2 Invariants

These invariants are design gates, not migration suggestions.

1. **Semantic authority** — once a supported field is decoded into Task IR, later wire output cannot recover an older semantic value from source JSON or SSE payload.
2. **Task before provider** — resolve the trusted Public Model task before semantic decode; provider selection happens only after final semantic requirements exist.
3. **One semantic owner** — protocol codec, policy, lowering and provider code cannot independently decide the same semantic value.
4. **Candidate isolation** — every route candidate lowers from the same immutable final Task IR. Candidate A cannot mutate input used by candidate B.
5. **No routing data in IR** — provider, endpoint, credential, route and retry state never enter Task IR.
6. **No semantic JSON surgery after encode** — provider execution may bind trusted URI/auth/headers and representation metadata, but must not rewrite modeled semantic fields in encoded JSON.
7. **Explicit loss** — unrepresentable semantics fail closed unless a named conversion policy explicitly authorizes an equivalent normalization or loss.
8. **Source metadata cannot resurrect semantics** — deletion of an IR item deletes its associated semantic output. Fidelity records may preserve only representation information still valid for surviving identities.
9. **Static/event coherence** — where both forms exist, materializing a valid Event IR must produce semantics compatible with the corresponding static response contract.
10. **Commit is irreversible** — fallback/retry is permitted only before downstream semantic output is committed. A second upstream response is never concatenated after commit.
11. **Bounded resources** — wire parsing, source records, event state, buffering, retries and preserved opaque data have explicit bounds.
12. **Trusted topology** — downstream input cannot provide provider identity, upstream URL, route, credential, authentication headers, or executable transformation rules.
13. **Capability dimensions stay distinct** — semantic support, wire representability, execution behavior and public promises are not collapsed into one capability flag.
14. **Evidence over compatibility** — legacy behavior is preserved only when supported by a product requirement, test, protocol contract, or provider evidence; source compatibility alone is not a requirement.
15. **Offline semantic tests are first-class** — codecs and lowering must be testable without provider accounts or network access using independent expected semantic fixtures.
