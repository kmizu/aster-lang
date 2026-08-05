# AGENTS.md — ASTER Repository Operating Contract

## Authority and scope

- This file applies to the entire repository. A deeper `AGENTS.md` may add stricter local rules but must not weaken these rules.
- Direct user, system, or harness instructions take precedence. When instructions conflict, call out the conflict in the final report.
- Treat repository-local, version-controlled artifacts as the source of truth. Do not rely on undocumented chat context or assumptions.
- Preserve unrelated user changes. Never use destructive Git commands to make the worktree convenient.

## Mission

ASTER is a deterministic, auditable language and runtime for governed AI agents.

Its core boundary is:

> Model output is a `Candidate`; external action is a `Proposal`; authority is a `Permit`; reality is checked by `Reconciliation`.

Optimize for semantic correctness, determinism, replayability, security, and legibility to future agents—not for cleverness or line-count reduction.

## Read before editing

Read the smallest relevant set, beginning with:

1. `README.md`
2. `ARCHITECTURE.md`
3. `docs/spec/aster-0.1.md`
4. `docs/design-docs/core-beliefs.md`
5. `docs/design-docs/runtime-and-replay.md`
6. `docs/design-docs/security-model.md`
7. Any active plan in `docs/exec-plans/active/`
8. Any nearer `AGENTS.md`

For work that changes semantics, architecture, public syntax, persistence, security boundaries, or multiple crates, create or update an execution plan before implementation. Keep progress, decisions, discoveries, and validation results in that plan.

## Non-negotiable semantic invariants

Do not weaken these without an explicit user instruction, a design record, specification changes, and regression tests.

1. `Candidate<T>` is opaque and is never implicitly or explicitly coercible to `T`.
2. A model can produce data or a typed action candidate; it never receives ambient authority and never executes tools directly.
3. Read tools execute only through `observe`.
4. Write tools execute only through `intent -> propose -> authorize -> commit`.
5. `Permit<A>` is affine, single-use, expiring, and cryptographically bound to one immutable proposal.
6. Policies and validators are pure, deterministic, total over their declared inputs, and unable to call models, tools, clocks, randomness, or mutate state.
7. Capabilities are runtime-issued values. Source code may require or narrow them but may not mint or broaden them.
8. Budget is checked or reserved before every external effect and settled deterministically afterward.
9. Replay never invokes an external driver. Any request mismatch is a hard replay-divergence error.
10. Prompt instructions are static source literals. Untrusted or remembered data may enter only the structured data channel.
11. `Secret<T>` is opaque and may not enter prompts, ordinary logs, traces, diagnostics, persistent agent state, or string conversion.
12. External data is decoded and validated at boundaries before semantic use.
13. Effectful execution lowers to a serializable explicit machine/IR. Do not hide effects inside recursive AST evaluation.
14. Runtime behavior must not depend on hash-map iteration order, wall-clock reads outside recorded effects, ambient environment variables, or nondeterministic serialization.
15. No arbitrary `eval`, dynamic source execution, shell execution, network access, or native plugin loading exists in ASTER 0.1.

## Architectural boundaries

The dependency direction is:

`aster-diagnostics <- aster-syntax <- aster-semantics <- aster-ir <- aster-runtime <- aster-cli`

- Lower layers must not depend on higher layers.
- `aster-syntax` owns lexing, parsing, spans, syntax trees, and canonical formatting only.
- `aster-semantics` owns names, types, effects, capabilities, policy purity, taint rules, and affine-use analysis.
- `aster-ir` owns the typed, serializable, explicit control-flow/effect representation.
- `aster-runtime` owns the deterministic VM, budgets, proposals, permits, traces, snapshots, replay, and drivers.
- `aster-cli` is orchestration only. It must not duplicate or bypass semantic or runtime rules.
- External effects cross exactly one interface: the runtime effect driver. Parser, checker, IR, policies, validators, and the VM core must not perform I/O.
- Cross-layer shortcuts are defects even when they make one test pass.

Run `./scripts/check-architecture.sh` after changing crate dependencies or module boundaries.

## Change discipline

Before changing code:

- Inspect `git status`, the relevant modules, their callers, tests, and current documentation.
- Reproduce a bug before fixing it when a reproduction is possible.
- State the invariant or acceptance criterion the change serves.
- Prefer the smallest coherent change. Do not mix feature work with unrelated cleanup.

While changing code:

- Keep the repository buildable at meaningful checkpoints.
- Add or update tests with the implementation, not afterward.
- Do not leave production `TODO`, `unimplemented!`, placeholder branches, fake success paths, or silent fallbacks.
- Do not suppress diagnostics or tests to make a change pass.
- Do not duplicate logic to avoid understanding an existing abstraction.
- Do not add compatibility aliases or multiple syntaxes unless the specification requires them.
- Do not guess external data shapes; parse typed structures at the boundary.
- Avoid broad refactors unless they are necessary for the requested behavior and documented in the plan.

After changing code:

- Re-read the diff for semantic drift, accidental API changes, secret leakage, nondeterminism, and unrelated edits.
- Update the specification, architecture, ADRs, examples, and diagnostics reference when their truth changed.
- Run the required validation commands after the final edit.

## Rust rules

- Use stable Rust and the repository toolchain file.
- `unsafe` is forbidden unless the user explicitly authorizes it and the reason is documented in an ADR. Keep `#![forbid(unsafe_code)]` enabled.
- Library code must not use `unwrap`, `expect`, `panic!`, `todo!`, or `unimplemented!` on reachable user-controlled paths. Tests may use them when failure is the assertion.
- Use typed errors with stable diagnostic codes. Preserve source spans and causal context.
- Validate at serialization, CLI, fixture, and adapter boundaries. Never deserialize directly into trusted runtime state without validation.
- Use deterministic collections or explicit sorting anywhere order affects diagnostics, hashes, traces, formatting, snapshots, or tests.
- Keep modules cohesive. Split files that become difficult to review; do not create generic “utils” dumping grounds.
- Public items require concise rustdoc that states invariants, ownership/affinity, and failure behavior where relevant.
- Prefer explicit domain types over booleans and strings for modes, risk, sensitivity, effect kinds, decisions, and resource units.

## Language and specification evolution

A syntax or semantic change is incomplete unless the same patch updates all affected layers:

- normative specification;
- parser and syntax tests;
- formatter and idempotence tests;
- name/type/effect/affine checks;
- IR lowering and serialization version, when relevant;
- runtime behavior and replay compatibility, when relevant;
- stable diagnostics and compile-fail fixtures;
- examples and user documentation.

Do not change an existing diagnostic code to mean something else. Add a new code or document a deliberate migration.

Canonical formatting must be idempotent: formatting twice produces byte-identical output.

## Required testing

Run the narrowest relevant tests during development. Before completion, run:

```bash
./scripts/check.sh
```

That script must cover at least:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
./scripts/check-architecture.sh
./scripts/check-docs.sh
```

Additional obligations:

- Parser/formatter change: add parse cases, malformed-input cases, span assertions, and formatter idempotence.
- Type/effect change: add compile-pass and compile-fail golden tests with stable diagnostic codes.
- Runtime/effect change: add record/replay equivalence, no-driver-on-replay, tamper/divergence, and budget tests.
- Proposal/permit change: test proposal binding, expiry, double-use, forgery rejection, and immutable argument hashing.
- Secret/taint change: test every serialization and diagnostic boundary for leakage.
- CLI change: add black-box exit-code and JSON-output tests.
- Serialization change: add round-trip and compatibility/version rejection tests.

Never claim a check passed unless you ran it successfully. If a required check cannot run, report the exact command, error, and residual risk.

## Dependencies

- Prefer the standard library and existing workspace dependencies.
- Add a dependency only when it materially reduces risk or complexity and is maintained, license-compatible, and narrowly scoped.
- Record non-obvious dependency decisions in the active execution plan or an ADR.
- Do not add Git dependencies, unpinned tools, overlapping libraries for the same job, or dependencies used only to avoid a small clear implementation.
- Keep default features minimal. Do not enable network, TLS, process execution, or dynamic loading transitively for the ASTER 0.1 runtime.

## Security and data handling

- Tests and examples use synthetic data only.
- Never place credentials, tokens, private keys, real personal data, or live endpoints in source, fixtures, snapshots, traces, or documentation.
- Fixture and trace files may contain private-classified synthetic values; create them with restrictive permissions where supported.
- Console output and diagnostics use redacted summaries. Full replay payloads belong only in explicit trace artifacts.
- A value classified `Secret` must remain an opaque handle through the runtime.

## Documentation as the system of record

`AGENTS.md` is a map and contract, not the encyclopedia. Put durable detail in `docs/`.

- Normative language behavior belongs in `docs/spec/`.
- Architectural rationale belongs in `docs/design-docs/`.
- Major decisions belong in `docs/adr/`.
- Multi-step work belongs in `docs/exec-plans/`.
- Generated references must be clearly marked and reproducible.
- When code and docs disagree, fix both in the same change; do not merely note the drift.

## Git and completion hygiene

- Follow harness-specific Git instructions.
- Never rewrite history, amend user commits, force-push, or discard unrelated changes.
- Do not commit build artifacts, local traces, snapshots, secrets, editor state, or temporary files.
- Finish with a reviewed diff and a clean worktree when the harness requires a commit.
- The final report must contain: implemented behavior, important design decisions, tests actually run, known limitations, and any follow-up risk. Keep claims evidence-based.
