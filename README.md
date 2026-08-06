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

ASTER 0.1 is the implemented source-language version and is not
production-ready. The runtime also implements the version 1 ASTER 0.2 host
protocol for a bounded external host. The language scope is one-file programs,
non-generic user types,
finite, lexically scoped pure computation, explicit
model/read/approval/write effects, fixture-backed drivers, transactional state,
hash-chained traces, snapshots, resume, and semantic replay.

It intentionally excludes live provider integrations, arbitrary network or
shell access, FFI, packages/imports, concurrency, loops, recursion,
self-modification, distributed execution, and production key management.

## ASTER v0.2.0

The current experimental release is available from the
[ASTER project site](https://kmizu.github.io/aster-lang/) and the
[GitHub Release](https://github.com/kmizu/aster-lang/releases/tag/v0.2.0).
It adds the governed external-host protocol and provides these native command-
line archives:

- [Linux x86_64 musl](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz)
- [macOS Apple Silicon](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-aarch64-apple-darwin.tar.gz)
- [macOS Intel](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-apple-darwin.tar.gz)
- [Windows x86_64](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-pc-windows-msvc.zip)

See the [v0.2.0 release notes](docs/releases/v0.2.0.md) for the implemented
scope, self-use evidence, trust boundary, and limitations.

Download [`SHA256SUMS`](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/SHA256SUMS)
with the archives and verify them before extraction:

```bash
sha256sum --check SHA256SUMS
```

The release binaries are unsigned. Checksums detect download corruption or
substitution but do not establish publisher identity. To build the same CLI
locally instead, use the pinned Rust toolchain:

```bash
cargo build --release -p aster-cli --bin aster
./target/release/aster --version
```

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
[normative language specification](docs/spec/aster-0.1.md), the
[normative host protocol](docs/spec/aster-host-protocol-0.2.md), and the
[ASTER 0.1 completion plan](docs/exec-plans/completed/complete-aster-0.1.md).

## Governed external host

`aster host` exposes typed effect requests to an external process over bounded
JSON Lines. A preview is not authority: the host may execute only after ASTER
has reserved budget, persisted a sealed continuation, and emitted the matching
execution grant. Standard output is protocol-only; diagnostics use standard
error.

Start the bundled synthetic note program with a bidirectional host attached to
standard input and output:

```bash
cargo run -p aster-cli --bin aster -- host examples/governed-note/main.aster \
  --agent NoteKeeper \
  --event message \
  --input examples/governed-note/event.json \
  --state examples/governed-note/initial-state.json \
  --capabilities examples/governed-note/capabilities.json \
  --trace .aster/note.trace.jsonl \
  --snapshot-dir .aster/note-snapshots \
  --output-state .aster/note.record-state.json
```

After a crash with an admitted pending effect, reconnect the host with:

```bash
cargo run -p aster-cli --bin aster -- host-resume \
  examples/governed-note/main.aster \
  --snapshot .aster/note-snapshots/snapshot-0000.json \
  --trace .aster/note.trace.jsonl \
  --snapshot-dir .aster/note-resume-snapshots \
  --output-state .aster/note.record-state.json
```

The end-to-end coding-host analogue uses only a temporary workspace, performs
the write after its grant, reconciles it, and proves driver-free replay:

```bash
cargo test -p aster-cli --test host_cli codex_style_host -- --nocapture
```

See the [governed-note walkthrough](examples/governed-note/README.md) and the
[host protocol specification](docs/spec/aster-host-protocol-0.2.md).

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

The source stages are deliberately visible: `infer` yields an opaque candidate,
`validate` yields checked data, `observe` performs read-only effects, and a
write crosses `intent -> propose -> authorize -> commit` before a final
observation is reconciled. State changes publish only after the handler returns
successfully. The completed execution plan records the implementation evidence
behind this command contract.

## Trace data warning

Fixture traces contain synthetic values in this repository, but the format may
hold private non-secret payloads. Treat trace and snapshot files as sensitive
artifacts. `Secret<T>` material is forbidden from them entirely.
