# Bootstrap ASTER 0.1 Execution Plan

## Goal and acceptance criteria

Build the complete ASTER 0.1 vertical slice specified by
`ASTER_CODEX_BOOTSTRAP_PROMPT.md`. Completion requires every item in its
sections 2, 23, 24, 25, 26, and 30 to have direct current-state evidence.

## Milestones

- [x] M1: Workspace, documentation skeleton, diagnostics, and mechanical checks.
- [x] M2: Lexer, parser, lossless comments, AST JSON, and canonical formatter.
- [x] M3: Names, types, wrappers, purity/effects, capabilities, affine analysis,
      recursion rejection, persistence checks, and conformance fixtures.
- [x] M4: Typed serializable IR and the meeting scheduler lowering path.
- [ ] M5: Deterministic VM, fixtures, budgets, capabilities, proposals, permits,
      commit, reconciliation, atomic state, trace, and snapshots.
- [ ] M6: Replay and resume with tamper/divergence validation and zero drivers.
- [ ] M7: CLI, example artifacts, black-box tests, CI, and complete docs.
- [ ] M8: Requirement-by-requirement audit and all final validation commands.

## Decisions and rationale

- Hand-written lexer/parser: exact spans and formatting control with few
  dependencies.
- Nested block comments: deterministic and less surprising for generated code.
- Explicit instruction IR: serializable continuations and effect isolation.
- `BTreeMap` or explicit sorting for observable maps: stable formatting and
  hashes.
- RFC 3339 UTC timestamps normalize to `YYYY-MM-DDTHH:MM:SSZ`; no ambient clock.
- Canonical JSON recursively sorts keys and uses JSON's integral number form.
- Regular block comments nest; canonical formatting attaches comments to the
  next containing/following declaration while preserving text and source order.
- Records and enums require at least one field/variant in 0.1. This makes empty
  `{}` after `if` and `match` unambiguous and keeps matchable enums inhabited.
- Fixture matching uses effect kind, declaration identity, request hash, then
  queue position only to disambiguate exact duplicate requests.
- Replay reconstructs and steps the machine; recorded final output is never
  treated as execution.

## Progress log

- 2026-08-05: Inspected the repository. It contained only `AGENTS.md` and the
  1,942-line bootstrap prompt and was not yet a Git repository.
- 2026-08-05: Read the complete bootstrap prompt and confirmed that it is the
  approved design input and explicitly forbids clarification questions.
- 2026-08-05: Selected the general hand-written compiler plus explicit-VM
  architecture; rejected example specialization and recursive effect execution.
- 2026-08-05: Initialized the six-crate Rust workspace, stable diagnostic domain,
  architecture/docs/production-source checks, CI entry point, normative docs,
  security/runtime design docs, and the three required ADRs.
- 2026-08-05: The first diagnostics TDD cycle failed on missing public symbols,
  then passed five behavioral tests. Architecture and production-source checkers
  were also verified against deliberately invalid temporary workspaces.
- 2026-08-05: Implemented the lossless lexer, serializable AST, recoverable
  recursive-descent/Pratt parser, and canonical AST formatter. The complete
  meeting scheduler parses through every governed-action expression and its
  canonical format round-trips to the same normalized AST.
- 2026-08-05: Parser recovery reports independent malformed declarations in
  source order. Formatter tests preserve comments and block-string contents and
  prove byte-identical second formatting.
- 2026-08-05: Added deterministic declaration collection, type/field checking,
  opaque candidate enforcement, pure/effect and capability checks, affine
  proposal/permit consumption, policy totality, recursion rejection, budget and
  persistence restrictions, and secret-to-model rejection. All mandatory
  compile-fail fixtures now assert a stable code and relevant source span; the
  meeting scheduler and a direct-allow governed-write fixture compile.
- 2026-08-05: Added versioned typed IR with stable value/instruction identities,
  explicit routine calls and branches, pending state updates, and distinct
  inference/observation/validation/intent/proposal/authorization/commit/
  reconciliation instructions. The meeting scheduler lowers without hidden AST
  effect evaluation, and IR JSON validates its content hash on read.
- 2026-08-05: Began the runtime authority substrate with canonical JSON,
  proposal hashing over every authority-relevant field, proposal-bound expiring
  single-use permits, deterministic budget reservation/settlement, hash-chained
  traces, and pre-serialization secret rejection.
- 2026-08-05: Implemented the explicit-instruction VM through the complete
  meeting path: model candidate, validation, observation, intent/proposal,
  direct or approval policy authorization, affine permit consumption, commit,
  reconciliation, and atomic state publication. Effect-boundary snapshots
  preserve frames, slots, budgets, pending requests, and authority state.
- 2026-08-05: Added an exact fixture driver with pure preview before admission,
  variable-usage reservation, counted resolution, record-mode hash-chain traces,
  and driver-free semantic replay. The meeting record and replay produce the
  same canonical final state; changed input fails before effects.
- 2026-08-05: Replaced caller-supplied grant fingerprints with versioned exact
  capability grants. Agent admission and every model/read/approval/write
  boundary now verify the canonical capability request before an effect can be
  yielded; snapshots retain only the verified fingerprint and exact request
  hashes.
- 2026-08-05: Added strict typed decoding for event/state/model/tool boundaries,
  alias expansion, unknown-state/record-field rejection, canonical UTC instant
  validation with cross-day `add_seconds`, and state-default initialization.
- 2026-08-05: Implemented the `aster` binary with check, format, AST JSON,
  fixture-backed run, driver-free replay, durable resume, explain, atomic state
  and snapshot writes, and canonical hash-chained JSON Lines trace persistence.
  Added all four versioned meeting-scheduler input artifacts and demonstrated
  byte-identical record/replay output through the public CLI.
- 2026-08-05: Hardened replay evidence: VM-originated policy, permit, commit,
  and reconciliation audit events are recorded and independently recomputed;
  trace prefixes are bound into snapshots; program/event/state/capability
  fingerprints are explicit; maliciously rehashed results, program changes,
  changed inputs, and reordered requests now have direct regression tests.
- 2026-08-05: Added black-box CLI tests and runtime regressions for exact grant
  failure, pre-driver model/write budget rejection, model/tool schema mismatch,
  atomic failed-handler state, generic secret serialization rejection, and
  fixture actual-usage overflow. Registered runtime/replay/capability/budget/
  internal diagnostic families and remediation text.
- 2026-08-05: Persisted capability declaration signatures into IR and now
  reject undeclared, ill-typed, duplicate, or unsupported runtime grant files
  before admission. All versioned external runtime structures reject unknown
  fields rather than silently ignoring schema drift.
- 2026-08-05: Added evidence-bearing record failures. Driver/schema/usage/VM
  failures now end a valid partial chain with `run_failed`; the CLI atomically
  persists that trace and prior snapshots. Regression coverage includes
  actual-usage overflow after exactly one driver call and expired authority
  before any write call.

## Discoveries and deviations

- The repository has no prior Rust workspace, history, active plans, README,
  architecture document, specification, or local nested `AGENTS.md`.
- There is no existing worktree to preserve. Git initialization is part of the
  bootstrap, but history will not be rewritten once created.

## Commands and results

- `find . -maxdepth 3 -mindepth 1 -print | sort`: only the two seed files.
- `rustc --version`: `rustc 1.96.0 (ac68faa20 2026-05-25)`.
- `cargo --version`: `cargo 1.96.0 (30a34c682 2026-05-25)`.
- `cargo test --workspace --all-features`: 5 tests passed, 0 failed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed after documenting public error contracts and scoping the panic API scan
  to production Rust rather than tests.
- `bash scripts/tests/check-architecture.sh`: passed both valid and forbidden-edge
  cases.
- `bash scripts/tests/check-docs.sh`: passed with active-bootstrap allowance and
  rejected a repository missing required documents.
- `bash scripts/tests/check-production-rust.sh`: passed and rejected production
  use of `expect` while permitting it in tests.
- `cargo test -p aster-syntax --all-features`: lexer, parser, formatter,
  recovery, full-example, and control-expression tests passed.
- `cargo clippy -p aster-syntax --all-targets --all-features -- -D warnings`:
  passed after boxing internal parser diagnostics and splitting formatter
  responsibilities rather than suppressing lints.
- `cargo test -p aster-semantics --test conformance`: 3 tests passed, covering
  21 unsafe fixtures plus meeting-scheduler and direct-allow pass programs.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed with the complete static-semantics layer enabled.
- `cargo test -p aster-ir --test lowering`: 3 tests passed for stable identity
  and JSON round-trip, explicit meeting governance order, and control-flow
  branch targets.
- `cargo test -p aster-runtime --test governance`: 5 tests passed for proposal
  binding, affine permit consumption, budgets, trace tamper detection, and
  secret-safe snapshot rejection.
- `cargo test -p aster-runtime --tests`: 10 tests passed, including pending
  snapshot restore, direct-allow zero-approval execution, full meeting approval
  execution, fixture-backed recording, driver-free replay, and fingerprint
  mismatch rejection.

## Known limitations

None accepted. Any unavoidable limitation discovered during implementation must
be recorded here and in the specification and README before completion.
