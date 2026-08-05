# ASTER 0.1 Language and Runtime Specification

Status: normative for the 0.1 bootstrap. Features named here are unavailable
until their implementation and conformance evidence land in the same change.

## Source and lexical form

An ASTER source file is valid UTF-8. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*`; dotted declaration paths are sequences of identifiers.
No Unicode normalization is performed. Decimal integer literals, JSON string
literals, triple-quoted block strings, `//` comments, and nested `/* */`
comments are supported. Unterminated or malformed tokens are errors with exact
byte spans.

The parser accepts one `module` declaration followed by declarations. The
concrete declarations are `type`, `enum`, `capability`, `fn`, `flow`, `prompt`,
`validator`, `tool`, `policy`, and `agent`. Declaration order does not affect
resolution.

## Types and values

Primitive types are `Unit`, `Bool`, `Int`, `Text`, `Instant`, `Duration`,
`ProvenanceRef`, and `Error`. Compiler-known constructors are `Option<T>`,
`Result<T,E>`, `List<T>`, `Incoming<T>`, `Untrusted<T>`, `Candidate<T>`,
`Checked<T>`, `Observation<T>`, `Secret<T>`, `Intent<P>`, `Proposal<A>`,
`Permit<A>`, `Receipt<A>`, and `Reconciled<A>`.

Programs may define non-generic aliases, records, single-payload or nullary
enums, and capability signatures. There is no null, inheritance, user generic,
implicit conversion, exception, reflection, macro, dynamic type, or cast.

`Incoming`, `Untrusted`, `Checked`, `Observation`, `Receipt`, and `Reconciled`
expose their documented `value`. `Candidate`, `Secret`, capabilities, and
permits expose no payload. Equality is structural only for equatable types and
is unavailable for opaque governance values.

## Pure computation and effects

Expressions include literals, records, constructors, lists, projections,
statically resolved calls, unary and arithmetic/comparison/boolean operators,
`if`, finite `match`, and `Result` propagation with `?`. Statements are `let`,
`require`, transactional `update state`, `return`, and expression statements.
There are no loops, recursion, closures, mutable locals, or detached tasks.

`fn`, validators, and policy conditions are pure and deterministic. A `flow`
declares an upper bound in `uses`; inferred effects must be its subset. Event
handler effects must be covered by the agent's `requires` list.

Inference has type `Result<Candidate<T>, Error>`. Only
`validate candidate with Validator` can create `Checked<T>`. Read tools execute
only through `observe`. Write tools execute only through
`intent -> propose -> authorize -> commit`, and successful writes are checked
by `reconcile`.

## Capabilities, policies, affine values, and budgets

Source declares capability requirements but cannot mint grants. Runtime grants
are versioned JSON values and must exactly match the evaluated typed request.

Policies are ordered total decision tables. The first matching `allow`,
`approve`, or `deny` rule wins and a final `otherwise` is mandatory. Approval is
an external suspension after pure policy evaluation.

Proposals are immutable. Permit and proposal consumption is affine; a commit
requires matching action types and the runtime also checks proposal hash,
expiry, capability fingerprint, unique permit identity, and unused status.

Per-event budget dimensions are `model_calls`, `model_tokens`,
`external_reads`, `external_writes`, `approvals`, and `money_microunits`.
Omitted dimensions are zero. Reservations happen before driver invocation and
settlement rejects actual usage above the fixture-declared maximum.

## Prompt, secret, and boundary rules

Every prompt has exactly one static triple-quoted instruction and a structured
data channel. Instructions cannot interpolate or evaluate expressions.
`Secret<T>` has no source constructor and cannot enter prompts, state,
ordinary diagnostics, console output, traces, snapshots, equality, hashing, or
string conversion.

Event, state, capability, fixture, trace, snapshot, and final-state JSON each
carry `schema_version: 1`; unknown fields and privileged wrapper construction
are rejected at the boundary. Instants normalize to RFC 3339 UTC seconds ending
in `Z`. Runtime JSON objects use deterministic key ordering.

## Machine, trace, replay, and state

Effectful source lowers to serializable explicit instructions. Machine state
contains the instruction pointer, frames, locals, current state, pending state
delta, budgets, capability fingerprint, affine ledger, trace position, and any
pending request. No snapshot contains a host closure.

Trace entries contain schema version, run ID, sequence, kind, payload, previous
entry hash, and entry hash. Hashing uses recursively key-sorted canonical JSON
and SHA-256. Replay verifies the chain and source/input/state/capability
fingerprints, re-executes deterministic semantics, compares each request, and
uses recorded resolutions without a driver. Any mismatch is a hard divergence.
Failed handlers never publish pending state.

## Formatting and diagnostics

Canonical formatting preserves comments and prompt instruction contents,
normalizes layout and punctuation, emits one trailing newline, and is
idempotent. Malformed syntax is diagnosed rather than discarded.

Diagnostics use the stable families documented in
[diagnostics.md](../design-docs/diagnostics.md). JSON field order and lists are
deterministic. CLI exit codes are 0 success, 1 source/check/format failure, 2
runtime/fixture/capability/budget/policy failure, 3 replay/tamper/schema
divergence, and 4 internal invariant failure.

## Explicit non-goals

ASTER 0.1 has no live model provider, MCP/OpenAPI client, arbitrary network or
shell operation, FFI, package import, multi-agent spawn, vector memory,
heartbeat scheduler, compensation saga, distributed execution, generated-code
evaluation, user generic, concurrency, loops, recursion, theorem proving, trace
encryption, or backward-compatibility promise.
