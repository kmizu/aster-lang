# ASTER v0.2 Host Protocol Execution Plan

## Goal

Release ASTER v0.2.0 with a versioned, bounded, bidirectional stdio host
protocol. The host may preview and execute effects, while ASTER exclusively
owns admission, budget reservation and settlement, snapshots, trace evidence,
permits, reconciliation, crash recovery, and driver-free replay.

The approved design is
[`docs/superpowers/specs/2026-08-06-aster-v0.2-host-protocol-design.md`](../../superpowers/specs/2026-08-06-aster-v0.2-host-protocol-design.md).
The step-by-step implementation plan is
[`docs/superpowers/plans/2026-08-06-aster-v0.2-host-protocol.md`](../../superpowers/plans/2026-08-06-aster-v0.2-host-protocol.md).

## Baseline and constraints

- Design baseline: commit `bb2b09d` (`Design ASTER v0.2 host protocol`).
- Implementation-plan commit: `888457d`.
- Implementation branch: `agent/v0.2-host-protocol`.
- Language syntax remains ASTER 0.1. Distribution/runtime version becomes
  0.2.0 and host protocol schema begins at 1.
- Preserve all semantic, architectural, determinism, security, replay, and
  data-handling invariants in repository `AGENTS.md`.
- Every behavior change uses RED -> GREEN -> focused verification.
- The final gate is `./scripts/check.sh`, followed by real Codex-host use,
  public release workflow verification, and an external artifact audit.

## Milestones

- [x] Extract a pure, resumable runtime `RecordSession`.
- [x] Define canonical host frames and grant binding.
- [x] Implement the transport-independent `HostSession` state machine.
- [x] Add bounded JSONL CLI transport, persistence, and crash resume.
- [x] Register stable diagnostics and prove payload redaction.
- [x] Add the governed-note self-use example and end-to-end host proof.
- [x] Make the protocol and its trust boundary normative in documentation.
- [x] Bump v0.2.0 and generalize four-platform release automation and site.
- [ ] Pass full validation, merge, tag, publish, and externally audit v0.2.0.

## Progress

- 2026-08-06: Approved and committed the written design and detailed
  implementation plan.
- 2026-08-06: Verified the dedicated branch is clean and ran
  `cargo test -p aster-runtime --all-features`: 37 integration tests passed
  (9 governance, 19 machine, 9 record/replay), with no failures.
- 2026-08-06: Added `RecordSession`, explicit admission/resolution phases,
  sealed admitted-effect snapshots, checkpoint restoration, and the fixture
  driver adapter. The final Task 1 gate passed 40 integration tests
  (9 governance, 19 machine, 12 record/replay), runtime clippy with warnings
  denied, and `git diff --check`.
- 2026-08-06: Added exact host envelopes and payloads, strict raw-payload
  decoding, duplicate/unknown usage rejection, all six safe protocol error
  classes, and an execution-grant hash bound to protocol/run/request/trace/
  snapshot/maximums. Focused host tests, full runtime tests, and strict runtime
  clippy passed before the Task 2 commit.
- 2026-08-06: Added the pure `HostSession` handshake and exact outstanding
  reply phases, redacted terminal failures, EOF handling, resume-to-identical-
  grant behavior, and accessors for trace/snapshots/outcome. A fixture-backed
  host test drove model/read/approval/write/read and then reproduced the state
  through `replay_run` with no host interaction. The Task 3 gate passed 55
  integration tests (9 governance, 15 host, 19 machine, 12 record/replay) and
  strict runtime clippy.
- 2026-08-06: Added `aster host` and `aster host-resume`, a bounded JSONL
  transport, shared start-input loading, and atomic host evidence persistence.
  Black-box tests prove protocol-only stdout, malformed/unknown/version/UTF-8/
  EOF terminal failures, and crash-after-grant resume with identical request,
  maximums, snapshot hash, and grant hash. Unit tests prove exactly 1 MiB is
  accepted and one extra byte is rejected before unbounded allocation.
- 2026-08-06: Registered `ASTER-HOST-11001` through `11006` with distinct
  meanings and remediation. Runtime and CLI sentinel tests verify hostile
  private/secret frame values do not appear in typed errors, failed frames,
  stderr, traces, snapshots, or output-state artifacts.
- 2026-08-06: Added the synthetic `governed-note` program and used a Rust host
  harness as a coding-agent analogue. It drove model/read/approval/write/read,
  proved preview caused no filesystem mutation, performed the temporary-file
  write only after the matching durable grant, reconciled the result, and
  reproduced the final state byte-for-byte with driver-free replay. Full CLI
  tests and strict CLI clippy passed.
- 2026-08-06: Added the normative ASTER 0.2 host protocol with exact envelopes,
  payload fields, MUST/MUST NOT sequencing, usage settlement, durability,
  resume, terminal frames, diagnostics, disclosure, compatibility, replay,
  and malicious-host limits. Updated language, architecture, runtime, security,
  core-belief, example, and README documentation. The docs checker now rejects
  a missing required protocol term; its regression test, the repository docs
  check, architecture check, and governed-note source check all passed.
- 2026-08-06: Bumped every workspace package and lockfile entry to `0.2.0`,
  added curated v0.2.0 release notes, and updated README and Pages copy with
  the host boundary, self-use evidence, four native archives, and checksums.
  Release archive names, the Windows member check, and release-notes path now
  derive from the workspace version instead of matrix literals. The release
  checker derives the same version and rejects a synthetic Cargo 0.2.1 versus
  notes/site 0.2.0 drift. Release/site test suites, release/site repository
  checks, YAML parsing, `aster 0.2.0`, and governed-note checking passed.
- 2026-08-06: The first final `./scripts/check.sh` run exposed one policy
  violation in a `#[cfg(test)]` block under production `src`: a success-only
  `.expect()` in the host transport size test. The production scanner
  intentionally scans source files without interpreting cfg attributes. The
  assertion now checks `is_ok()` without weakening the scanner; its checker
  regression, focused boundary test, and a fresh full gate passed.
- 2026-08-06: Codex acted directly as the external host in a mode-700 temporary
  workspace. The live sequence was model/read/approval/write/read. The note
  hash remained `9160d4be...` (`before\n`) at write preview, changed only after
  the matching grant to `0af71c02...` (`ship v0.2\n`), then reconciled. Five
  snapshots and 34 trace entries were produced. Driver-free CLI replay opened
  no host session and produced byte-identical record/replay state files with
  SHA-256 `0d9c39cf...`.
- 2026-08-06: Final inline review found no critical defects and one important
  direct-coverage gap: restore-to-identical-grant was tested with a model
  effect, while the approved design explicitly named an uncertain write. The
  new `host_session_restore_reemits_uncertain_write_without_readmission` test
  drives through approval, stops at the write grant, restores its sealed
  continuation, and proves the first post-handshake frame is the identical
  write grant rather than a second preview/admission.
- 2026-08-06: Pushed `agent/v0.2-host-protocol` at validation commit
  `5261cd1` and opened ready PR #3 against `main`. Both the push CI run
  `31105595264` and pull-request CI run `31105610743` completed successfully;
  each ran the repository `./scripts/check.sh` job on the exact head commit.
- 2026-08-06: PR #3 remained mergeable with no reviews or unresolved review
  threads and was merged with merge commit
  `cc3f8b583c6cb2e8a558780c6c1d886dd4b51ea7`. Main CI run `31105858348`
  and tag CI run `31105911672` both completed successfully.
- 2026-08-06: Created annotated tag `v0.2.0`. Its immutable tag object is
  `9a55bd448ed40bf8968d64a1d24c5941b630b7cc`, and the GitHub tag API proves
  that it peels to merge commit `cc3f8b583c6cb2e8a558780c6c1d886dd4b51ea7`.
  Release run `31105912137` passed validation, all four native full test and
  binary-smoke jobs, exact-member/checksum publication checks, and publish.
  The resulting GitHub Release is non-draft and non-prerelease.
- 2026-08-06: Downloaded the five public release assets into a fresh mode-700
  directory. `sha256sum --check SHA256SUMS` passed all four archives. Every
  tar contains only `LICENSE`, `README.md`, and `aster`; the Windows zip
  contains only `LICENSE`, `README.md`, and `aster.exe`. The downloaded Linux
  binary reports `aster 0.2.0`, checks `examples/governed-note/main.aster`, and
  is an x86-64 static PIE (`ELF` type `DYN`). The release, every asset, and the
  Pages root returned HTTP 200. Pages still served the prior v0.1.0 content:
  the first deployment stalled until cancellation, and same-commit recovery
  runs were cancelled because Pages deployment build versions must be unique.
  A post-release evidence commit provides a new immutable deployment version;
  Pages content and link verification remain required before plan completion.

## Surprises & Discoveries

- Holding `AdmittedEffect` directly in the progress and internal phase enums
  made each enum approximately the size of a full `MachineSnapshot`. Boxing
  only that variant retained value semantics at the API boundary while keeping
  the session state compact and satisfying strict clippy.
- Self-review exposed that a mismatched resolution could be traced before the
  machine rejected its request hash. A red regression now proves hash
  substitution is rejected at the session boundary before `effect_resolved`
  or its payload can enter the trace.
- Deserializing an envelope payload through `serde_json::Value` erased
  duplicate usage keys before the strict payload type could inspect them.
  Keeping the payload as `RawValue` until kind-specific decoding preserves the
  original map structure without retaining it in any public error.
- A protocol error must both be returned to the transport and leave durable
  terminal evidence. `HostSession` therefore closes the active
  `RecordSession` into a `RecordFailure`, exposes the redacted `failed` frame,
  and retains the resulting trace/snapshots after returning the typed error.
- The CLI must persist every session snapshot and the hash-chained trace before
  writing `execute_grant`. A process killed immediately after the host reads
  the grant can therefore restore the exact admitted continuation without a
  second admission.
- `read` and `write` are reserved source words and cannot currently be used as
  qualified tool-name segments. The example therefore names the tool methods
  `Workspace.fetch` and `Workspace.store`; their governed effect kinds remain
  `read` and `write`.
- The protocol's durable point has two layers: `RecordSession` creates and
  hashes the admitted snapshot, then the CLI atomically writes every snapshot
  and the current trace before putting `execute_grant` on stdout. Documenting
  only the in-memory transition would overstate crash safety.
- The v0.1 release workflow encoded a version three times per matrix row and
  again in publish checks. Removing the matrix `bundle`/`asset` fields and
  deriving all four names from Cargo leaves the site and notes as deliberate
  public-copy assertions while eliminating workflow drift.
- A `#[cfg(test)]` module inside a production `src` file is still in scope for
  the repository's textual panic-policy checker. Tests in that location must
  avoid forbidden APIs even though test targets elsewhere may use them.

## Decision Log

- Use the existing dedicated branch rather than create a nested worktree. The
  repository checkout is already isolated from the user's normal branch, and
  the approved plan names this branch as the execution line.
- Execute inline because the current session policy prohibits subagent
  delegation. Retain task-level commits and verification checkpoints from the
  approved plan.
- Represent `AwaitingResolution` as `Box<AdmittedEffect>` and borrow
  `EffectResolution` in `resolve`. This avoids copying or inflating the sealed
  snapshot while preserving the phase and ownership contract.
- Enable only serde_json's existing `raw_value` feature. This adds no new
  library or I/O capability and is required to reject duplicate nested usage
  dimensions before a lossy JSON object representation can collapse them.
- Classify the private usage-deserializer marker only when it begins an error,
  so an attacker-controlled unknown field containing the marker cannot alter
  the public diagnostic class.
- Keep the six host-boundary diagnostic classes distinct from an existing
  typed runtime failure. `HostProtocolError::RuntimeFailure` maps only to
  `ASTER-RUNTIME-9001`; it is used when session setup or deterministic VM/
  trace internals fail rather than mislabeling those failures as malformed
  host frames.
- A resumed `HostSession` always performs a fresh hello exchange, then emits
  the identical grant reconstructed from the sealed snapshot. It never emits
  a preview or accepts a second admission for that pending effect.
- Standard output ownership is enforced structurally: host commands write only
  through `HostTransport`; all `CliError` reporting remains on standard error.
  Each successful write is one JSON object, one newline, and a flush.
- Host diagnostics intentionally contain only their stable class and fixed
  redacted summary. Raw serde errors and complete input frames are discarded at
  the decoding boundary before CLI error construction or trace failure
  evidence.
- Keep ASTER 0.1 syntax unchanged for v0.2: use non-keyword tool identities
  instead of expanding qualified-name grammar solely for the self-use example.
  The exact synthetic note allowlist provides an auditable non-empty bounded
  validator without adding a new text-length built-in.
- Treat the wire contract as a separate versioned normative specification.
  The ASTER 0.1 language spec links to it but does not claim a source-language
  version change.
- Keep recovery dispatch explicit but without a stale default tag. Exact-tag
  checkout and Cargo/tag equality still prevent a recovery run from publishing
  a branch or mismatched version.

## Requirement audit

| Approved requirement | Direct evidence |
| --- | --- |
| One pure execution path | `crates/aster-runtime/src/run.rs` owns `RecordSession`; fixture `record_run_evidenced` and `HostSession` both delegate to it. Record/replay tests remain green. |
| Exact envelopes, fields, and sequencing | `crates/aster-runtime/src/host.rs`; `hello_has_the_exact_envelope`, strict/duplicate/unknown-field tests, and out-of-sequence/cross-session tests in `host_protocol.rs`. |
| Preview is not authority; budget and durable continuation precede grant | `RecordSession::admit`, `drive_host`, `host_session_handshake_precedes_two_phase_effect_execution`, CLI crash/resume, and governed-note preview-before-write assertions. |
| Request/grant binding and bounded usage | `ExecutionGrant::for_admitted`, `HostEffectResolution::validate_against`, substitution, duplicate dimension, overflow, and exhaustion tests. |
| Existing write governance remains exclusive | The VM still creates write requests only after intent/proposal/policy/permit checks; governance/machine suites and governed-note reconciliation pass unchanged. |
| Crash/resume never readmits an uncertain effect | `host_session_restore_reemits_the_same_grant_without_readmission`, `host_resume_reemits_the_durable_execution_grant`, and the write-specific `host_session_restore_reemits_uncertain_write_without_readmission`. |
| Bounded strict transport and stable failures | `crates/aster-cli/src/host_io.rs`, exact 1 MiB/one-byte-over tests, malformed JSON/UTF-8/version/unknown-field/EOF black-box cases, and `ASTER-HOST-11001` through `11006` registry tests. |
| No payload or secret disclosure through failures | Runtime and CLI sentinel tests cover typed errors, `failed`, stderr, trace, snapshots, and output state; existing secret serialization/snapshot tests remain green. |
| Replay invokes no host or driver | `replay_run` has no driver/host parameter; runtime all-effects replay test, CLI governed-note test, and the manual Codex-host replay all produce identical state. |
| Fixture compatibility | Meeting fixture record/replay and the complete runtime/CLI regression suites pass through the shared session adapter. |
| Real host self-use | `examples/governed-note`, its black-box coding-host test, and the manual mode-700 PTY session prove model/read/approval/write/read, write-after-grant, reconciliation, and byte-identical replay. |
| Normative and explanatory documentation | `docs/spec/aster-host-protocol-0.2.md` plus language, architecture, runtime, security, diagnostics, README, and example docs; the docs checker rejects a missing required protocol term. |
| Versioned four-platform release | Workspace/lockfile are 0.2.0; release workflow derives names/notes from Cargo, runs four native suites/smokes, validates exact members/checksums, and gates publish on all builds. Release/site drift tests and YAML parsing pass. |

## Outcomes & Retrospective

The implementation and release candidate satisfy the approved design and
repository invariants. After the write-resume review fix and canonical
formatting, the fresh moved-plan `./scripts/check.sh` gate passed 156 tests
across all crates, strict clippy/formatting, architecture, documentation,
production panic policy, site, and release contracts. `./scripts/check-docs.sh`
and `git diff --check` also passed on the same final tree.

The host boundary deliberately does not sandbox its host. A malicious host can
act before a grant, falsify provider behavior, or under-report usage using
authority it already possesses. Production deployment still requires process
isolation, least privilege, protected trace/snapshot storage, provider-specific
authenticity controls, and idempotent recovery.

The public PR, merge, annotated tag, release workflow, and downloaded-asset
evidence are recorded above. The immutable release is healthy; the remaining
completion item is a successful Pages deployment from a new post-release
commit followed by external v0.2.0 headline and link verification.
