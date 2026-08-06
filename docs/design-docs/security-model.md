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
typed request. An external host receives the same request through a strict
versioned protocol. It is outside ASTER's trust boundary and should possess
only the operating-system authority needed by its declared adapters.

## Threats and enforced mitigations

- Model prompt injection cannot mint authority: model output is `Candidate` and
  prompt instructions are static syntax.
- Tool confusion is prevented by read/write invocation typing and exact action
  identities.
- Permit substitution, deserialized forgery, and double use are prevented by
  canonical proposal hashes, self-validating permit identities, an issuance
  ledger, affine static checks, a runtime consumption ledger, and proposal-seal
  revalidation at permit issuance and immediately before permit consumption.
- Excessive effects are rejected by capability and pre-driver budget checks.
- Replay substitution is rejected by hash-chain, fingerprint, request-order,
  and semantic recomputation checks.
- Partial state publication is prevented by pending transactional updates.
- Unverified write success cannot be discarded: committed receipt hashes remain
  in the snapshot until successful reconciliation, and block normal completion.
- Secret exfiltration is prevented by opaque representation and rejection at
  every prompt, render, serialization, state, trace, and snapshot boundary.
- Ordinary JSON is restricted to boundary-specific data shapes: untrusted
  wrappers only at external input, validated wrappers only at model/tool input,
  and no candidate or authority wrapper at a persistent or external boundary.
- Nondeterministic audit output is prevented by canonical serialization,
  ordered collections, recorded effect results, and no ambient clock/randomness.
- Early host execution is excluded by the protocol contract: a preview is not
  authority, and the execution grant is emitted only after maximum usage and a
  sealed continuation are durable. Request and grant hashes prevent ASTER from
  accepting a substituted resolution.
- Host framing failures are bounded, strictly decoded, redacted, and recorded
  with stable `ASTER-HOST-*` classifications. Standard output remains
  protocol-only and secrets are forbidden at the boundary.

## Residual risks

ASTER 0.1 does not solve production key storage, trace encryption, distributed
consensus, or live-provider authenticity. The 0.2 host protocol cannot stop a
malicious host from using its own ambient authority before a grant, falsifying
a provider result, or under-reporting usage. Isolation, credentials, provider
attestation, and least privilege remain deployment responsibilities. Traces
and snapshots may contain private non-secret data and must be protected as
sensitive files.
