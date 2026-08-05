# ASTER

ASTER is an experimental, deterministic language and runtime for governed AI
agents. Its central boundary is:

> Model output is a `Candidate`; external action is a `Proposal`; authority is a
> `Permit`; reality is checked by `Reconciliation`.

The model is a typed inference oracle, never an executor or security principal.
ASTER source declares prompts, validators, tools, policies, capabilities,
budgets, durable agent state, and event handlers. A deterministic runtime owns
all effects and records enough evidence to replay them without contacting a
driver.

## Status and scope

ASTER 0.1 is under active bootstrap and is not production-ready. The 0.1 scope
is one-file programs, non-generic user types, finite pure computation, explicit
model/read/approval/write effects, fixture-backed drivers, transactional state,
hash-chained traces, snapshots, resume, and semantic replay.

It intentionally excludes live provider integrations, arbitrary network or
shell access, FFI, packages/imports, concurrency, loops, recursion,
self-modification, distributed execution, and production key management.

## Prerequisites and repository checks

Install stable Rust 1.96 or allow `rustup` to install the pinned toolchain.

```bash
cargo build --workspace --all-features
./scripts/check.sh
```

The workspace layers are:

```text
aster-diagnostics <- aster-syntax <- aster-semantics <- aster-ir <- aster-runtime <- aster-cli
```

See [ARCHITECTURE.md](ARCHITECTURE.md), the
[normative specification](docs/spec/aster-0.1.md), and the
[active bootstrap plan](docs/exec-plans/active/bootstrap-aster-0.1.md).

## Meeting scheduler workflow

The bundled program takes an untrusted meeting request through inference,
validation, a free-slot observation, intent construction, proposal,
authorization (including fixture-backed human approval), commit, lookup, and
reconciliation before publishing state.

The final CLI uses these exact commands:

```bash
cargo run -p aster-cli --bin aster -- check examples/meeting-scheduler/main.aster

cargo run -p aster-cli --bin aster -- run examples/meeting-scheduler/main.aster \
  --agent Scheduler \
  --event message \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --fixtures examples/meeting-scheduler/fixtures.json \
  --trace .aster/meeting.trace.jsonl \
  --snapshot-dir .aster/snapshots \
  --output-state .aster/record.output-state.json

cargo run -p aster-cli --bin aster -- replay examples/meeting-scheduler/main.aster \
  --trace .aster/meeting.trace.jsonl \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --output-state .aster/replay.output-state.json

cmp .aster/record.output-state.json .aster/replay.output-state.json
```

The command contract is part of the target 0.1 vertical slice tracked by the
active execution plan; it is not claimed complete until that plan moves to
`docs/exec-plans/completed/` and `./scripts/check.sh` passes.

## Trace data warning

Fixture traces contain synthetic values in this repository, but the format may
hold private non-secret payloads. Treat trace and snapshot files as sensitive
artifacts. `Secret<T>` material is forbidden from them entirely.
