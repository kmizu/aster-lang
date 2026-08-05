# ASTER Architecture

ASTER separates source representation, authority analysis, executable control
flow, and external effects so that no convenient shortcut can bypass policy.

## Crate map and dependencies

- `aster-diagnostics`: stable codes, spans, safe rendering, registry.
- `aster-syntax`: UTF-8 source, lexer, parser, syntax tree, formatter.
- `aster-semantics`: names, types, effects, capabilities, purity, taint,
  affine-use and termination checks.
- `aster-ir`: typed serializable instructions and explicit suspension points.
- `aster-runtime`: VM, grants, budgets, proposals, permits, drivers, state,
  traces, snapshots, replay, and resume.
- `aster-cli`: file-boundary validation and orchestration only.

Dependencies point left in the chain shown in [README.md](README.md).
[`scripts/check-architecture.sh`](scripts/check-architecture.sh) enforces this
from Cargo metadata.

## Compilation pipeline

```text
UTF-8 source
  -> lex and parse with byte spans
  -> collect declarations
  -> resolve names
  -> type and wrapper visibility checks
  -> purity/effect and capability checks
  -> affine-use, recursion, and persistence checks
  -> typed serializable IR
```

The CLI never duplicates these checks. Stable diagnostics originate at the
layer that owns the violated invariant.

## Typed governance boundary

`Candidate<T>` hides model output until a validator produces `Checked<T>`.
`Proposal<A>` immutably binds a write action, arguments, intent, risk,
sensitivity, capability request, idempotency key, program hash, and schema.
`Permit<A>` records issue/expiry times and decision evidence and is an expiring
single-use runtime value bound to exactly one proposal hash. `Receipt` must be
reconciled against a later observation before normal completion.

Semantic enforcement locations are catalogued in the
[security model](docs/design-docs/security-model.md) and diagnostics are listed
in [the registry](docs/design-docs/diagnostics.md).

## Explicit machine and effect boundary

Checked source lowers to instructions with stable identities, explicit locals,
branches, pending state updates, and effect requests. Pure VM stepping performs
no I/O. The runtime's `EffectDriver` interface is the only external effect
boundary; AST, checker, policy evaluation, lowering, and VM core cannot call it.
Lowering gives each lexical `let` and pattern payload binding a unique runtime
local identity, preserving source shadowing across nested blocks and branches.
The versioned program uses deterministically keyed catalogs and a SHA-256
content hash. Its pure-expression representation cannot encode model, tool,
authorization, commit, or reconciliation effects; those remain distinct
instructions even when nested in a larger source expression.

Before yielding to a driver, the runtime resolves and verifies an exact grant,
reserves the declared budget, and writes a snapshot. It then validates and
settles the resolution. At permit issuance and immediately before a write it
revalidates the proposal seal, then consumes the matching issued permit. State
mutations accumulate in a transaction and publish atomically only after
successful handler completion.

## Recording, replay, and resume

Record mode appends canonical JSON Lines entries to a SHA-256 hash chain and
writes versioned snapshots at external-effect boundaries. Replay verifies the
chain and run fingerprints, re-steps the machine, compares each complete
request identity, and injects the recorded resolution without constructing a
driver. Resume restores the serializable instruction pointer, frames, locals,
state transaction, budget, capability fingerprint, affine ledger, and trace
position before accepting one matching resolution.

The full rationale is in
[runtime-and-replay.md](docs/design-docs/runtime-and-replay.md). The
implementation sequence and validation evidence are recorded in the
[completed execution plan](docs/exec-plans/completed/bootstrap-aster-0.1.md).
