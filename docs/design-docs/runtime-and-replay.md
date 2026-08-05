# Runtime and Replay

The ASTER runtime steps a typed instruction machine. Pure stepping returns
`Continue`, `Completed`, or a controlled diagnostic. External instructions
return `Yield(EffectRequest)` and perform no I/O themselves.

Record mode checks exact capabilities and reserves budget before calling the
fixture driver. It snapshots the continuation, records the canonical request,
validates a typed resolution, settles usage, and resumes. Budget evidence
contains before/reserved/actual/released/after ledgers and is independently
recomputed during replay. State updates remain pending until successful handler
completion.

Each JSON Lines trace entry hashes its schema, run, sequence, kind, payload, and
previous hash using recursively sorted canonical JSON and SHA-256. The run
header binds normalized program, input, initial state, and capability grants.

Replay first verifies this chain and all fingerprints. It creates a fresh
machine, steps to each suspension, compares the complete canonical request with
the next recorded request, and injects only the matching resolution. It
independently repeats budget, policy, proposal, permit, reconciliation, and
final-state calculations. Its API has no driver parameter, preventing accidental
external calls by construction.

A versioned snapshot contains only serializable domain values: program identity,
agent/handler/event identity, instruction state, frames, locals, transactional
state, budget, capability fingerprint, affine ledger, trace position, and the
pending request. Provenance-bearing wrappers retain a stable request or boundary
reference without exposing their hidden payload. The affine state also retains
outstanding write receipts, so normal completion is impossible until each
receipt is reconciled. Snapshot decoding rejects unknown nested fields. Resume
accepts exactly one matching typed resolution and verifies that the snapshot's
trace position and chain head identify the supplied trace prefix.
