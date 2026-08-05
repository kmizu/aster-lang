# ASTER 0.1 Bootstrap Design

Status: approved by the request to implement `ASTER_CODEX_BOOTSTRAP_PROMPT.md`

## Objective

ASTER 0.1 is a small governed-agent language whose executable boundary is:
model output becomes an opaque `Candidate`, a write becomes an immutable
`Proposal`, authority is represented by a single-use `Permit`, and the world is
accepted only after `Reconciliation`.

The bootstrap must be a real vertical slice. The parser, checker, formatter,
typed IR, deterministic VM, fixture-backed effect driver, trace recorder,
snapshot/resume path, replay verifier, and CLI all operate through general
declarations. No component recognizes the bundled meeting scheduler by file
name or hard-coded source text.

## Chosen approach

The syntax crate uses a hand-written lexer and recursive-descent parser. This
keeps byte spans, comments, recovery boundaries, and canonical formatting under
repository control without a parser-generator build dependency. Block comments
are nested, and source identifiers remain ASCII-only in 0.1.

Semantic analysis is a sequence of explicit passes over the parsed module:
declaration collection, name resolution, type and wrapper visibility checking,
purity/effect inference, capability coverage, affine-use checking, recursion
rejection, persistence restrictions, and IR lowering. Diagnostics are typed,
stable-code values rather than strings.

The IR is serializable and instruction-addressed. Pure instructions advance the
machine without I/O; external instructions yield an `EffectRequest`. The runtime
alone reserves budgets, checks concrete capability grants, invokes the one
driver boundary in record mode, validates resolutions, maintains affine
resources, writes snapshots, and commits state atomically.

Record and replay use canonical JSON and SHA-256. Replay validates the trace
hash chain and all run fingerprints, re-executes the VM, compares each yielded
request with the recorded request, injects the recorded response, recomputes
policy/budget/proposal/permit/reconciliation decisions, and never constructs a
driver.

## Alternatives rejected

An example-specialized interpreter would be smaller, but it would create fake
success paths and could not enforce the language invariants on arbitrary source.
A large parser/runtime framework would reduce some handwritten code, but adds
dependency and determinism risk without improving the narrow 0.1 grammar. A
recursive AST interpreter was also rejected because effectful host calls would
be hidden in control flow and snapshots could not represent continuation state
explicitly.

## Component boundaries

- `aster-diagnostics` owns spans, stable diagnostics, rendering, and the code
  registry.
- `aster-syntax` owns UTF-8 lexing, parsing, lossless comments, AST JSON, and
  canonical formatting.
- `aster-semantics` owns symbols, types, effects, capabilities, purity, taint,
  affine use, termination restrictions, and checked programs.
- `aster-ir` owns typed serializable instructions, program identity, machine
  values, and effect request/resolution schemas.
- `aster-runtime` owns deterministic execution, fixture matching, budgets,
  proposal hashes, permits, trace chains, snapshots, replay, and atomic state.
- `aster-cli` validates file boundaries and orchestrates lower layers without
  duplicating their rules.

Dependencies point only from the CLI toward lower layers. External effects
cross only the runtime driver trait.

## Data flow

`source -> tokens -> syntax module -> checked program -> IR program -> machine`

In record mode the machine yields a request, the runtime snapshots it, reserves
budget, resolves it through fixtures, validates and records the response, and
resumes. On successful completion the pending state delta is atomically
published. Replay takes the same path through the machine but obtains each
resolution exclusively from the verified trace.

## Error and security model

All user-controlled failures become diagnostics with stable families. CLI
boundaries reject malformed UTF-8 and versioned JSON before trusted domain
construction. Candidate and secret payloads are not exposed by generic value
operations. `Secret` has no source constructor and cannot cross prompt, state,
diagnostic, trace, snapshot, formatting, equality, or hashing boundaries.

Capabilities are exact runtime grants; source can only require them. Budget is
reserved before every driver call. Proposal hashing binds schema, program,
action, arguments, intent, risk, capability request, and idempotency key. Permit
consumption is checked both statically and in the runtime ledger.

## Testing strategy

Each behavior is developed red-green-refactor. Syntax tests cover malformed
input, exact spans, comment retention, AST normalization, and formatter
idempotence. Semantic golden tests cover every mandatory unsafe fixture and
stable diagnostic span. Runtime tests cover record/replay equivalence, zero
drivers on replay, tampering and divergence, proposal/permit binding, budgets,
capabilities, schema failures, state rollback, snapshots, and secret sentinels.
CLI tests assert outputs and exit codes. `./scripts/check.sh` is the final gate.

## Approval and scope

The user supplied the complete bootstrap prompt and explicitly requested that
the repository be built according to it. The prompt itself prohibits follow-up
clarification and defines tie-breaking priorities, so this document records that
already-approved design rather than opening a second design-choice gate.
