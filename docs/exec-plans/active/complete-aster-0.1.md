# Complete ASTER 0.1 Execution Plan

> **For agentic workers:** Execute this plan inline. Repository instructions
> prohibit delegated/subagent implementation for this task. Every behavior
> change follows a red-green-refactor cycle.

## Goal and acceptance criteria

Close the gap between the green meeting-scheduler vertical path and the full
ASTER 0.1 language/runtime contract in `ASTER_CODEX_BOOTSTRAP_PROMPT.md`.
Completion requires direct current-state evidence for every requirement in
sections 2, 4, 6–20, and 23–30; a passing happy-path example is not sufficient.

In particular:

- invalid declarations, types, calls, records, intents, policies, validators,
  tools, patterns, and affine dataflow fail with stable diagnostics;
- every accepted source construct has deterministic checker, lowering, and VM
  behavior rather than recovering through `Type::Unknown`;
- the complete mandatory runtime/replay test matrix remains green;
- normative docs describe the implemented behavior exactly;
- the final record and replay outputs are byte-identical with zero replay
  driver construction or calls.

## Global constraints

- Preserve all twenty non-negotiable invariants in prompt section 4.
- Keep the dependency direction
  `diagnostics <- syntax <- semantics <- ir <- runtime <- cli`.
- Use stable Rust, `#![forbid(unsafe_code)]`, typed errors, stable diagnostic
  codes, deterministic collections/order, and no production panic paths.
- Add no network, async-runtime, shell, FFI, plugin, or live-provider dependency.
- Never turn a malformed program into a checked program by treating an
  unresolved or contradictory type as a compatible wildcard.
- Add tests before production changes and observe the expected failure.

## File map

- `crates/aster-semantics/src/model.rs`: resolved declaration/type catalog and
  type-property queries.
- `crates/aster-semantics/src/checker.rs`: declaration passes, executable-body
  orchestration, recursion, and whole-program diagnostics.
- `crates/aster-semantics/src/expression.rs`: expression typing, lexical
  environments, call/record/pattern validation, effects, and affine flow.
- `crates/aster-semantics/src/types.rs`: resolved type domain and type
  properties such as equality, persistence, and affinity.
- `crates/aster-semantics/tests/conformance.rs`: compile-pass/fail golden
  harness and focused semantic regressions.
- `tests/conformance/{pass,fail}`: canonical source fixtures with expected
  stable diagnostics.
- `crates/aster-ir/src/lowering.rs`: checked-AST to explicit IR lowering; it
  may assume, but must validate, checker invariants.
- `crates/aster-ir/tests/lowering.rs`: general source-control-flow and
  serialization regressions.
- `crates/aster-runtime/src/{machine,run,value,fixture}.rs`: deterministic VM
  semantics and typed boundary decoding.
- `crates/aster-runtime/tests/{machine,record_replay,governance}.rs`: runtime
  invariant and mandatory matrix tests.
- `crates/aster-cli/{src/lib.rs,tests/cli.rs}`: boundary schemas, exit classes,
  atomic artifacts, and public commands.
- `docs/spec/aster-0.1.md`, `ARCHITECTURE.md`, and design docs: normative and
  architectural truth after behavior changes.

## Milestones

- [ ] M1: Declaration and type well-formedness is closed.
- [ ] M2: Expressions, records, calls, built-ins, and patterns are fully typed.
- [ ] M3: Affine values are sound across aliases and control-flow joins.
- [ ] M4: Every accepted construct lowers and executes deterministically.
- [ ] M5: Boundary schemas and all mandatory test obligations have direct tests.
- [ ] M6: Documentation, full checks, record/replay demonstration, commit, and
      push are complete.

## Task 1: Declaration and type well-formedness

**Interfaces**

- Consumes: syntax declarations and `TypeReference` values.
- Produces: a `Model` in which every resolved type/declaration lookup is known
  valid before body checking and lowering.

- [ ] Add fail fixtures/tests for unknown types; wrong built-in generic arity;
      cyclic aliases; duplicate record fields, parameters, enum variants,
      state fields, handlers, prompt data names, and tool metadata; missing
      tool mode/capability/sensitivity/risk; invalid idempotency parameters;
      undeclared capability kinds; and invalid wrapper placement.
- [ ] Run `cargo test -p aster-semantics --test conformance` and confirm each
      new case fails because the checker incorrectly accepts it or emits the
      wrong stable diagnostic.
- [ ] Add explicit declaration/type-validation passes and the minimum new
      diagnostic codes. Never rely on `Type::Unknown` as successful recovery.
- [ ] Re-run the focused tests and `cargo test -p aster-semantics` to green.
- [ ] Update the diagnostic registry/reference for every new public code.

## Task 2: Exact expression, call, record, intent, and pattern typing

**Interfaces**

- Consumes: the validated `Model` from Task 1.
- Produces: exact expression types and branch environments for lowering.

- [ ] Add focused fail/pass tests proving: positional/named argument arity and
      uniqueness; required/extraneous record fields; exact five-field intent
      shape and types; validator/policy compatibility; equatable-only equality;
      integer-only ordering; homogeneous `contains`/`subset`; valid
      `provenance` inputs; typed `if` results; enum/`Option`/`Result` pattern
      binding and exhaustive arms; duplicate/unreachable wildcard rejection;
      and branch-local binding isolation.
- [ ] Verify the new focused test fails on current behavior. A representative
      initial assertion is that `fn f() -> Int { return if true { 1; } else {
      false; }; }` must emit `ASTER-TYPE-2002`, while matching enum payloads
      introduces the payload name only inside its arm.
- [ ] Implement exact argument mapping, field-set comparison, intent schema
      validation, operator type properties, branch result unification, pattern
      resolution/binding, and exhaustiveness checks in cohesive helpers.
- [ ] Re-run semantic tests, syntax formatter/idempotence tests, and clippy.

## Task 3: Sound affine dataflow

**Interfaces**

- Consumes: typed lexical environments and structured control flow.
- Produces: deterministic post-expression move states accepted by lowering.

- [ ] Add red tests for proposal/permit aliasing, moving through a function
      argument, commit in one `if`/`match` branch followed by reuse, both-branch
      consumption, and branch-local values escaping their scope.
- [ ] Replace the single mutable `moved` bit behavior with explicit affine
      ownership transfer and conservative control-flow joins. A value consumed
      on any reachable branch is unavailable after the join.
- [ ] Ensure commit consumes the exact proposal and permit expressions once,
      ordinary aliasing transfers ownership, and diagnostics distinguish
      proposal from permit use-after-move.
- [ ] Run all semantic conformance tests and add an IR/runtime low-level
      regression showing forged/deserialized reuse remains rejected.

## Task 4: General lowering and VM coverage

**Interfaces**

- Consumes: only fully checked programs.
- Produces: serializable explicit instructions that execute all accepted 0.1
  constructs without recursive AST effect evaluation.

- [ ] Add red IR/runtime tests for typed `if`/`match` result merging, payload
      variables, pure function/flow calls, every required built-in, enum,
      `Option`, and `Result` matching, arithmetic error paths, and state update
      expressions.
- [ ] Remove lowering/runtime assumptions exposed by the tests; return
      `ASTER-INTERNAL-9901` only for genuinely unreachable checked-program
      invariants and typed runtime diagnostics for user/runtime data failures.
- [ ] Prove IR/snapshot round trips, stable instruction identities, no closure
      capture, and deterministic execution across repeated runs.

## Task 5: Boundary and mandatory-matrix audit

- [ ] Build a requirement-to-test matrix for prompt sections 23 and 24 in this
      plan's progress log. Add any missing direct tests, including changed
      initial-state fingerprints, malformed UTF-8/source/JSON/trace/snapshot,
      canonical ordering, exact CLI exit classes, and every secret sentinel
      sink.
- [ ] Add schema tests for unknown fields and non-`1` versions at event, state,
      capability, fixture, trace, snapshot, resolution, IR, and output-state
      boundaries.
- [ ] Confirm run failures preserve a valid partial hash chain and atomically
      avoid publishing final state; confirm replay never constructs a driver.
- [ ] Run `cargo test --workspace --all-features` and clippy to green.

## Task 6: Documentation and final evidence

- [ ] Update the normative spec, architecture, security/runtime docs,
      diagnostics reference, README, and this plan so every accepted/rejected
      behavior agrees with code.
- [ ] Self-review the diff for placeholders, semantic shortcuts, secret
      leakage, nondeterminism, stale claims, and unrelated changes.
- [ ] Move this plan to `docs/exec-plans/completed/` only after all earlier
      milestones are proven.
- [ ] Run, after the final edit: `git diff --check`, `./scripts/check.sh`, the
      documented `aster check`, `run`, and `replay` commands, `cmp` on final
      states, and a representative JSON compile-fail command.
- [ ] Inspect artifact permissions/ignore status and scan trace/snapshots/output
      for the secret sentinel.
- [ ] Commit the reviewed diff, push the branch, and report exact evidence and
      residual risks. Mark the persistent goal complete only if every item is
      proven.

## Decisions and rationale

- The 1,942-line bootstrap prompt and normative 0.1 spec are already the
  user-approved design. This plan does not introduce a new language direction;
  it restores implementation conformance and therefore does not reopen design
  questions the prompt explicitly instructed the implementer to resolve.
- Static acceptance must imply lowerability and runtime meaning. `Unknown` is
  reserved for diagnostic recovery inside a failing compilation, never for a
  successful checked program.
- Well-formedness, expression typing, and affine flow stay separate so future
  agents can audit each invariant without reading a monolithic checker.
- Stable codes are added rather than reusing an existing code for a new
  meaning.

## Progress log

- 2026-08-06: Created `agent/complete-aster-0.1` from the clean published
  `main` branch. The fresh baseline workspace suite contained 63 passing tests.
- 2026-08-06: Added red tests showing that unknown/cyclic/shadowing types,
  duplicate declaration members, incomplete tool metadata, invalid
  idempotency names, prompt data drift, invalid validator/policy signatures,
  and undeclared tool capabilities were accepted. Added declaration/type
  well-formedness checks and made all of those tests green.
- 2026-08-06: Added red tests for exact call arguments and record fields,
  branch-result typing, equatable-only equality, integer ordering,
  homogeneous collection built-ins, enum payload bindings/exhaustiveness,
  intent shape, validator selection, governance action types, typed policy
  decisions, and explicit routine returns. Implemented exact checking and made
  the full conformance suite green.
- 2026-08-06: Added affine regressions for alias ownership transfer and
  conservative `if` joins, plus pure-function commit rejection. Implemented
  move propagation and external-effect checking; retained the valid governed
  meeting and direct-allow paths.
- 2026-08-06: Added declaration-boundary regressions for exact capability
  requests and the rule that `Secret<T>` is legal only in parameters of a
  sensitivity-secret tool. Split both into independent semantic passes and
  made the focused tests and clippy green.
- 2026-08-06: Rejected unknown prompt, tool, policy, and reconciliation
  validator references instead of silently accepting `Type::Unknown`.
  Reconciliation now requires a declared two-parameter validator and checks
  both declared parameter types against the receipt result and observation.

## Discoveries and deviations

- 2026-08-06: The previous completed plan and green suite prove the governed
  meeting path, replay, budgets, permits, snapshots, and required unsafe
  fixtures, but not full static-language closure.
- 2026-08-06: `if` currently returns `Type::Unknown`; accepted programs can
  therefore hide branch/return mismatches from the checker.
- 2026-08-06: call argument and record construction checking currently validates
  only supplied values, not missing, extra, duplicate, or unknown names.
- 2026-08-06: match checking does not bind payload variables or prove
  exhaustiveness; affine state is not joined soundly across branches.
- 2026-08-06: tool metadata lowering silently omits a tool with no mode and
  carries other required metadata as optional instead of rejecting the source.
- No limitation above is accepted; each remains open until implemented and
  verified.

## Commands and results

- `git status -sb`: clean `main`, tracking `origin/main`.
- `cargo test --workspace --all-features -- --list`: 63 tests enumerated;
  existing coverage is concentrated on the vertical path and mandatory named
  fixtures.
- Source audit of `checker.rs`, `expression.rs`, `model.rs`, and `lowering.rs`:
  exposed the static-semantic gaps recorded above.
- `cargo test -p aster-semantics --test conformance`: 38 passed after the
  declaration, expression, initial affine, capability, secret-placement, and
  effect-reference TDD cycles.
- `cargo test --workspace --all-features`: passed with 97 tests after the
  checkpoint changes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed after splitting record/list checking out of the expression dispatcher.
- `git diff --check`: passed at the checkpoint.

## Known limitations

- ASTER 0.1 is not complete while any milestone in this plan remains open.
