# ASTER 0.2 Host Protocol Specification

Status: normative for the ASTER 0.2 runtime host boundary. The ASTER source
language remains version 0.1.

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHOULD**, and **MAY** in
this document are normative requirements.

## Purpose and authority boundary

The protocol lets an external host execute typed effects requested by ASTER.
ASTER owns program semantics, capabilities, fixed budgets, variable-usage
reservation and settlement, proposals, permits, transactional state, traces,
snapshots, reconciliation, and replay. The host owns provider integration and
the operating-system authority needed for its declared tools.

An `effect_preview` is data, not authority. A conforming host MUST NOT perform
the external effect until it receives the matching `execute_grant`. An
execution grant is a transport sequencing token; it does not replace a source
capability, policy decision, or `Permit<A>` and cannot broaden a proposal.

The protocol does not sandbox the host. A malicious host can act early, forge
provider behavior, or lie about usage by using authority it already possesses.
ASTER records and validates the exchange but cannot prevent those out-of-
process actions. Production hosts SHOULD be isolated with only the external
authority required by the effects they implement.

## Transport

`aster host` and `aster host-resume` use UTF-8 JSON Lines over standard input
and standard output. Each frame is exactly one JSON object followed by one LF
byte. ASTER standard output is reserved for protocol frames; diagnostics go to
standard error. ASTER flushes standard output after every frame.

A host input line, excluding its terminating LF, MUST be no larger than 1 MiB
(1,048,576 bytes). ASTER rejects an oversized line before JSON decoding. Empty
input while a reply is outstanding is premature EOF. A non-empty final line
without LF, invalid UTF-8, malformed JSON, an unknown field at any nesting
level, an unknown message kind, or an unsupported version is a controlled
protocol failure.

## Common envelopes

Every ASTER-to-host frame has exactly these fields:

```json
{
  "schema_version": 1,
  "session_id": "<run id>",
  "message_id": 0,
  "kind": "hello",
  "payload": {}
}
```

- `schema_version` MUST be integer `1`.
- `session_id` is the deterministic run ID and MUST remain identical for the
  run, including crash/resume.
- `message_id` is an ASTER-issued unsigned integer. It starts at `0` for
  `hello` and increases by one for each outbound frame.
- `kind` selects exactly one payload shape defined below.
- `payload` MUST contain exactly the fields defined for that kind.

Every host-to-ASTER reply has exactly these fields:

```json
{
  "schema_version": 1,
  "session_id": "<same run id>",
  "in_reply_to": 0,
  "kind": "hello_ack",
  "payload": {}
}
```

`schema_version` and `session_id` have the same requirements as outbound
frames. `in_reply_to` MUST equal the `message_id` of the one currently
outstanding ASTER frame. A host MUST send no unsolicited frame and MUST NOT
reply twice. ASTER rejects stale, skipped, duplicate, out-of-order, and cross-
session replies.

There is at most one outstanding reply. The only legal reply mapping is:

| ASTER frame | Required host reply |
| --- | --- |
| `hello` | `hello_ack` |
| `effect_preview` | `effect_admission` |
| `execute_grant` | `effect_resolution` |
| `completed` | none |
| `failed` | none |

## Handshake

ASTER MUST emit `hello` before any effect frame. Its payload is:

```json
{
  "protocol": "aster-host",
  "protocol_version": 1,
  "runtime_version": "0.2.0",
  "program_hash": "<canonical SHA-256>",
  "run_id": "<same value as session_id>"
}
```

The host MUST answer with `hello_ack`:

```json
{
  "protocol": "aster-host",
  "protocol_version": 1
}
```

Both values MUST match exactly. ASTER admits no effect before a valid
acknowledgement.

## Effect exchange

Each external effect follows preview, admission, durable grant, execution, and
resolution in that order.

### Preview

ASTER emits `effect_preview` after the deterministic VM yields and after the
complete request has entered the hash-chained trace:

```json
{
  "request": {
    "kind": "model",
    "identity": "DraftNote",
    "payload": {},
    "request_hash": "<canonical SHA-256>"
  }
}
```

`kind` is one of `model`, `read`, `approval`, or `write`. `identity` is the
declared prompt, policy, or tool identity. `payload` is the complete typed
request data. `request_hash` binds the full request. The host MUST treat every
field as immutable and MUST NOT perform the effect at this phase.

The host answers with `effect_admission`:

```json
{
  "request_hash": "<exact preview request hash>",
  "max_usage": {
    "model_tokens": 200,
    "money_microunits": 1000
  }
}
```

`request_hash` MUST match the outstanding request. `max_usage` may contain
only `model_tokens` and `money_microunits`, each at most once and with an
unsigned integer value. A zero or irrelevant dimension MAY be omitted. The
host cannot declare or change the fixed `model_calls`, `external_reads`,
`external_writes`, or `approvals` counters.

ASTER verifies the request binding and atomically reserves every declared
maximum. Exhaustion or an invalid dimension fails before any grant.

### Durable execution grant

After admission, ASTER appends reservation evidence, checkpoints the trace,
seals the pending continuation, records its snapshot hash, and atomically
persists the snapshot and current trace before writing `execute_grant` to
standard output. Its payload is:

```json
{
  "request": {
    "kind": "write",
    "identity": "Workspace.store",
    "payload": {},
    "request_hash": "<canonical SHA-256>"
  },
  "max_usage": {},
  "snapshot_hash": "<canonical SHA-256>",
  "execution_grant_hash": "<canonical SHA-256>"
}
```

`request` and `max_usage` MUST equal the admitted values. The execution-grant
hash binds protocol version, run ID, request hash, trace checkpoint position,
trace checkpoint hash, snapshot hash, and maximum usage. A conforming host MAY
execute only this exact effect after receiving this frame.

### Resolution and settlement

The host answers with `effect_resolution`:

```json
{
  "request_hash": "<exact request hash>",
  "execution_grant_hash": "<exact execution grant hash>",
  "payload": {},
  "actual_usage": {
    "model_tokens": 12,
    "money_microunits": 200
  }
}
```

Both hashes MUST match the outstanding grant. `payload` MUST decode as the
declared prompt, tool, or approval result type. `actual_usage` MUST contain
exactly the same dimensions as the admitted `max_usage`; every value MUST be
no greater than its maximum. Duplicate or unknown dimensions are invalid.

ASTER validates grant binding and usage before appending the resolution. It
then supplies the typed result to the VM, deterministically settles budget,
and continues. Another effect repeats the entire exchange; no admission or
grant carries over.

## Terminal frames

After successful handler completion, ASTER atomically publishes transactional
state and emits `completed`:

```json
{
  "final_state_hash": "<canonical SHA-256>",
  "trace_hash": "<hash-chain head>"
}
```

After a controlled protocol or runtime failure, ASTER preserves valid evidence
and, when standard output is writable, emits `failed`:

```json
{
  "code": "ASTER-HOST-11003",
  "summary": "host protocol binding mismatch"
}
```

Terminal frames require no reply. EOF is valid only after a terminal frame.
Host protocol failures exit with CLI class 2. A failed run does not publish a
successful final state.

## Crash and resume

The durable execution point is the sealed snapshot and trace persisted before
`execute_grant`. `aster host-resume` verifies snapshot schema and seal, program
hash, runtime version, trace position and chain head, pending request, and
reserved-usage evidence. It performs a fresh `hello` / `hello_ack` handshake
for the same session ID and then re-emits the same grant, including identical
request, maximums, snapshot hash, and execution-grant hash. It MUST NOT emit a
second preview or accept a second admission.

If a crash occurs after the external operation but before ASTER receives its
resolution, ASTER cannot know whether the operation happened. The host MUST
use the declared idempotency key or provider lookup before resolving a resumed
write. ASTER does not silently execute or retry the write. A pending receipt
continues to block completion until a later observation reconciles it.

## Replay

Replay verifies the trace chain and run fingerprints, re-steps the VM, compares
every complete effect request, and injects only the recorded matching
resolution. It recomputes budget, policy, proposal, permit, reconciliation,
and final state. `aster replay` has no host or driver parameter and MUST NOT
invoke an external driver. This driver-free replay property is structural, not
a convention.

## Diagnostics and disclosure

Protocol failures use stable codes:

- `ASTER-HOST-11001`: malformed or unsupported frame, including malformed
  JSON, invalid UTF-8, unknown fields/kinds/version, and oversized input;
- `ASTER-HOST-11002`: stale, duplicate, skipped, unsolicited, or otherwise
  out-of-sequence reply;
- `ASTER-HOST-11003`: session, request, snapshot, or grant binding mismatch;
- `ASTER-HOST-11004`: duplicate, unknown, missing, or over-maximum usage;
- `ASTER-HOST-11005`: EOF before a required reply; and
- `ASTER-HOST-11006`: an outbound frame could not be written completely.

`ASTER-RUNTIME-9001` remains the code for a typed deterministic runtime failure
that is not a host framing error.

Errors and `failed` frames contain only stable classifications and redacted
summaries. They MUST NOT include complete inbound frames, prompt data, tool
arguments or results, private payloads, or secret material. `Secret<T>` MUST
NOT enter the protocol, standard output, diagnostics, traces, snapshots,
persistent state, hashes, or string conversion. Traces and snapshots may hold
private non-secret data and MUST be protected as sensitive artifacts.

## Compatibility

Protocol version 1 has no extension negotiation. Unknown fields and versions
are rejected. A future incompatible field, sequencing, or semantic change MUST
increment `protocol_version`; persistence changes MUST increment their own
schema version and reject unsupported artifacts explicitly.

The protocol does not change ASTER 0.1 source syntax or static semantics.
Fixture-backed `aster run`, explicit `aster resume`, and `aster replay` remain
supported and share the same deterministic `RecordSession` behavior.
