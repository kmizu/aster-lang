# Runtime and Replay

The ASTER runtime steps a typed instruction machine. Pure stepping returns
`Continue`, `Completed`, or a controlled diagnostic. External instructions
return `Yield(EffectRequest)` and perform no I/O themselves.

`RecordSession` is the pure record-mode state machine shared by fixture and
host execution. It checks exact capabilities, records the canonical request,
accepts only an admission bound to that request, atomically reserves declared
maximum usage, seals the continuation, validates a typed resolution, settles
usage, and resumes. Budget evidence
contains before/reserved/actual/released/after ledgers and is independently
recomputed during replay. State updates remain pending until successful handler
completion.

`HostSession` adds a handshake and one-outstanding-reply protocol over that
state machine. It emits a preview before admission, then derives an execution
grant from the run ID, request, trace checkpoint, snapshot, and reserved usage.
The CLI atomically persists the snapshot and hash-chained trace before writing
that grant to the host. Transport framing stays in `aster-cli`; the runtime
session itself performs no I/O.

Source lexical bindings are alpha-renamed while lowering, so same-named locals
in nested blocks and match arms occupy distinct serializable VM entries. Pure
metadata uses the same scoped `if` and `match` evaluation without admitting an
effect instruction.

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

Host restore rewinds to the snapshot checkpoint, reconstructs the admitted
pending effect and reservation, performs a fresh handshake for the same run,
and re-emits the identical execution grant. It does not preview or admit the
effect again. If execution may already have happened, the host must use the
write's idempotency key or lookup; ASTER does not guess or silently retry.

Replay remains driver-free by construction. It never starts `HostSession`,
reads host input, or invokes an external adapter. The normative sequence and
frame fields are specified in
[the ASTER 0.2 host protocol](../spec/aster-host-protocol-0.2.md).
