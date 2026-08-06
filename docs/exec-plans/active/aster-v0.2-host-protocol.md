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
- [ ] Implement the transport-independent `HostSession` state machine.
- [ ] Add bounded JSONL CLI transport, persistence, and crash resume.
- [ ] Register stable diagnostics and prove payload redaction.
- [ ] Add the governed-note self-use example and end-to-end host proof.
- [ ] Make the protocol and its trust boundary normative in documentation.
- [ ] Bump v0.2.0 and generalize four-platform release automation and site.
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

## Outcomes & Retrospective

Not complete. This section will record released behavior, exact validation and
artifact evidence, limitations, and residual malicious-host risk after the
public v0.2.0 audit.
