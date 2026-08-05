# Bootstrap ASTER 0.1 Execution Plan

## Goal and acceptance criteria

Build the complete ASTER 0.1 vertical slice specified by
`ASTER_CODEX_BOOTSTRAP_PROMPT.md`. Completion requires every item in its
sections 2, 23, 24, 25, 26, and 30 to have direct current-state evidence.

## Milestones

- [ ] M1: Workspace, documentation skeleton, diagnostics, and mechanical checks.
- [ ] M2: Lexer, parser, lossless comments, AST JSON, and canonical formatter.
- [ ] M3: Names, types, wrappers, purity/effects, capabilities, affine analysis,
      recursion rejection, persistence checks, and conformance fixtures.
- [ ] M4: Typed serializable IR and the meeting scheduler lowering path.
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

## Discoveries and deviations

- The repository has no prior Rust workspace, history, active plans, README,
  architecture document, specification, or local nested `AGENTS.md`.
- There is no existing worktree to preserve. Git initialization is part of the
  bootstrap, but history will not be rewritten once created.

## Commands and results

- `find . -maxdepth 3 -mindepth 1 -print | sort`: only the two seed files.
- `rustc --version`: `rustc 1.96.0 (ac68faa20 2026-05-25)`.
- `cargo --version`: `cargo 1.96.0 (30a34c682 2026-05-25)`.

## Known limitations

None accepted. Any unavoidable limitation discovered during implementation must
be recorded here and in the specification and README before completion.
