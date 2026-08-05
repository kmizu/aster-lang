# Security Model

## Assets and trust boundaries

Protected assets are external write authority, capability grants, budget,
private fixture data, durable agent state, trace integrity, and opaque secret
handles. Source, event/state/capability JSON, fixture responses, trace files,
snapshots, and model/tool/human outputs are untrusted at their boundaries.

The compiler is trusted to enforce wrapper visibility, purity, effect bounds,
capability coverage, affine use, recursion rejection, and persistence rules.
The runtime is trusted to recheck concrete grants, budget, proposal/permit
binding, expiry, response schemas, state atomicity, and replay identity. The
effect driver has no access to compiler or policy internals and receives only a
typed request.

## Threats and enforced mitigations

- Model prompt injection cannot mint authority: model output is `Candidate` and
  prompt instructions are static syntax.
- Tool confusion is prevented by read/write invocation typing and exact action
  identities.
- Permit substitution and double use are prevented by canonical proposal hashes,
  affine static checks, and a runtime consumption ledger.
- Excessive effects are rejected by capability and pre-driver budget checks.
- Replay substitution is rejected by hash-chain, fingerprint, request-order,
  and semantic recomputation checks.
- Partial state publication is prevented by pending transactional updates.
- Secret exfiltration is prevented by opaque representation and rejection at
  every prompt, render, serialization, state, trace, and snapshot boundary.
- Nondeterministic audit output is prevented by canonical serialization,
  ordered collections, recorded effect results, and no ambient clock/randomness.

## Residual risks

ASTER 0.1 uses fixture-backed drivers and synthetic data only. It does not solve
production key storage, trace encryption, distributed consensus, malicious host
processes, or live-provider authenticity. Traces may contain private non-secret
data and must be protected as sensitive files.
