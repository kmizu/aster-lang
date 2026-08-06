# ASTER 0.1 Language and Runtime Specification

Status: normative for the implemented ASTER 0.1 experimental vertical slice.
The external runtime wire boundary is specified separately by the
[ASTER 0.2 host protocol](aster-host-protocol-0.2.md); it does not change this
source-language version.

## Source and lexical form

An ASTER source file is valid UTF-8. Identifiers match
`[A-Za-z_][A-Za-z0-9_]*`; dotted declaration paths are sequences of identifiers.
No Unicode normalization is performed. Decimal integer literals, JSON string
literals, triple-quoted block strings, `//` comments, and nested `/* */`
comments are supported. Unterminated or malformed tokens are errors with exact
byte spans.

The parser accepts one `module` declaration followed by declarations. The
concrete declarations are `type`, `enum`, `capability`, `fn`, `flow`, `prompt`,
`validator`, `tool`, `policy`, and `agent`. Declaration order does not affect
resolution.

Block comments nest. A `/*` inside a block comment increments nesting depth and
each `*/` decrements it; end of file at nonzero depth is
`ASTER-PARSE-0005`. Regular strings use JSON escapes and may not cross a source
line. Triple-quoted strings preserve every byte between their delimiters and are
valid only in the static `instruction` position.

The concrete grammar below uses `*`, `+`, and `?` in their conventional EBNF
sense. `IDENT`, `INT`, `STRING`, `BLOCK_STRING`, and `MODEL_ALIAS` are lexical
tokens. Semantic passes impose the mode, purity, totality, and type restrictions
that are not context-free.

```ebnf
module        = "module", path, ";", declaration*, EOF ;
path          = IDENT, (".", IDENT)* ;
declaration   = type_decl | enum_decl | capability_decl | function_decl
              | flow_decl | prompt_decl | validator_decl | tool_decl
              | policy_decl | agent_decl ;

type_decl     = "type", IDENT, "=", (type_ref | record_type), ";" ;
record_type   = "{", type_field, (",", type_field)*, ","?, "}" ;
type_field    = IDENT, ":", type_ref ;
type_ref      = path, ("<", type_ref, (",", type_ref)*, ">")? ;
enum_decl     = "enum", IDENT, "{", enum_variant,
                (",", enum_variant)*, ","?, "}" ;
enum_variant  = IDENT, ("(", type_ref, ")")? ;
parameters    = "(", (parameter, (",", parameter)*, ","?)?, ")" ;
parameter     = IDENT, ":", type_ref ;

capability_decl = "capability", IDENT, parameters, ";" ;
function_decl = "fn", IDENT, parameters, "->", type_ref, block ;
flow_decl     = "flow", IDENT, parameters, "->", type_ref,
                "uses", capability_list, block ;
capability_list = "[", (capability_expr,
                  (",", capability_expr)*, ","?)?, "]" ;
capability_expr = path, arguments ;

prompt_decl   = "prompt", IDENT, parameters, "->", type_ref, "{",
                "instruction", BLOCK_STRING, ";",
                "data", "{", (IDENT, (",", IDENT)*, ","?)?, "}", ";",
                "}" ;
validator_decl = "validator", IDENT, parameters, "{",
                 ("require", expression, ";")*, "}" ;
tool_decl     = "tool", path, parameters, "->", type_ref, "{",
                tool_metadata*, "}" ;
tool_metadata = "mode", ("read" | "write"), ";"
              | "capability", capability_expr, ";"
              | "sensitivity", ("public" | "internal" | "private" | "secret"), ";"
              | "risk", ("reversible" | "irreversible"), ";"
              | "idempotency", IDENT, ";" ;

policy_decl   = "policy", IDENT, parameters, "{", policy_rule*, "}" ;
policy_rule   = "allow", "when", expression, ";"
              | "approve", "by", expression, "when", expression, ";"
              | "deny", expression, ("when", expression | "otherwise"), ";" ;

agent_decl    = "agent", IDENT, parameters, "requires", capability_list,
                "{", state_block, budget_block, handler+, "}" ;
state_block   = "state", "{",
                (IDENT, ":", type_ref, "=", expression, ";")*, "}" ;
budget_block  = "budget", "per_event", "{",
                (IDENT, "<=", INT, ";")*, "}" ;
handler       = "on", IDENT, parameters, "->", type_ref, block ;

block         = "{", statement*, "}" ;
statement     = "let", IDENT, (":", type_ref)?, "=", expression, ";"
              | "require", expression, ";"
              | "update", "state", "{", field_assignment+, "}"
              | "return", expression, ";"
              | expression, ";" ;
field_assignment = IDENT, "=", expression, ";" ;

expression    = literal | path | list | record | call | projection
              | unary | binary | try | if_expr | match_expr
              | infer_expr | validate_expr | observe_expr | intent_expr
              | propose_expr | authorize_expr | commit_expr | reconcile_expr
              | "(", expression, ")" ;
arguments     = "(", (argument, (",", argument)*, ","?)?, ")" ;
argument      = (IDENT, "=")?, expression ;
list          = "[", (expression, (",", expression)*, ","?)?, "]" ;
record        = path, "{", record_field, (",", record_field)*, ","?, "}" ;
record_field  = IDENT, "=", expression ;
if_expr       = "if", expression, block, "else", block ;
match_expr    = "match", expression, "{", match_arm,
                (",", match_arm)*, ","?, "}" ;
match_arm     = pattern, "=>", expression ;
pattern       = "_" | path, ("(", IDENT, ")")? ;
infer_expr    = "infer", path, arguments, "using", MODEL_ALIAS ;
validate_expr = "validate", expression, "with", path ;
observe_expr  = "observe", path, arguments ;
intent_expr   = "intent", path, "{", field_assignment+, "}" ;
propose_expr  = "propose", path, arguments, "for", expression ;
authorize_expr = "authorize", expression, "using", path ;
commit_expr   = "commit", expression, "with", expression ;
reconcile_expr = "reconcile", expression, "against", expression,
                 "with", path ;
```

Binary precedence, from weakest to strongest, is `||`, `&&`, equality,
ordering, addition/subtraction, and multiplication/division. Binary operators
associate left. Unary `!` and `-`, postfix `?`, projection, and calls bind more
tightly. Record declarations and constructions contain at least one field in
0.1; this also keeps `{}` after an `if` condition or `match` scrutinee
unambiguous.

## Types and values

Primitive types are `Unit`, `Bool`, `Int`, `Text`, `Instant`, `Duration`,
`ProvenanceRef`, and `Error`. Compiler-known constructors are `Option<T>`,
`Result<T,E>`, `List<T>`, `Incoming<T>`, `Untrusted<T>`, `Candidate<T>`,
`Checked<T>`, `Observation<T>`, `Secret<T>`, `Intent<P>`, `Proposal<A>`,
`Permit<A>`, `Receipt<A>`, and `Reconciled<A>`.

Programs may define non-generic aliases, records, single-payload or nullary
enums, and capability signatures. There is no null, inheritance, user generic,
implicit conversion, exception, reflection, macro, dynamic type, or cast.

`Incoming`, `Untrusted`, `Checked`, `Observation`, `Receipt`, and `Reconciled`
expose their documented `value`. `Candidate`, `Secret`, capabilities, and
permits expose no payload. Equality is structural only for equatable types and
is unavailable for opaque governance values.

## Pure computation and effects

Expressions include literals, records, constructors, lists, projections,
statically resolved calls, unary and arithmetic/comparison/boolean operators,
`if`, finite `match`, and `Result` propagation with `?`. Statements are `let`,
`require`, transactional `update state`, `return`, and expression statements.
There are no loops, recursion, closures, mutable locals, or detached tasks.
Bindings are lexical: a `let` or pattern payload may shadow an outer name only
inside its block or match arm, and branch-local bindings never escape.
Declared functions, flows, prompts, tools, and capabilities accept positional
arguments or an exact set of declared names. Built-ins and enum constructors
accept positional arguments only.

`fn`, validators, and policy conditions are pure and deterministic. A `flow`
declares an upper bound in `uses`; inferred effects must be its subset. Event
handler effects must be covered by the agent's `requires` list. An agent has at
least one handler, and every handler accepts exactly one `Incoming<T>` payload.
Pure declaration metadata, including state defaults and capability expressions,
supports the same finite `if`/`match` block computation but cannot return,
update state, or perform an effect.

Inference has type `Result<Candidate<T>, Error>`. Only
`validate candidate with Validator` can create `Checked<T>`. Read tools execute
only through `observe`. Write tools execute only through
`intent -> propose -> authorize -> commit`, and successful writes are checked
by `reconcile`.

## Capabilities, policies, affine values, and budgets

Source declares capability requirements but cannot mint grants. Runtime grants
are versioned JSON values and must exactly match the evaluated typed request.

Policies are ordered total decision tables. The first matching `allow`,
`approve`, or `deny` rule wins, and there is exactly one `otherwise` rule in the
final position. A policy's optional second parameter is the authorizing
agent's exact `Agent.State`; a stateful policy cannot be authorized from a flow
whose agent context is not statically known. Approval is an external suspension
after pure policy evaluation.

Proposals are immutable and hash action, arguments, intent, risk, sensitivity,
capability request, idempotency key, program identity, and schema version. The
runtime revalidates that seal before issuing and immediately before consuming a
permit, so a deserialized or host-mutated proposal cannot acquire or retain
authority.
Permit and proposal consumption is affine; a commit requires matching action
types and the runtime also checks proposal hash, issue and expiry times,
capability fingerprint, policy decision evidence, issuance-ledger membership,
unique permit identity, and unused status.

Per-event budget dimensions are `model_calls`, `model_tokens`,
`external_reads`, `external_writes`, `approvals`, and `money_microunits`.
Omitted dimensions are zero. Reservations happen before driver invocation and
settlement rejects actual usage above the fixture-declared maximum. Each trace
effect records the complete budget ledger before reservation, count and
variable reservations, the ledger after reservation, actual and released
usage, and the ledger after settlement; replay recomputes all of it.

## Prompt, secret, and boundary rules

Every prompt has exactly one static triple-quoted instruction and a structured
data channel. Instructions cannot interpolate or evaluate expressions.
`Secret<T>` has no source constructor and cannot enter prompts, state,
ordinary diagnostics, console output, traces, snapshots, equality, hashing, or
string conversion.

`Candidate<T>` may be transported only as an opaque in-memory value on the way
to validation. Candidate-containing values cannot appear in prompt or tool
schemas, capability grants, agent inputs, handler outputs, or persistent state;
these restrictions also apply through aliases, records, enums, and containers.

Boundary types are checked transitively. Capability parameters, prompt results,
validator values, tool results, agent state, and handler results contain only
plain data. External agent inputs may additionally use `Incoming` and
`Untrusted`; prompt inputs may additionally use `Checked` and `Observation`;
tool arguments may additionally use `Secret`. Governance and authority wrappers
are never constructible from ordinary JSON.

Event, state, capability, fixture, trace, snapshot, and final-state JSON each
carry `schema_version: 1`; unknown fields and privileged wrapper construction
are rejected at the boundary. Instants normalize to RFC 3339 UTC seconds ending
in `Z`. Runtime JSON objects use deterministic key ordering.

## Versioned JSON boundary schemas

All objects in this section reject unknown fields. A schema version other than
the integer `1` is rejected. Files are UTF-8 JSON; traces are UTF-8 JSON Lines.
Ordinary JSON can construct only declared data types and the external
`Incoming`/`Untrusted` wrappers inserted by the runtime. It cannot construct
`Candidate`, `Checked`, `Observation`, `Intent`, `Proposal`, `Permit`,
`Receipt`, `Reconciled`, a capability value, or `Secret`.

### Event and state

The event input has exactly this shape:

```json
{
  "schema_version": 1,
  "event_id": "evt-001",
  "event_time": "2026-08-05T12:00:00Z",
  "agent_arguments": { "user": "user-001" },
  "payload": { "text": "Schedule a meeting" }
}
```

`event_time` must already be canonical `YYYY-MM-DDTHH:MM:SSZ`; offsets,
fractional seconds, invalid dates, and leap seconds are rejected. Agent
arguments and payload are decoded against the selected agent and handler.

Initial state has exactly `{ "schema_version": 1, "state": OBJECT }`. Unknown
state fields fail. Supplied fields are decoded against their declarations;
omitted fields use their source-declared defaults. Final state has exactly the
same envelope and contains every declared field in canonical name order.

Null represents `Unit` or `None`; `Some(value)` is
`{"some":VALUE}`. `Ok(value)` is `{"ok":VALUE}` and `Err(error)` is
`{"error":ERROR}`. These tags keep `Some(Unit)` distinct from `None` and keep
an `Ok` record containing a field named `error` distinct from `Err`. Each
tagged object has exactly one key. A user enum is `{"variant":"Name"}` for a nullary variant and
`{"variant":"Name","value":PAYLOAD}` for a payload variant. The variant must
belong to the statically expected enum, and extra keys are rejected.

### Capability grants and fixtures

Capability grants have exactly this shape:

```json
{
  "schema_version": 1,
  "grants": [
    { "capability": "ModelUse", "arguments": ["planner"] }
  ]
}
```

Each name must resolve to a declared capability, argument count and types must
match, and duplicate exact grants fail. Runtime authorization uses exact typed
equality; the file has no wildcard syntax.

Fixture files have this schema:

```json
{
  "schema_version": 1,
  "entries": [
    {
      "kind": "model",
      "identity": "ParseMeeting",
      "match_request": { "model_alias": "planner" },
      "response": { "title": "Planning" },
      "max_usage": { "model_tokens": 100 },
      "actual_usage": { "model_tokens": 20 }
    }
  ]
}
```

`kind` is `model`, `read`, `approval`, or `write`. `identity` is the resolved
prompt, tool, or policy name. `match_request` is a nonempty recursive subset of
the complete request payload. Matching uses kind and identity first, then that
subset. Distinct simultaneous matches are ambiguous and fail; identical
duplicate match templates are consumed in source order. `response` is decoded
against the declared result schema. `max_usage` is reserved before the driver
call and `actual_usage` must use only reserved dimensions and not exceed it.

### Effect resolutions, traces, and snapshots

A durable resume resolution has exactly:

```json
{
  "request_hash": "sha256-hex",
  "payload": {},
  "actual_usage": {}
}
```

The hash must equal the snapshot's pending request. Each trace line has exactly
the following keys:

```json
{
  "schema_version": 1,
  "run_id": "sha256-hex",
  "sequence": 0,
  "kind": "run_header",
  "payload": {},
  "previous_entry_hash": "",
  "entry_hash": "sha256-hex"
}
```

Entries are contiguous and share one run ID. The implemented logical kinds are
`run_header`, `event_received`, `fingerprints`, `effect_requested`,
`budget_reserved`, `snapshot_written`, `effect_resolved`, `budget_settled`,
`policy_decision`, `permit_issued`, `proposal_committed`,
`reconciliation_decision`, `state_committed`, and exactly one terminal
`run_completed` or `run_failed`.

A snapshot is one JSON object with these top-level fields:
`schema_version`, `runtime_version`, `program_hash`, `agent`, `event`,
`event_id`, `event_time`, `input_hash`, `frames`, `current_state`,
`pending_state`, `budget`, `grant_fingerprint`, `grant_request_hashes`,
`authority`, `outstanding_receipts`, `trace_position`, `trace_hash`,
`pending_effect`, and `snapshot_hash`. Frames contain `routine`,
`instruction_pointer`, `locals`, `slots`, and `return_target`. A pending effect
contains its complete request, target slot, count reservation, variable-usage
reservations, pre-reservation budget ledger, and typed completion descriptor.
Internal runtime values use a tagged `{ "kind": ..., "value": ... }` encoding;
provenance-bearing wrappers
store a stable non-secret provenance reference alongside their typed value.
Snapshots reject unknown nested fields, secret handles, program/runtime/schema
mismatches, and content-hash changes.

## Machine, trace, replay, and state

Effectful source lowers to serializable explicit instructions. Machine state
contains the instruction pointer, frames, locals, current state, pending state
delta, budgets, capability fingerprint, permit-consumption and
outstanding-receipt ledgers, trace position, and any pending request. No
snapshot contains a host closure. A top-level handler cannot complete while a
committed receipt remains unreconciled.

Trace entries contain schema version, run ID, sequence, kind, payload, previous
entry hash, and entry hash. Hashing uses recursively key-sorted canonical JSON
and SHA-256. Replay verifies the chain and source/input/state/capability
fingerprints, re-executes deterministic semantics, compares each request, and
uses recorded resolutions without a driver. Any mismatch is a hard divergence.
Failed handlers never publish pending state.

## Formatting and diagnostics

Canonical formatting preserves comments and prompt instruction contents,
normalizes layout and punctuation, emits one trailing newline, and is
idempotent. Malformed syntax is diagnosed rather than discarded.

Comments remain in source order. A comment before the module remains before the
module. Other comments are canonically emitted immediately before the next
declaration whose source extent contains or follows the comment; trailing
comments remain after the final declaration. This deterministic attachment rule
preserves comment text without making original whitespace semantically relevant.

Diagnostics use the stable families documented in
[diagnostics.md](../design-docs/diagnostics.md). JSON field order and lists are
deterministic. CLI exit codes are 0 success, 1 source/check/format failure, 2
runtime/fixture/capability/budget/policy failure, 3 replay/tamper/schema
divergence, and 4 internal invariant failure.

## Explicit non-goals

ASTER 0.1 has no live model provider, MCP/OpenAPI client, arbitrary network or
shell operation, FFI, package import, multi-agent spawn, vector memory,
heartbeat scheduler, compensation saga, distributed execution, generated-code
evaluation, user generic, concurrency, loops, recursion, theorem proving, trace
encryption, or backward-compatibility promise.
