# ASTER 0.1 Bootstrap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for
> tracking. This repository's harness forbids subagent delegation, so execution
> remains inline.

**Goal:** Build the complete, tested, documented, runnable ASTER 0.1 governed
agent language and deterministic record/replay runtime.

**Architecture:** A six-crate dependency chain turns lossless source into a
checked module, lowers it to serializable instructions, and executes those
instructions in a deterministic effect-yielding VM. The runtime is the sole I/O
boundary and supports fixture-backed recording, hash-chained traces, snapshots,
resume, and driver-free semantic replay.

**Tech Stack:** Stable Rust 1.96, `serde`, `serde_json`, `thiserror`, `clap`,
`sha2`, `hex`, and Rust integration tests.

## Global Constraints

- Preserve all twenty invariants in `ASTER_CODEX_BOOTSTRAP_PROMPT.md` section 4.
- Keep dependency direction exactly as specified in `AGENTS.md`.
- Use `#![forbid(unsafe_code)]` in every crate.
- No production placeholder, panic path for user input, network client, async
  runtime, arbitrary shell, native plugin, or ambient time/randomness.
- Every externally visible ordering and serialization must be deterministic.
- Add tests before production behavior and observe each test fail for the
  intended reason.

---

### Task 1: Workspace, diagnostics, and repository gates

**Files:** Create root Cargo/config files, all crate manifests and `lib.rs`
roots, `aster-diagnostics` domain modules, scripts, CI, and documentation
skeletons listed in bootstrap section 5.

**Interfaces:** Produce `Span`, `Diagnostic`, `DiagnosticCode`, human/JSON
rendering, and the checked-in diagnostic registry. All later crates consume
these types.

- [x] Write diagnostics serialization/rendering and registry tests.
- [x] Run the narrow tests and confirm missing symbols/behavior fail.
- [x] Implement the workspace and diagnostics crate minimally.
- [x] Run format, diagnostics tests, architecture script, and docs script.
- [x] Record exact results in the active execution plan.

### Task 2: Lossless syntax and canonical formatting

**Files:** Create focused lexer, token, AST, parser, formatter, and source modules
under `crates/aster-syntax/src`, plus syntax fixtures and tests.

**Interfaces:** Produce `parse(SourceFile) -> Result<Module, Vec<Diagnostic>>`,
`format(&Module) -> String`, and serializable spanned AST types preserving
comments and instruction block content.

- [x] Add lexer/parser failure tests with exact byte/line/column spans.
- [x] Observe failures for missing lexical and declaration grammar.
- [x] Implement tokens and recursive-descent declarations/expressions.
- [x] Add formatter idempotence, comment, and parse-format-parse tests.
- [x] Implement canonical formatting and run all syntax tests.

### Task 3: Static semantics and conformance

**Files:** Create symbols, types, declarations, checker passes, effect sets,
capability patterns, affine ledger, termination analysis, and checked-program
modules under `aster-semantics`; add every section 23 fixture and golden result.

**Interfaces:** Produce `check(&Module) -> Result<CheckedProgram,
Vec<Diagnostic>>`, with resolved symbols, expression types, effect bounds, and
affine identities used by lowering.

- [x] Add one failing golden test for each mandatory unsafe fixture.
- [x] Confirm each fails because the rule is not implemented, not parse noise.
- [x] Implement passes in the specified order with stable diagnostic codes.
- [x] Add valid direct-allow and human-approval compile-pass fixtures.
- [x] Run all semantic and conformance tests and update diagnostics docs.

### Task 4: Typed IR and lowering

**Files:** Create IR types, instructions, blocks, values, effects, serialization,
program hashing, and lowering modules under `aster-ir`.

**Interfaces:** Produce a versioned `Program`, stable instruction IDs,
serializable frames/locals, explicit state delta operations, and explicit
external effect suspension instructions.

- [ ] Add IR round-trip, stable-ID, and no-hidden-effect tests.
- [ ] Confirm tests fail against missing IR/lowering.
- [ ] Implement IR domain types and lower checked programs.
- [ ] Lower the bundled scheduler and assert its ordered effect points.
- [ ] Run IR and upstream test suites.

### Task 5: Deterministic runtime and recording

**Files:** Create VM, values, schema validation, canonical JSON, fixture driver,
budgets, capability grants, proposal/permit ledger, policy, state transaction,
trace, snapshot, and atomic-file modules under `aster-runtime`.

**Interfaces:** Produce `Machine::step`, `EffectDriver::resolve`, `record_run`,
versioned trace/snapshot/state types, and deterministic proposal/entry hashes.

- [ ] Add focused failing tests for runtime requirements 3-7 and 14-29.
- [ ] Implement pure VM stepping and typed effect yielding.
- [ ] Implement pre-driver capability/budget gates and fixture resolution.
- [ ] Implement proposal hashes, permits, consumption, commit, reconciliation,
      and transactional state publication.
- [ ] Implement append-only traces and snapshots; run runtime tests.

### Task 6: Semantic replay and resume

**Files:** Add replay verification and resume modules and their adversarial
fixtures under `aster-runtime`.

**Interfaces:** Produce `replay_run` without an `EffectDriver` parameter and
`resume(snapshot, resolution)` with complete request/fingerprint checks.

- [ ] Add failing tests for zero drivers, chain tamper, recomputed malicious
      chain, source/input/state changes, reordered effects, and mismatch.
- [ ] Implement full chain and fingerprint verification.
- [ ] Re-step the VM and inject only matching recorded resolutions.
- [ ] Implement snapshot resume and same-final-state assertion.
- [ ] Run all record/replay/resume tests.

### Task 7: CLI, example, docs, and final gates

**Files:** Implement `aster-cli`, meeting example JSON/source files, black-box
tests, complete README/architecture/spec/design docs/ADRs, scripts, and CI.

**Interfaces:** Produce all commands and exit codes in bootstrap section 19 and
the exact documented record/replay workflow.

- [ ] Add failing CLI tests for every command class and exit code.
- [ ] Implement CLI boundary validation and atomic writes through lower APIs.
- [ ] Complete example and documentation, then make docs checks strict.
- [ ] Run all section 30 commands and inspect every artifact.
- [ ] Audit each prompt requirement against direct evidence, move the execution
      plan to completed only when every item is proven, and record results.
