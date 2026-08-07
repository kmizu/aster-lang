# ASTER

ASTER is an experimental language and deterministic runtime for AI agents that
need judgment without ambient authority. A model may propose typed data, but it
cannot execute a tool, mint a capability, or turn its output into an ordinary
value. The runtime owns effects, budgets, state publication, audit evidence,
and replay.

> Model output is a `Candidate`; external action is a `Proposal`; authority is a
> `Permit`; reality is checked by `Reconciliation`.

**Experimental reference processor · current release v0.2.0**

[Project site](https://kmizu.github.io/aster-lang/) ·
[Release v0.2.0](https://github.com/kmizu/aster-lang/releases/tag/v0.2.0) ·
[Language specification](docs/spec/aster-0.1.md) ·
[Host protocol](docs/spec/aster-host-protocol-0.2.md)

## Why ASTER

ASTER treats the model as a typed inference oracle, never as an executor or
security principal. The language makes the governance boundary explicit:

| Concern | ASTER boundary |
| --- | --- |
| Model result | `Candidate<T>` is opaque until deterministic validation. |
| Read effect | A declared read tool runs only through `observe`. |
| Write effect | The program must cross `intent -> propose -> authorize -> commit`. |
| Authority | `Permit<A>` is runtime-issued, expiring, proposal-bound, and single-use. |
| Success | A receipt must match a later observation before state publishes. |
| Replay | The VM replays recorded resolutions without a driver parameter. |

## Five-minute deterministic proof

This proof requires Rust 1.96, a repository checkout, and a Bash-compatible
shell. The checked-in inputs and fixture effects are synthetic. Successful
ASTER commands are intentionally quiet; the final three commands print the
evidence.

```bash
cargo build --release -p aster-cli --bin aster

ASTER_DEMO_DIR=$(mktemp -d)

./target/release/aster check examples/meeting-scheduler/main.aster

./target/release/aster run examples/meeting-scheduler/main.aster \
  --agent Scheduler \
  --event message \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --fixtures examples/meeting-scheduler/fixtures.json \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --snapshot-dir "$ASTER_DEMO_DIR/snapshots" \
  --output-state "$ASTER_DEMO_DIR/record-state.json"

./target/release/aster replay examples/meeting-scheduler/main.aster \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --output-state "$ASTER_DEMO_DIR/replay-state.json"

cmp "$ASTER_DEMO_DIR/record-state.json" "$ASTER_DEMO_DIR/replay-state.json" \
  && echo "record and replay states match"
wc -l < "$ASTER_DEMO_DIR/meeting.trace.jsonl"
cat "$ASTER_DEMO_DIR/record-state.json"
```

The reproduced output is:

```text
record and replay states match
34
{"schema_version":1,"state":{"last_event":{"some":{"id":"event-001"}},"profile":{"known_attendees":[]}}}
```

## What the proof establishes

### Fixture-backed record

Record mode consumes the five synthetic fixture effects, reserves and settles
budget, writes snapshots and a 34-entry hash chain, and publishes state only
after reconciliation.

### Driver-free replay

Replay receives no fixture or driver input. It re-steps deterministic semantics
against the recorded trace and rejects request or trace divergence before it can
publish an inconsistent result.

## Authority model

The four governing values have separate owners and cannot be shortcut:

| Value | Meaning and transition |
| --- | --- |
| `Candidate<T>` | Opaque model output; only deterministic `validate` can turn it into checked data. |
| `Proposal<A>` | Immutable desired write, bound to the action, arguments, intent, risk, capability request, and program identity. |
| `Permit<A>` | A runtime-issued, expiring, proposal-bound, affine authority token consumed by one matching commit. |
| `Reconciliation` | A later checked observation proves the receipt matches reality before state publication. |

The full source-stage sequence is visible on purpose:
`infer -> validate -> observe -> intent -> propose -> authorize -> commit -> reconcile`.
Reads cross only `observe`; writes require
`intent -> propose -> authorize -> commit`, and every write receipt must later
be reconciled.

## External host integration

Fixture-backed `aster run` is the deterministic proof above. `aster host`
instead presents bounded JSON Lines effect requests to an external process. Its
protocol sequence is:

```text
effect_preview -> effect_admission -> execute_grant -> effect_resolution
```

An `effect_preview` is not authority. ASTER emits `execute_grant` only after
the host admits the exact request, the runtime reserves budget, and the sealed
continuation plus trace are durable. The returned resolution must still match
the request and settle through the runtime.

This protocol cannot confine a malicious host: a host may misuse authority it
already has, act before a grant, falsify provider behavior, or under-report
usage. Deployers remain responsible for isolation, least privilege, and
provider-specific controls. See the [governed-note walkthrough](examples/governed-note/README.md)
and the [normative host protocol](docs/spec/aster-host-protocol-0.2.md).

## Install a release archive

Download the v0.2.0 archive for your platform:

- [Linux x86_64 musl](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz)
- [macOS Apple Silicon](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-aarch64-apple-darwin.tar.gz)
- [macOS Intel](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-apple-darwin.tar.gz)
- [Windows x86_64](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/aster-v0.2.0-x86_64-pc-windows-msvc.zip)

Download [SHA256SUMS](https://github.com/kmizu/aster-lang/releases/download/v0.2.0/SHA256SUMS)
alongside the archives and verify it before extraction:

```bash
sha256sum --check SHA256SUMS
```

The release binaries are unsigned and the macOS archives are not notarized.
Checksums detect download corruption or substitution, but do not establish
publisher identity. To build from source instead, use the pinned toolchain:

```bash
cargo build --release -p aster-cli --bin aster
./target/release/aster --version
```

## Project scope

ASTER 0.1 is the implemented source-language version. The runtime implements
the version 1 ASTER 0.2 host protocol for a bounded external host. The
implemented vertical slice is one-file programs, non-generic user types,
finite lexically scoped pure computation, explicit model/read/approval/write
effects, fixture-backed drivers, transactional state, hash-chained traces,
snapshots, resume, and semantic replay.

It intentionally excludes live provider integrations, arbitrary network or
shell access, FFI, packages/imports, concurrency, loops, recursion,
self-modification, distributed execution, and production key management.

The workspace dependency direction is:

```text
aster-diagnostics <- aster-syntax <- aster-semantics <- aster-ir <- aster-runtime <- aster-cli
```

Run the repository checks from a pinned Rust toolchain:

```bash
cargo build --workspace --all-features
./scripts/check.sh
```

Fixture traces are synthetic in this repository, but traces and snapshots may
contain private non-secret payloads and must be protected as sensitive
artifacts. `Secret<T>` material is forbidden from prompts, ordinary logs,
diagnostics, traces, snapshots, and persistent agent state.

## Documentation map

- [Language specification](docs/spec/aster-0.1.md)
- [Host protocol specification](docs/spec/aster-host-protocol-0.2.md)
- [Architecture](ARCHITECTURE.md)
- [Core beliefs](docs/design-docs/core-beliefs.md)
- [Runtime and replay](docs/design-docs/runtime-and-replay.md)
- [Security model](docs/design-docs/security-model.md)
- [Diagnostics reference](docs/design-docs/diagnostics.md)
- [Meeting scheduler example](examples/meeting-scheduler/main.aster)
- [Governed-note example](examples/governed-note/README.md)
- [v0.2.0 release notes](docs/releases/v0.2.0.md)
