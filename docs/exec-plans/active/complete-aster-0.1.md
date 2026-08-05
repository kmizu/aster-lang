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

- [x] M1: Declaration and type well-formedness is closed.
- [x] M2: Expressions, records, calls, built-ins, and patterns are fully typed.
- [x] M3: Affine values are sound across aliases and control-flow joins.
- [x] M4: Every accepted construct lowers and executes deterministically.
- [x] M5: Boundary schemas and all mandatory test obligations have direct tests.
- [ ] M6: Documentation, full checks, record/replay demonstration, commit, and
      push are complete.

## Task 1: Declaration and type well-formedness

**Interfaces**

- Consumes: syntax declarations and `TypeReference` values.
- Produces: a `Model` in which every resolved type/declaration lookup is known
  valid before body checking and lowering.

- [x] Add fail fixtures/tests for unknown types; wrong built-in generic arity;
      cyclic aliases; duplicate record fields, parameters, enum variants,
      state fields, handlers, prompt data names, and tool metadata; missing
      tool mode/capability/sensitivity/risk; invalid idempotency parameters;
      undeclared capability kinds; and invalid wrapper placement.
- [x] Run `cargo test -p aster-semantics --test conformance` and confirm each
      new case fails because the checker incorrectly accepts it or emits the
      wrong stable diagnostic.
- [x] Add explicit declaration/type-validation passes and the minimum new
      diagnostic codes. Never rely on `Type::Unknown` as successful recovery.
- [x] Re-run the focused tests and `cargo test -p aster-semantics` to green.
- [x] Update the diagnostic registry/reference for every new public code.

## Task 2: Exact expression, call, record, intent, and pattern typing

**Interfaces**

- Consumes: the validated `Model` from Task 1.
- Produces: exact expression types and branch environments for lowering.

- [x] Add focused fail/pass tests proving: positional/named argument arity and
      uniqueness; required/extraneous record fields; exact five-field intent
      shape and types; validator/policy compatibility; equatable-only equality;
      integer-only ordering; homogeneous `contains`/`subset`; valid
      `provenance` inputs; typed `if` results; enum/`Option`/`Result` pattern
      binding and exhaustive arms; duplicate/unreachable wildcard rejection;
      and branch-local binding isolation.
- [x] Verify the new focused test fails on current behavior. A representative
      initial assertion is that `fn f() -> Int { return if true { 1; } else {
      false; }; }` must emit `ASTER-TYPE-2002`, while matching enum payloads
      introduces the payload name only inside its arm.
- [x] Implement exact argument mapping, field-set comparison, intent schema
      validation, operator type properties, branch result unification, pattern
      resolution/binding, and exhaustiveness checks in cohesive helpers.
- [x] Re-run semantic tests, syntax formatter/idempotence tests, and clippy.

## Task 3: Sound affine dataflow

**Interfaces**

- Consumes: typed lexical environments and structured control flow.
- Produces: deterministic post-expression move states accepted by lowering.

- [x] Add red tests for proposal/permit aliasing, moving through a function
      argument, commit in one `if`/`match` branch followed by reuse, both-branch
      consumption, and branch-local values escaping their scope.
- [x] Replace the single mutable `moved` bit behavior with explicit affine
      ownership transfer and conservative control-flow joins. A value consumed
      on any reachable branch is unavailable after the join.
- [x] Ensure commit consumes the exact proposal and permit expressions once,
      ordinary aliasing transfers ownership, and diagnostics distinguish
      proposal from permit use-after-move.
- [x] Run all semantic conformance tests and add an IR/runtime low-level
      regression showing forged/deserialized reuse remains rejected.

## Task 4: General lowering and VM coverage

**Interfaces**

- Consumes: only fully checked programs.
- Produces: serializable explicit instructions that execute all accepted 0.1
  constructs without recursive AST effect evaluation.

- [x] Add red IR/runtime tests for typed `if`/`match` result merging, payload
      variables, pure function/flow calls, every required built-in, enum,
      `Option`, and `Result` matching, arithmetic error paths, and state update
      expressions.
- [x] Remove lowering/runtime assumptions exposed by the tests; return
      `ASTER-INTERNAL-9901` only for genuinely unreachable checked-program
      invariants and typed runtime diagnostics for user/runtime data failures.
- [x] Prove IR/snapshot round trips, stable instruction identities, no closure
      capture, and deterministic execution across repeated runs.

## Task 5: Boundary and mandatory-matrix audit

- [x] Build a requirement-to-test matrix for prompt sections 23 and 24 in this
      plan's progress log. Add any missing direct tests, including changed
      initial-state fingerprints, malformed UTF-8/source/JSON/trace/snapshot,
      canonical ordering, exact CLI exit classes, and every secret sentinel
      sink.
- [x] Add unknown-field tests at each object boundary and non-`1` tests for the
      versioned event, state/output-state, capability, fixture, trace, snapshot,
      and IR envelopes; durable resolutions are exact unversioned objects bound
      by the pending request hash.
- [x] Confirm run failures preserve a valid partial hash chain and atomically
      avoid publishing final state; confirm replay never constructs a driver.
- [x] Run `cargo test --workspace --all-features` and clippy to green.

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
- 2026-08-06: Closed additional expression and state gaps: match patterns now
  verify their enum qualifier, reject arms after a wildcard, and directly
  cover `Option`/`Result` payload binding; provenance and collection search
  enforce their supported/equatable domains; state updates are handler-only
  and may update each declared mutable state field at most once.
- 2026-08-06: Made idempotency metadata match its specified type contract.
  The checker rejects opaque/non-serializable key types, while the VM retains
  text keys verbatim and canonicalizes other serializable JSON keys. A focused
  runtime test proves an integer key reaches the write boundary.
- 2026-08-06: Duplicate tool metadata now fails in the parser rather than
  silently overwriting the earlier entry. Affine diagnostics and move joins
  now cover calls, match arms, nested containers, records, and enum payloads.
- 2026-08-06: General VM coverage exposed and fixed accepted-source/runtime
  mismatches. Pure helpers invoked from policy metadata now execute `match`,
  `require`, and result unwrapping; proposal argument projection decodes the
  declared tool types instead of losing enum/record identity in generic JSON.
- 2026-08-06: Runtime `Result<T,E>` now preserves a typed `E` payload rather
  than hard-coding errors to strings, while postfix `?` still requires the
  statically declared `Error` channel and converts it to a controlled machine
  failure. State defaults can use recorded event metadata but cannot read a
  partially initialized `self`.
- 2026-08-06: Named and positional arguments are normalized against declared
  parameters in handler calls, pure metadata calls, prompt data, tool
  requests, and capability requests. Reversed named calls/capabilities and a
  positional write call now execute with the same mapping proven statically.
- 2026-08-06: Added a runtime sweep for records, lists, `Option`, generic
  `Result`, enums, `if`, `match`, pure calls, built-ins, unary/binary
  operators, transactional state update, division by zero, and overflow.
- 2026-08-06: Closed the remaining boundary and scope mismatches. Candidate
  and privileged wrappers are rejected transitively at external/persistent
  schemas; `Option`/`Result` use unambiguous tagged JSON; agent arguments and
  handlers have exact shapes; nested lexical shadowing lowers to unique VM
  locals; and pure metadata executes scoped `if`/`match` blocks.
- 2026-08-06: Tightened policy and authority totality. A policy now has exactly
  one final `otherwise`, its optional state parameter must match the
  authorizing agent, and permit consumption revalidates the proposal seal so
  post-authorization host mutation cannot retain authority.

## Mandatory conformance matrix

All section 23 cases are asserted by
`mandatory_compile_fail_fixtures_have_stable_codes_and_relevant_spans`; each
row names the source fixture whose diagnostic code and relevant span are
checked. Safe direct-allow and human-approval paths are compile-pass fixtures.

| Requirement | Direct evidence |
| --- | --- |
| 23.1 candidate used before validation | `candidate_used_without_validation.aster` |
| 23.2 candidate passed to write | `candidate_passed_to_write.aster` |
| 23.3 write tool observed | `write_tool_observed.aster` |
| 23.4 read tool proposed | `read_tool_proposed.aster` |
| 23.5 direct tool call | `direct_tool_call.aster` |
| 23.6 commit without permit | `commit_without_permit.aster` |
| 23.7 permit/action mismatch | `permit_action_mismatch.aster` |
| 23.8 permit reused | `permit_reused.aster` |
| 23.9 proposal reused | `proposal_reused.aster` |
| 23.10 effect in policy | `effect_in_policy.aster` |
| 23.11 non-total policy | `non_total_policy.aster`; `policy_otherwise_rule_is_unique_and_final` |
| 23.12 missing capability | `missing_capability.aster` |
| 23.13 dynamic prompt instruction | `dynamic_prompt_instruction.aster` |
| 23.14 secret to model | `secret_to_model.aster` |
| 23.15 secret in state | `secret_in_state.aster` |
| 23.16 direct/mutual recursion | `direct_recursion.aster`; `mutual_recursion.aster` |
| 23.17 unknown/duplicate budget | `unknown_budget.aster`; `duplicate_budget.aster` |
| 23.18 write without idempotency | `write_without_idempotency.aster` |
| safe direct allow and human approval | `direct_allow.aster`; `examples/meeting-scheduler/main.aster` |

## Mandatory runtime/replay matrix

| Requirement | Direct evidence |
| --- | --- |
| 24.1 meeting checks | `bundled_meeting_scheduler_is_a_compile_pass_program`; CLI `check` test |
| 24.2 formatter idempotence/comments | syntax `canonical_format_is_idempotent_and_preserves_normalized_ast`; `comments_survive_and_prompt_instruction_contents_are_byte_identical` |
| 24.3 fixture record succeeds | `meeting_record_and_driver_free_replay_have_identical_state` |
| 24.4 expected driver counts | same record/replay test asserts 1 model, 2 reads, 1 approval, 1 write |
| 24.5 direct allow has no approval | `direct_allow_record_run_never_invokes_approval_driver` |
| 24.6 reconciled final event | meeting record/replay test asserts `last_event` |
| 24.7 byte-identical replay state | CLI `public_commands_check_format_ast_record_and_replay`; final `cmp` demonstration |
| 24.8 replay has no driver | `replay_run` has no driver parameter; `meeting_record_and_driver_free_replay_have_identical_state` |
| 24.9 ordinary tamper | `canonical_json_and_trace_chain_are_deterministic_and_tamper_evident` |
| 24.10 maliciously resealed result | `maliciously_rehashed_result_still_fails_semantic_replay` |
| 24.11 changed source/program | `replay_rejects_modified_program_and_reordered_effect_requests` |
| 24.12 changed input/state | `replay_rejects_changed_input_before_effects` |
| 24.13 reordered effects | `replay_rejects_modified_program_and_reordered_effect_requests` |
| 24.14 proposal hash coverage | `proposal_hash_binds_every_authority_relevant_field`, including schema version |
| 24.15 permit A versus proposal B | `permit_is_bound_expiring_and_single_use` |
| 24.16 dynamic double commit | same low-level authority test |
| 24.17 expired intent/permit before write | `expired_intent_rejects_before_write_driver_invocation`; low-level authority test |
| 24.18 missing runtime capability | `missing_exact_runtime_capability_is_rejected_at_admission` |
| 24.19 model-call budget | `exhausted_model_budget_rejects_before_driver_invocation` |
| 24.20 write budget | `exhausted_write_budget_rejects_before_driver_invocation` |
| 24.21 usage above maximum | `usage_overflow_failure_is_hash_chained_into_partial_trace`; fixture maximum test |
| 24.22 model schema mismatch | `model_response_is_decoded_against_the_prompt_schema` |
| 24.23 tool schema mismatch | `tool_response_is_decoded_against_the_declared_result_schema` |
| 24.24 snapshot round trip | `machine_yields_resumes_and_round_trips_pending_snapshot` |
| 24.25 resume equivalence | CLI `assert_resume` in `public_commands_check_format_ast_record_and_replay` |
| 24.26 failed state transaction | `failed_handler_keeps_state_update_unpublished` |
| 24.27 secret sentinel sinks | `opaque_secret_sentinel_cannot_escape_through_generic_serialization_or_debug`; `secret_is_rejected_before_snapshot_serialization`; static prompt/state boundary fixtures |
| 24.28 malformed inputs never panic | CLI malformed JSON/trace/snapshot and non-UTF-8 source tests |
| 24.29 deterministic diagnostic/trace JSON | diagnostics stable-order test; canonical JSON/trace test |
| 24.30 CLI exit classes | `exit_codes_distinguish_source_runtime_and_replay_failures` |

## Discoveries and deviations

- 2026-08-06: The previous completed plan and green suite prove the governed
  meeting path, replay, budgets, permits, snapshots, and required unsafe
  fixtures, but not full static-language closure.
- 2026-08-06: The initial `Type::Unknown`, inexact-call/record, match-binding,
  affine-join, and optional-tool-metadata gaps were baseline discoveries. Each
  now has a focused regression and no longer remains an accepted limitation.
- 2026-08-06: Durable effect resolutions are deliberately unversioned exact
  objects bound by the pending request hash; event, state, grant, fixture,
  trace, snapshot, IR, and final-state envelopes carry schema version 1.

## Commands and results

- `git status -sb`: clean `main`, tracking `origin/main`.
- `cargo test --workspace --all-features -- --list`: 63 tests enumerated;
  existing coverage is concentrated on the vertical path and mandatory named
  fixtures.
- Source audit of `checker.rs`, `expression.rs`, `model.rs`, and `lowering.rs`:
  exposed the static-semantic gaps recorded above.
- `cargo test -p aster-semantics --test conformance`: 45 passed after the
  declaration, expression, affine, capability, secret-placement, pattern,
  state-update, provenance, and effect-reference TDD cycles.
- `cargo test --workspace --all-features`: passed with 111 tests after the
  third checkpoint changes, including all record/replay and CLI black-box
  suites.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed after the generic result and argument-normalization runtime changes.
- `git diff --check`: passed after the third checkpoint changes.
- `cargo test --workspace --all-features -q`: passed with 126 tests before the
  final policy/authority audit additions.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed after splitting boundary and admission validation helpers.
- Focused post-audit suites: semantics conformance 57/57, runtime governance
  9/9, and runtime machine 19/19 passed.
- `cargo test --workspace --all-features -q`: passed after the final audit with
  128 tests and zero failures.
- README workflow demonstration in `/tmp/tmp.8TOQKD05ve`: `check`, record,
  replay, and `cmp` passed; the trace had 34 entries; trace, snapshots, and both
  output states were mode `0600`; `.aster/` was ignored; no secret sentinel was
  present; and representative JSON compile-fail returned exit 1 with
  `ASTER-TYPE-2001` at the candidate projection span.

## Known limitations

- ASTER 0.1 is not complete while any milestone in this plan remains open.
