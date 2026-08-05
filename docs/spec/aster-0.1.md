# ASTER 0.1 Language and Runtime Specification

Status: normative for the 0.1 bootstrap. Features named here are unavailable
until their implementation and conformance evidence land in the same change.

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

`fn`, validators, and policy conditions are pure and deterministic. A `flow`
declares an upper bound in `uses`; inferred effects must be its subset. Event
handler effects must be covered by the agent's `requires` list.

Inference has type `Result<Candidate<T>, Error>`. Only
`validate candidate with Validator` can create `Checked<T>`. Read tools execute
only through `observe`. Write tools execute only through
`intent -> propose -> authorize -> commit`, and successful writes are checked
by `reconcile`.

## Capabilities, policies, affine values, and budgets

Source declares capability requirements but cannot mint grants. Runtime grants
are versioned JSON values and must exactly match the evaluated typed request.

Policies are ordered total decision tables. The first matching `allow`,
`approve`, or `deny` rule wins and a final `otherwise` is mandatory. Approval is
an external suspension after pure policy evaluation.

Proposals are immutable. Permit and proposal consumption is affine; a commit
requires matching action types and the runtime also checks proposal hash,
expiry, capability fingerprint, unique permit identity, and unused status.

Per-event budget dimensions are `model_calls`, `model_tokens`,
`external_reads`, `external_writes`, `approvals`, and `money_microunits`.
Omitted dimensions are zero. Reservations happen before driver invocation and
settlement rejects actual usage above the fixture-declared maximum.

## Prompt, secret, and boundary rules

Every prompt has exactly one static triple-quoted instruction and a structured
data channel. Instructions cannot interpolate or evaluate expressions.
`Secret<T>` has no source constructor and cannot enter prompts, state,
ordinary diagnostics, console output, traces, snapshots, equality, hashing, or
string conversion.

Event, state, capability, fixture, trace, snapshot, and final-state JSON each
carry `schema_version: 1`; unknown fields and privileged wrapper construction
are rejected at the boundary. Instants normalize to RFC 3339 UTC seconds ending
in `Z`. Runtime JSON objects use deterministic key ordering.

## Machine, trace, replay, and state

Effectful source lowers to serializable explicit instructions. Machine state
contains the instruction pointer, frames, locals, current state, pending state
delta, budgets, capability fingerprint, affine ledger, trace position, and any
pending request. No snapshot contains a host closure.

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
