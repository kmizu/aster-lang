# Bootstrap ASTER 0.1: an executable language for governed AI agents

You are the principal implementer for a new programming language named **ASTER**.

Your task is to take the repository in the current working directory from its present state—possibly empty—to a complete, tested, documented, runnable **ASTER 0.1 vertical slice**. Do not stop at design notes, a parser-only scaffold, mocked CLI success, pseudocode, or unimplemented placeholders. The result must compile, run the included example, record a deterministic trace, replay that trace without invoking external drivers, and reject the specified unsafe programs with stable diagnostics.

The repository root contains a companion `AGENTS.md`. Read it first and obey it throughout the task. Preserve it. You may update paths or commands in it only when the implementation genuinely requires that change, and any update must preserve or strengthen its semantic and maintenance invariants.

Do not ask for clarification. Resolve underspecified implementation details in favor of:

1. semantic safety;
2. determinism and replayability;
3. a small coherent language;
4. explicit, inspectable representations;
5. maintainability by future coding agents;
6. a working end-to-end vertical slice rather than broad but shallow coverage.

Document material decisions in the repository.

---

## 1. Product thesis

ASTER is a deterministic, auditable language and runtime for AI agents that interact with models, tools, humans, and persistent state.

The central semantic boundary is:

> **Model output is a `Candidate`; external action is a `Proposal`; authority is a `Permit`; reality is checked by `Reconciliation`.**

The LLM is not the executor, security principal, or owner of ambient tools. It is a typed, nondeterministic inference oracle that can produce candidate data or candidate plans. The ASTER runtime owns control flow, capabilities, policy evaluation, budgets, external effects, durable state, trace recording, and replay.

ASTER 0.1 is intentionally not a general-purpose replacement for Rust, Python, or Scala. It is an orchestration language for governed agents. Pure computation can later be imported from WASM or host languages, but no arbitrary FFI is required in this milestone.

---

## 2. Required outcome

At completion, all of the following must be true:

- The repository is a Rust workspace that builds on stable Rust.
- `aster check` parses and statically checks ASTER source.
- `aster fmt` emits a canonical, idempotent representation.
- `aster ast --json` emits a machine-readable syntax tree.
- `aster run` executes the bundled meeting-scheduler example with fixture-backed model, tool, and human-approval drivers.
- `aster run` writes:
  - a final agent-state file;
  - an append-only, hash-chained trace;
  - serializable machine snapshots at external effect boundaries.
- `aster replay` reproduces the same final state and externally visible result from the trace while making **zero** driver calls.
- `aster replay` rejects a modified program, changed request, reordered effect, or tampered trace entry as a replay divergence.
- The checker rejects all mandatory unsafe fixtures listed later, with stable diagnostic codes and useful source spans.
- The workspace has unit tests, compile-pass/compile-fail conformance tests, runtime/replay tests, CLI black-box tests, CI, architecture checks, documentation checks, and a single `./scripts/check.sh` entry point.
- The included `AGENTS.md`, specification, architecture document, design documents, ADRs, examples, and execution plan agree with the implementation.
- Production code contains no reachable placeholders or fake success paths.
- All required checks pass after the final edit.

---

## 3. Working method

Follow this sequence:

1. Inspect the repository, all applicable `AGENTS.md` files, Git status, toolchain, and any existing work.
2. Create `docs/exec-plans/active/bootstrap-aster-0.1.md` before substantial implementation. Maintain:
   - goal and acceptance criteria;
   - milestones;
   - progress checkboxes;
   - decisions and rationale;
   - discoveries and deviations;
   - commands run and results;
   - known limitations.
3. Establish the workspace and repository knowledge structure.
4. Implement a thin end-to-end path early:
   - parse one file;
   - check it;
   - lower it;
   - run one fixture-backed effect;
   - record it;
   - replay it.
5. Expand that path until every required semantic invariant and acceptance test is covered.
6. Run narrow tests continuously.
7. Run `./scripts/check.sh` after the final modification.
8. Review the entire diff for semantic shortcuts, nondeterminism, secret leakage, stale docs, and unrelated changes.
9. Move the completed execution plan to `docs/exec-plans/completed/` only when the definition of done is actually met.

Do not compensate for a failing test by weakening the test, suppressing a diagnostic, or bypassing a layer. Fix the underlying implementation.

---

## 4. Non-negotiable ASTER invariants

Implement and test these as language/runtime properties, not prose conventions:

1. `Candidate<T>` is opaque. It has no public `.value` projection and no cast, coercion, pattern match, serialization escape hatch, or generic helper that extracts `T`.
2. Only `validate candidate with Validator` can produce `Checked<T>`.
3. Model inference returns `Result<Candidate<T>, Error>`.
4. Model providers never receive tool handles or capabilities.
5. A read tool can be invoked only with `observe`.
6. A write tool can be invoked only through:
   `intent -> propose -> authorize -> commit`.
7. `Proposal<A>` is immutable after construction.
8. `Permit<A>` is affine, single-use, expiring, and bound at runtime to the exact canonical hash of one proposal.
9. `commit` consumes both its proposal and permit. Reuse is a static affine-use error when visible statically and a runtime rejection for forged/deserialized values.
10. Policies and validators are pure, deterministic, and total over declared inputs. They cannot infer, observe, authorize, commit, reconcile, read a clock, use randomness, mutate state, inspect ambient configuration, or invoke effectful flows.
11. Capabilities are issued by runtime configuration. Source code may declare requirements and delegate narrower subsets, but may not mint or broaden authority.
12. Every external effect checks or reserves budget before driver invocation and settles usage deterministically afterward.
13. Replay does not instantiate or call model, tool, or approval drivers.
14. Prompt instructions are static block-string literals. Runtime/untrusted/memory values can enter only the structured `data` channel.
15. `Secret<T>` is opaque and cannot enter model data, prompt instructions, ordinary diagnostics, console logs, traces, snapshots, persistent state, equality, hashing exposed to source, or string conversion.
16. All external responses are decoded against declared ASTER types before becoming effect results.
17. Pure evaluation and policy evaluation are deterministic.
18. Effectful code lowers to explicit serializable control flow. Do not implement external effects by recursively interpreting AST nodes that directly call drivers.
19. Serialized forms use versioned schemas and deterministic key/order handling.
20. No ASTER 0.1 feature may execute arbitrary shell commands, open arbitrary network connections, load native plugins, or evaluate generated ASTER source.

Any architecture that makes one of these properties merely “expected usage” rather than enforceable is unacceptable.

---

## 5. Implementation stack and repository layout

Use stable Rust with `#![forbid(unsafe_code)]` in every crate.

Create this workspace structure unless existing repository constraints require a clearly documented variation:

```text
.
├── AGENTS.md
├── ARCHITECTURE.md
├── Cargo.toml
├── Cargo.lock
├── README.md
├── rust-toolchain.toml
├── .gitignore
├── .github/
│   └── workflows/
│       └── ci.yml
├── crates/
│   ├── aster-diagnostics/
│   ├── aster-syntax/
│   ├── aster-semantics/
│   ├── aster-ir/
│   ├── aster-runtime/
│   └── aster-cli/
├── docs/
│   ├── spec/
│   │   └── aster-0.1.md
│   ├── design-docs/
│   │   ├── core-beliefs.md
│   │   ├── runtime-and-replay.md
│   │   ├── security-model.md
│   │   └── diagnostics.md
│   ├── adr/
│   │   ├── 0001-rust-workspace-and-layering.md
│   │   ├── 0002-explicit-effect-machine.md
│   │   └── 0003-trace-canonicalization.md
│   └── exec-plans/
│       ├── active/
│       └── completed/
├── examples/
│   └── meeting-scheduler/
│       ├── main.aster
│       ├── event.json
│       ├── initial-state.json
│       ├── capabilities.json
│       └── fixtures.json
├── tests/
│   ├── conformance/
│   │   ├── pass/
│   │   └── fail/
│   └── expected/
└── scripts/
    ├── check.sh
    ├── check-architecture.sh
    └── check-docs.sh
```

Required dependency direction:

```text
aster-diagnostics
        ↑
aster-syntax
        ↑
aster-semantics
        ↑
aster-ir
        ↑
aster-runtime
        ↑
aster-cli
```

The CLI may depend on all lower crates for orchestration. No lower crate may depend on a higher one. Enforce this with `scripts/check-architecture.sh`, not documentation alone.

Use a small dependency set. Reasonable choices include `serde`, `serde_json`, `thiserror`, `clap`, `sha2`, `hex`, and focused test dependencies. Parsing strategy is your decision, but it must be deterministic, fully in-repository or based on a small maintained library, preserve accurate spans, produce useful recovery diagnostics, and support canonical formatting. Record the choice in an ADR.

Do not add network clients, async runtimes, native plugin frameworks, database clients, TLS stacks, or an actual OpenAI API integration. ASTER 0.1 uses fixture-backed drivers only.

---

## 6. ASTER 0.1 source model

ASTER 0.1 source is UTF-8 and file based. The MVP may compile one root source file without an import system. Reserve module paths for future multi-file use.

### 6.1 Lexical rules

Support:

- identifiers: `[A-Za-z_][A-Za-z0-9_]*`;
- dotted paths such as `Calendar.create`;
- model aliases such as `@planner`;
- decimal integers;
- `true` and `false`;
- JSON-style quoted strings;
- triple-quoted block strings;
- `//` line comments;
- nested or non-nested `/* ... */` block comments—choose one behavior and specify it;
- punctuation and operators required by the grammar below.

Reject malformed UTF-8 input at the CLI boundary, unterminated strings/comments, invalid escapes, invalid numeric literals, and unknown tokens with stable parse diagnostics and exact spans.

Do not perform Unicode identifier normalization in 0.1. State this in the specification.

### 6.2 Built-in types

Implement these built-in value types:

```text
Unit
Bool
Int
Text
Instant
Duration
ProvenanceRef
Error

Option<T>
Result<T, E>
List<T>

Incoming<T>
Untrusted<T>
Candidate<T>
Checked<T>
Observation<T>
Secret<T>

Intent<P>
Proposal<A>
Permit<A>
Receipt<A>
Reconciled<A>
```

`P` is an intent-purpose symbol. `A` is a statically resolved write-tool action symbol, such as `Calendar.create`.

Support user-defined non-generic:

- type aliases;
- record types;
- enums with nullary or single-payload variants;
- capabilities.

Do not implement:

- null;
- inheritance;
- traits/typeclasses;
- user-defined generics;
- implicit conversions;
- operator overloading;
- exceptions;
- reflection;
- macros;
- dynamic typing;
- arbitrary casts.

Built-in wrapper types are compiler-known and cannot be shadowed.

### 6.3 Core declarations

Support the declaration forms illustrated below. This fragment is schematic; the bundled meeting-scheduler program later in this prompt is the required complete executable example.

```aster
module example.name;

type UserId = Text;

type User = {
  id: UserId,
  name: Text
};

enum Choice {
  First,
  Other(Text)
}

capability ModelUse(alias: Text);
capability CatalogRead(id: Text);
capability CatalogWrite(id: Text);
capability HumanApproval(owner: UserId);

fn pure_function(x: Int) -> Bool {
  return x > 0;
}

flow effectful_function(x: Text) -> Result<Unit, Error>
uses [ModelUse("planner")] {
  return Ok(Unit);
}

prompt ParseThing(message: Untrusted<Text>) -> Thing {
  instruction """
  Extract only explicitly supported fields.
  """;

  data {
    message
  };
}

validator ThingRules(x: Thing) {
  require x.count > 0;
}

tool Catalog.lookup(id: Text) -> Item {
  mode read;
  capability CatalogRead(id);
  sensitivity private;
}

tool Catalog.update(id: Text, value: Item, request_id: Text) -> ItemRef {
  mode write;
  capability CatalogWrite(id);
  risk reversible;
  idempotency request_id;
  sensitivity private;
}

policy UpdatePolicy(p: Proposal<Catalog.update>, s: Worker.State) {
  allow when is_safe(p.args.value);
  approve by Human(s.owner) when true;
  deny "No policy rule allowed the update" otherwise;
}

agent Worker(owner: UserId)
requires [
  ModelUse("planner"),
  CatalogRead(owner),
  CatalogWrite(owner),
  HumanApproval(owner)
] {
  state {
    last_item: Option<ItemRef> = None;
  }

  budget per_event {
    model_calls <= 2;
    model_tokens <= 4000;
    external_reads <= 3;
    external_writes <= 1;
    approvals <= 1;
    money_microunits <= 10000;
  }

  on message(msg: Incoming<Message>) -> Result<Unit, Error> {
    return Ok(Unit);
  }
}
```

The parser may allow declarations in any order. Semantic resolution must be independent of declaration order and deterministic.

### 6.4 Expressions and statements

Implement enough pure expression language for meaningful validators, policies, and handlers:

- literals;
- variable references;
- dotted name references;
- record construction;
- enum and built-in constructors;
- list construction;
- field projection;
- pure function calls;
- built-in calls;
- unary `!` and numeric negation;
- arithmetic `+ - * /`;
- comparison `== != < <= > >=`;
- boolean `&& ||`;
- `if ... { ... } else { ... }` expressions;
- `match` over enums, `Option`, and `Result`;
- postfix `?` for `Result<T, Error>` propagation;
- parenthesized expressions.

Statements:

```aster
let name = expression;
let name: Type = expression;
require condition;
update state {
  field = expression;
}
return expression;
expression;
```

No user loops, recursion, closures, asynchronous blocks, detached tasks, mutable local variables, or general assignment in 0.1. Detect direct and mutual recursion in pure functions and flows and reject it with a stable diagnostic. This keeps source-level computation finite while the durable agent itself remains long-lived through events.

State updates are atomic at handler completion. External-effect snapshots may contain a pending transactional state delta, but failed handlers must not partially publish state.

### 6.5 Required pure built-ins

Implement and document at least:

```text
len(List<T>) -> Int
first(List<T>) -> Result<T, Error>
contains(List<T>, T) -> Bool
subset(List<T>, List<T>) -> Bool
Some(T) -> Option<T>
None -> Option<T>
Ok(T) -> Result<T, E>
Err(E) -> Result<T, E>
provenance(wrapper) -> ProvenanceRef
add_seconds(Instant, Int) -> Instant
```

Equality is available only for types that are structurally equatable. `Secret<T>`, `Candidate<T>`, capabilities, proposals, permits, and opaque runtime handles are not source-level equatable.

`Instant` values enter through event inputs or recorded effects. There is no ambient `now()` in 0.1.

---

## 7. Prompt and inference semantics

A prompt declaration has:

- typed parameters;
- a result type;
- exactly one static `instruction` block string;
- a structured `data` block naming its runtime parameters.

The instruction is syntax, not an expression. Interpolation, concatenation, variable references, and dynamic instruction promotion are invalid.

For:

```aster
prompt ParseMeeting(
  message: Untrusted<Text>
) -> MeetingRequest {
  instruction """
  Extract only meeting information explicitly present in the message.
  Do not invent people, addresses, dates, or times.
  """;

  data {
    message
  };
}
```

this expression:

```aster
infer ParseMeeting(message = msg.value.text) using @planner
```

has type:

```text
Result<Candidate<MeetingRequest>, Error>
```

The runtime effect request contains:

- prompt symbol;
- static instruction text;
- typed structured data;
- expected result schema;
- model alias;
- source provenance;
- budget reservation;
- deterministic request hash.

The fixture model driver returns structured JSON and usage metadata. Decode the JSON against the prompt result type before constructing `Candidate<T>`. A schema mismatch is a typed runtime error and a trace event, not a panic or unchecked dynamic value.

`Candidate<T>` must not expose its value to source code. Validation receives the hidden typed candidate internally:

```aster
let checked = (validate candidate with MeetingRules)?;
```

This has type `Checked<MeetingRequest>`. `Checked<T>.value` is readable.

A candidate may carry runtime metadata—model run identifier, raw output handle, and provenance—but raw output is not source-visible in 0.1.

---

## 8. Validators and checked values

A validator is a pure declaration:

```aster
validator MeetingRules(x: MeetingRequest) {
  require x.duration_minutes >= 15;
  require x.duration_minutes <= 120;
  require len(x.attendees) > 0;
}
```

Validators may call pure, nonrecursive functions and built-ins. They cannot read agent state unless state is explicitly passed as a normal value.

`validate candidate with V` is valid only when:

- the expression is `Candidate<T>`;
- `V` accepts exactly one `T`;
- `V` is pure.

It returns `Result<Checked<T>, Error>`.

For reconciliation, a validator may accept two parameters:

```aster
validator EventMatches(expected: EventRef, actual: EventRef) {
  require expected.id == actual.id;
}
```

Validation failures preserve:

- validator symbol;
- failed requirement span;
- a safe rendered summary;
- provenance of the candidate;
- no secret values.

Validators are deterministic. Evaluate all requirements and return an ordered list of failures rather than stopping at the first, unless an earlier expression itself fails to evaluate.

---

## 9. Tools, observations, and action types

A tool declaration is metadata and a typed boundary. It is not executable host code.

Read tool:

```aster
tool Calendar.free(
  owner: UserId,
  duration_minutes: Int
) -> List<Slot> {
  mode read;
  capability CalendarRead(owner);
  sensitivity private;
}
```

Write tool:

```aster
tool Calendar.create(
  owner: UserId,
  slot: Slot,
  title: Text,
  attendees: List<Email>,
  request_id: Text
) -> EventRef {
  mode write;
  capability CalendarWrite(owner);
  risk reversible;
  idempotency request_id;
  sensitivity private;
}
```

Required metadata:

- `mode read|write`;
- one capability expression;
- `sensitivity public|internal|private|secret`.

Write tools additionally require:

- `risk reversible|irreversible`;
- `idempotency <parameter-name>`.

The named idempotency parameter must exist and have a deterministically serializable type. Reject write declarations without it in 0.1.

Read invocation:

```aster
let slots = (observe Calendar.free(
  owner = user,
  duration_minutes = request.value.duration_minutes
))?;
```

Type:

```text
Observation<List<Slot>>
```

`Observation<T>.value` is readable and it carries a `ProvenanceRef`.

Reject:

- `observe` of a write tool;
- `propose` of a read tool;
- direct tool calls as ordinary functions;
- tool symbols passed as values;
- undeclared or missing required capabilities.

---

## 10. Intent, proposal, policy, permit, commit, and reconciliation

### 10.1 Intent

An intent is a pure immutable value:

```aster
let purpose = intent ScheduleMeeting {
  actor = self;
  beneficiary = user;
  basis = [
    provenance(msg),
    provenance(request),
    provenance(slots)
  ];
  expected = EventExpectation {
    owner = user,
    slot_id = selected.id
  };
  expires_at = add_seconds(event.time, 120);
};
```

Every intent must contain exactly:

- `actor`;
- `beneficiary`;
- non-empty `basis: List<ProvenanceRef>`;
- `expected`, of any non-secret serializable type;
- `expires_at: Instant`.

The purpose symbol becomes the phantom parameter of `Intent<ScheduleMeeting>`.

No model-generated free text can become a purpose symbol at runtime.

### 10.2 Proposal

For a write action:

```aster
let proposal = propose Calendar.create(
  owner = user,
  slot = selected,
  title = request.value.title,
  attendees = request.value.attendees,
  request_id = event.id
) for purpose;
```

This is pure and performs no external effect. It produces `Proposal<Calendar.create>` containing:

- resolved action identity;
- typed canonical arguments;
- intent;
- required capability request;
- risk;
- idempotency key;
- sensitivity;
- canonical proposal hash.

The proposal is immutable. Its canonical hash covers action identity, arguments, intent, risk, capability request, idempotency key, source program hash, and schema version.

### 10.3 Policy

A policy is an ordered, total decision table:

```aster
policy CalendarPolicy(
  p: Proposal<Calendar.create>,
  s: Scheduler.State
) {
  allow when all_known(p.args.attendees, s.profile.known_attendees);
  approve by Human(s.user) when true;
  deny "No policy rule allowed the action" otherwise;
}
```

Allowed clauses:

```text
allow when <pure Bool>;
approve by Human(<pure expression>) when <pure Bool>;
deny <static or pure Text> when <pure Bool>;
deny <static or pure Text> otherwise;
```

Require a final `otherwise` rule. Evaluate conditions in source order. The first match wins.

Policy evaluation itself is pure and deterministic. `approve` returns a decision requiring a human-approval effect; it does not call the driver from inside policy evaluation.

The policy input proposal exposes read-only:

- `p.args.<name>`;
- `p.intent`;
- `p.risk`;
- `p.action`;
- `p.idempotency_key`;
- metadata explicitly listed in the spec.

It never exposes a capability minting primitive or secret payload.

### 10.4 Authorization and human approval

```aster
let permit = (authorize proposal using CalendarPolicy)?;
```

Static type:

```text
Permit<Calendar.create>
```

Authorization:

1. verifies required capability against runtime grants;
2. verifies intent has not expired relative to the recorded event time;
3. evaluates the pure policy against an immutable state snapshot;
4. returns denial, direct permit, or a human-approval effect request;
5. for approval, verifies the approval response is bound to the proposal hash and principal;
6. creates an affine permit with:
   - proposal hash;
   - action identity;
   - grant/capability fingerprint;
   - policy identity and decision evidence;
   - issue time;
   - expiry;
   - unique permit identifier;
   - consumed flag maintained by runtime state.

The fixture approval driver returns a typed approval decision. Approval data is recorded for replay.

### 10.5 Commit

```aster
let receipt = (commit proposal with permit)?;
```

`commit`:

- statically requires matching `Proposal<A>` and `Permit<A>`;
- consumes both affine values;
- at runtime rechecks proposal hash, action, expiry, capability fingerprint, budget, and unused status;
- reserves write budget before invoking the driver;
- sends exactly the immutable proposal arguments and idempotency key;
- decodes the tool result against the declared result type;
- settles usage;
- records a `Receipt<A>` and trace entries.

A permit for one proposal cannot authorize another proposal of the same action.

The runtime must reject duplicate commit attempts even if a malicious snapshot or hand-built test bypasses static checking.

### 10.6 Reconciliation

A successful write response is not proof that the intended world state exists. The program performs a read observation and reconciles:

```aster
let actual = (observe Calendar.lookup(
  owner = user,
  event_id = receipt.value.id
))?;

let confirmed = (
  reconcile receipt against actual with EventMatches
)?;
```

`reconcile`:

- is pure after both values exist;
- checks a two-parameter validator against `receipt.value` and `observation.value`;
- emits a trace decision;
- returns `Reconciled<Calendar.create>`;
- exposes `.value` as the write result on success;
- returns a typed mismatch error on failure.

For ASTER 0.1, every committed write receipt must be either reconciled before normal handler completion or explicitly returned as a pending receipt. The bundled example must reconcile it. Prefer a static outstanding-receipt check; if that is too invasive, enforce it as a runtime completion error and document the exact limitation. Do not silently discard an unreconciled receipt.

---

## 11. Capabilities

Capabilities are declared in source:

```aster
capability ModelUse(alias: Text);
capability CalendarRead(owner: UserId);
capability CalendarWrite(owner: UserId);
capability HumanApproval(owner: UserId);
```

An agent declares requirements:

```aster
requires [
  ModelUse("planner"),
  CalendarRead(user),
  CalendarWrite(user),
  HumanApproval(user)
]
```

Runtime grants come from `capabilities.json`. Source declarations do not mint grants.

Static checks:

- every inferred model alias is covered by a declared agent/flow requirement;
- every observed/proposed tool capability kind is covered by requirements;
- a flow’s inferred effects are a subset of its `uses`;
- a handler’s effects are a subset of its agent’s `requires`;
- pure functions, validators, and policies have an empty effect set.

Runtime checks evaluate capability arguments and scopes against concrete runtime values. Use exact typed equality for 0.1; no wildcard or policy language is required in capability files. A grant must match the resolved capability request exactly.

Capabilities are opaque runtime values and cannot be serialized by source programs, logged as bearer material, compared, or constructed.

---

## 12. Effects and reusable flows

A `fn` is pure. A `flow` may contain effects and must declare an upper bound with `uses`.

Example:

```aster
flow parse_request(
  msg: Incoming<UserMessage>
) -> Result<Checked<MeetingRequest>, Error>
uses [ModelUse("planner")] {
  let candidate = (
    infer ParseMeeting(message = msg.value.text) using @planner
  )?;

  return validate candidate with MeetingRules;
}
```

Infer effects from the body and verify they are a subset of `uses`.

Flow calls are statically resolved. Detect direct and mutual recursion across both `fn` and `flow`. No higher-order calls.

Effect categories in 0.1:

- model inference;
- read tool;
- human approval;
- write tool commit.

Event input time is recorded data, not an effect.

---

## 13. Budget model

Each agent defines `budget per_event` with these dimensions:

```text
model_calls
model_tokens
external_reads
external_writes
approvals
money_microunits
```

All are non-negative integers. Unknown dimensions are compile errors. Duplicate dimensions are compile errors. Missing dimensions default to zero; document this clearly.

Runtime behavior:

- count-based resources are reserved before the effect;
- variable resources such as tokens and money require the fixture entry to declare a deterministic maximum reservation;
- reject the effect before driver invocation if reservation would exceed the remaining budget;
- settle actual usage after the response;
- reject a driver response whose actual usage exceeds its declared maximum;
- release unused reservation;
- write budget reservation occurs before any commit driver call;
- trace budget-before, reserved, actual, released, and budget-after;
- replay uses recorded usage and independently recomputes the same budget transitions.

A budget failure must prove by a test that the external driver call count remains unchanged.

No ambient wall-clock duration budget is required in 0.1. Keep `elapsed` out of the implemented syntax rather than implementing it nondeterministically.

---

## 14. Secret and taint behavior

`Untrusted<T>`:

- may be inspected as data where the underlying type supports it;
- may be passed to prompt `data`;
- cannot enter prompt instructions because instructions are not expressions;
- carries provenance.

`Incoming<T>` exposes `.value` and provenance.

`Secret<T>`:

- is an opaque runtime handle;
- is allowed only as a parameter type for a tool explicitly declared with `sensitivity secret`;
- cannot be placed in records that enter persistent state;
- cannot be passed to prompts;
- cannot be rendered, compared, hashed from source, serialized into trace/snapshot/state, or included in diagnostics;
- may be represented internally only by an opaque, non-serializable runtime handle.

Do not implement real secret retrieval or a source-level secret constructor in 0.1. It is sufficient to implement the type, declaration restrictions, leakage checks, and a runtime opaque value used only by focused tests. A live secret must never cross a trace or snapshot boundary; attempting to snapshot such a value must fail with a controlled typed error before serialization.

Private, non-secret fixture values may be stored in the explicit replay trace. Console output and diagnostics must use redacted summaries. Create trace/snapshot files with restrictive permissions where the platform supports it, and document that traces can contain sensitive non-secret data.

---

## 15. Static semantics and diagnostics

Implement distinct passes:

1. parse;
2. declaration collection;
3. name resolution;
4. type checking;
5. purity/effect inference;
6. capability checking;
7. affine-use analysis;
8. recursion/termination restriction;
9. state/persistence restriction;
10. lowering to typed IR.

Do not collapse all failures into strings. Use a structured diagnostic type shared through `aster-diagnostics`:

```json
{
  "code": "ASTER-TYPE-2001",
  "severity": "error",
  "message": "candidate data must be validated before use",
  "primary_span": {
    "file": "example.aster",
    "start": 120,
    "end": 129,
    "line": 8,
    "column": 14
  },
  "labels": [],
  "notes": [],
  "help": "use `validate candidate with <Validator>` to obtain `Checked<T>`"
}
```

Human diagnostics must include file, line, column, source excerpt, labels, notes, and actionable help where possible. JSON diagnostics must be stable and deterministic.

Reserve and document code families:

```text
ASTER-PARSE-0xxx
ASTER-NAME-1xxx
ASTER-TYPE-2xxx
ASTER-EFFECT-3xxx
ASTER-POLICY-4xxx
ASTER-AFFINE-5xxx
ASTER-CAP-6xxx
ASTER-PROMPT-7xxx
ASTER-SECRET-8xxx
ASTER-RUNTIME-9xxx
ASTER-REPLAY-10xxx
ASTER-BUDGET-11xxx
ASTER-INTERNAL-99xx
```

Mandatory diagnostics include stable codes for:

- parse error;
- duplicate declaration;
- unknown name;
- type mismatch;
- candidate value extraction/direct use;
- invalid prompt instruction;
- effect in pure context;
- non-total policy;
- read tool proposed;
- write tool observed/directly called;
- commit without matching permit;
- use after move of proposal or permit;
- missing capability declaration;
- invalid capability grant at runtime;
- secret leakage;
- recursion;
- budget exhaustion;
- replay divergence;
- trace schema/program-hash mismatch.

Choose exact numeric suffixes and lock them in tests and `docs/design-docs/diagnostics.md`.

Internal compiler/runtime invariant failures must become `ASTER-INTERNAL-*` diagnostics with context, not user-facing panics.

---

## 16. Typed IR and explicit machine

Do not let AST nodes directly invoke drivers.

Lower checked source into a typed, serializable IR with explicit control flow and effect suspension points. The representation may be bytecode, basic blocks, or an instruction graph, but it must provide:

- stable instruction identities within a compiled program;
- explicit locals and value slots;
- explicit branches and returns;
- explicit state reads and pending atomic state updates;
- explicit instructions for:
  - inference request;
  - observation request;
  - validation;
  - intent construction;
  - proposal construction;
  - authorization;
  - human approval suspension;
  - commit request;
  - reconciliation;
  - budget reservation/settlement or equivalent runtime transitions;
- a serializable instruction pointer and frame stack;
- no host-language closure captured in snapshots.

The deterministic VM API should conceptually support:

```rust
enum Step {
    Continue,
    Yield(EffectRequest),
    Completed(RunOutcome),
    Failed(RuntimeDiagnostic),
}
```

The driver resolves only yielded external requests. Pure VM stepping performs no I/O.

A `MachineSnapshot` must include enough versioned data to resume after an effect boundary:

- schema version;
- compiler/runtime version;
- normalized program hash;
- agent and handler identity;
- event identity and input hash;
- instruction pointer and frames;
- locals;
- immutable current state;
- pending state delta;
- remaining/reserved budget;
- capability grant fingerprint;
- affine-resource ledger;
- trace position/hash;
- pending effect request, when applicable.

Serialize and deserialize snapshots deterministically. Add round-trip tests. Implement a CLI resume path or a runtime API exercised by tests that restores a snapshot, supplies the matching effect result, and reaches the same final outcome. A public `aster resume` command is preferred and required unless a clearly documented technical constraint makes the tested runtime API materially safer.

---

## 17. Runtime driver and fixture format

Define one narrow runtime interface for external effects. No other layer may perform them.

A generic shape is acceptable:

```rust
trait EffectDriver {
    fn resolve(&mut self, request: &EffectRequest)
        -> Result<EffectResolution, DriverError>;
}
```

The production implementation for 0.1 is a deterministic fixture driver loaded from `fixtures.json`.

Fixture matching must use:

- effect kind;
- declaration/model/policy identity;
- canonical request hash or explicitly declared match fields;
- queue position only as a secondary disambiguator.

Do not silently select the first loosely matching fixture.

Each fixture response declares:

- typed response payload;
- `max_usage`;
- `actual_usage`;
- optional expected request summary.

The fixture driver tracks call counts by effect kind so tests can prove:

- budget rejection invokes no driver;
- replay invokes no driver;
- direct policy allow invokes no approval driver;
- approval path invokes exactly one approval driver;
- one commit invokes exactly one write driver.

Fixture files contain synthetic data only.

---

## 18. Trace, hashing, recording, replay, and tamper detection

Use an append-only JSON Lines trace with an explicit schema version and a hash chain.

Every entry contains at least:

```text
schema_version
run_id
sequence
kind
payload
previous_entry_hash
entry_hash
```

Use a documented canonical JSON encoding before SHA-256 hashing. Object keys must be sorted recursively, numeric representation must be stable, and no map iteration order may leak into hashes. Record the canonicalization decision in ADR 0003.

Required logical entries include:

- run header;
- event received;
- program/capability/state fingerprints;
- effect requested;
- budget reserved;
- effect resolved;
- budget settled;
- policy decision;
- permit issued;
- proposal committed;
- reconciliation decision;
- state committed;
- run completed or failed;
- snapshot written.

Do not include actual `Secret<T>` material.

`aster run` record mode:

1. validates source, input, state, capabilities, and fixtures;
2. creates a run header;
3. steps the VM;
4. before each driver call, appends the request and reservation;
5. resolves the effect;
6. validates the response;
7. appends resolution and settlement;
8. resumes the VM;
9. writes snapshots atomically;
10. commits final state atomically only on successful handler completion;
11. appends completion.

`aster replay`:

1. verifies the entire trace hash chain and schema;
2. verifies normalized program hash;
3. verifies event/input and initial-state fingerprints;
4. initializes a replay VM;
5. steps until the VM yields a request;
6. compares the complete canonical request identity with the next recorded request;
7. injects the recorded resolution without constructing or invoking a driver;
8. independently recomputes budget transitions, policy decisions, proposal hashes, permit binding, reconciliation, and final state;
9. fails immediately on any mismatch;
10. verifies the final state and outcome hash.

Replay must not merely print recorded output. It must re-execute deterministic semantics and verify the effect sequence.

Tampering with any entry, changing source semantics, changing event input, changing initial state, skipping/reordering an effect, or changing a proposal argument must fail with a stable replay diagnostic.

---

## 19. CLI

Create a binary named `aster`.

Required commands:

```text
aster check <SOURCE> [--diagnostic-format human|json]
aster fmt <SOURCE> [--check] [--write]
aster ast <SOURCE> --json
aster run <SOURCE>
  --agent <NAME>
  --event <NAME>
  --input <FILE>
  --state <FILE>
  --capabilities <FILE>
  --fixtures <FILE>
  --trace <FILE>
  --snapshot-dir <DIR>
  --output-state <FILE>
  [--diagnostic-format human|json]
aster replay <SOURCE>
  --trace <FILE>
  --input <FILE>
  --state <FILE>
  --capabilities <FILE>
  --output-state <FILE>
  [--diagnostic-format human|json]
aster resume <SOURCE>
  --snapshot <FILE>
  --resolution <FILE>
  --trace <FILE>
  --snapshot-dir <DIR>
  --output-state <FILE>
aster explain <DIAGNOSTIC_CODE>
```

If `resume` is implemented through a different but coherent interface, update this list, docs, tests, and `AGENTS.md`; do not omit durable-resume validation.

Exit codes:

- `0`: success;
- `1`: source/specification/check/format error;
- `2`: runtime, fixture, capability, budget, or policy failure;
- `3`: replay divergence/tamper/schema mismatch;
- `4`: internal invariant failure.

CLI output must be deterministic. Human-readable status goes to stderr; requested machine output goes to stdout or explicit files. Do not print private payloads by default.

`aster fmt --check` must be nonzero when formatting differs. `aster fmt --write` must use atomic replacement.

`aster explain` reads the checked-in diagnostics reference or generated registry and prints code, meaning, cause, and remediation.

---

## 20. Canonical formatting

Define one canonical source representation.

Requirements:

- deterministic;
- idempotent;
- preserves comments;
- does not change program meaning;
- emits a trailing newline;
- normalizes indentation, spaces, commas, semicolons, and block layout;
- keeps static prompt block-string contents semantically unchanged;
- returns a diagnostic instead of dropping malformed regions.

Required tests:

- format twice is byte-identical;
- parse-format-parse preserves normalized AST;
- comments survive;
- all bundled pass fixtures are already formatted;
- `fmt --check` succeeds on the repository examples.

Do not introduce multiple equivalent surface syntaxes merely to make parsing easier.

---

## 21. Bundled meeting-scheduler example

Implement a complete example close to the following. You may make small syntax corrections, but preserve every semantic stage and update the specification to match exactly.

```aster
module meeting.scheduler;

type UserId = Text;
type Email = Text;

type UserMessage = {
  text: Untrusted<Text>
};

type MeetingRequest = {
  title: Text,
  attendees: List<Email>,
  duration_minutes: Int
};

type Slot = {
  id: Text
};

type EventRef = {
  id: Text
};

type EventExpectation = {
  owner: UserId,
  slot_id: Text
};

type UserProfile = {
  known_attendees: List<Email>
};

capability ModelUse(alias: Text);
capability CalendarRead(owner: UserId);
capability CalendarWrite(owner: UserId);
capability HumanApproval(owner: UserId);

prompt ParseMeeting(
  message: Untrusted<Text>
) -> MeetingRequest {
  instruction """
  Extract only meeting information explicitly present in the message.
  Do not invent attendees, addresses, dates, or times.
  """;

  data {
    message
  };
}

validator MeetingRules(x: MeetingRequest) {
  require x.duration_minutes >= 15;
  require x.duration_minutes <= 120;
  require len(x.attendees) > 0;
}

fn all_known(
  attendees: List<Email>,
  known: List<Email>
) -> Bool {
  return subset(attendees, known);
}

tool Calendar.free(
  owner: UserId,
  duration_minutes: Int
) -> List<Slot> {
  mode read;
  capability CalendarRead(owner);
  sensitivity private;
}

tool Calendar.create(
  owner: UserId,
  slot: Slot,
  title: Text,
  attendees: List<Email>,
  request_id: Text
) -> EventRef {
  mode write;
  capability CalendarWrite(owner);
  risk reversible;
  idempotency request_id;
  sensitivity private;
}

tool Calendar.lookup(
  owner: UserId,
  event_id: Text
) -> EventRef {
  mode read;
  capability CalendarRead(owner);
  sensitivity private;
}

validator EventMatches(
  expected: EventRef,
  actual: EventRef
) {
  require expected.id == actual.id;
}

policy CalendarPolicy(
  p: Proposal<Calendar.create>,
  s: Scheduler.State
) {
  allow when all_known(
    p.args.attendees,
    s.profile.known_attendees
  );

  approve by Human(s.user) when true;

  deny "No policy rule allowed the calendar write" otherwise;
}

agent Scheduler(user: UserId)
requires [
  ModelUse("planner"),
  CalendarRead(user),
  CalendarWrite(user),
  HumanApproval(user)
] {
  state {
    profile: UserProfile = UserProfile {
      known_attendees = []
    };

    last_event: Option<EventRef> = None;
  }

  budget per_event {
    model_calls <= 2;
    model_tokens <= 4000;
    external_reads <= 3;
    external_writes <= 1;
    approvals <= 1;
    money_microunits <= 10000;
  }

  on message(
    msg: Incoming<UserMessage>
  ) -> Result<Unit, Error> {
    let candidate = (
      infer ParseMeeting(
        message = msg.value.text
      ) using @planner
    )?;

    let request = (
      validate candidate with MeetingRules
    )?;

    let slots = (
      observe Calendar.free(
        owner = user,
        duration_minutes = request.value.duration_minutes
      )
    )?;

    require len(slots.value) > 0;

    let selected = first(slots.value)?;

    let purpose = intent ScheduleMeeting {
      actor = self;
      beneficiary = user;
      basis = [
        provenance(msg),
        provenance(request),
        provenance(slots)
      ];
      expected = EventExpectation {
        owner = user,
        slot_id = selected.id
      };
      expires_at = add_seconds(event.time, 120);
    };

    let proposal = propose Calendar.create(
      owner = user,
      slot = selected,
      title = request.value.title,
      attendees = request.value.attendees,
      request_id = event.id
    ) for purpose;

    let permit = (
      authorize proposal using CalendarPolicy
    )?;

    let receipt = (
      commit proposal with permit
    )?;

    let actual = (
      observe Calendar.lookup(
        owner = user,
        event_id = receipt.value.id
      )
    )?;

    let confirmed = (
      reconcile receipt against actual with EventMatches
    )?;

    update state {
      last_event = Some(confirmed.value);
    }

    return Ok(Unit);
  }
}
```

The example fixture set must exercise the human-approval path by using an attendee absent from `known_attendees`.

A second fixture or test must exercise direct policy allow with only known attendees.

The example’s record-mode run and replay-mode run must produce byte-identical canonical final-state JSON.

Document exact commands in `README.md`.

---

## 22. Input, state, capability, fixture, and output schemas

Define versioned JSON schemas in the specification and validate them in code.

Minimum event input:

```json
{
  "schema_version": 1,
  "event_id": "evt-001",
  "event_time": "2026-08-05T12:00:00Z",
  "agent_arguments": {
    "user": "user-001"
  },
  "payload": {
    "text": "Schedule a 30 minute meeting with new.person@example.test"
  }
}
```

Use a deterministic RFC 3339 UTC parser for `Instant`. Normalize to a single canonical representation.

The runtime decodes the event payload against the selected handler's `Incoming<T>` parameter. Only the runtime may construct `Incoming<T>` and `Untrusted<T>` at this external boundary; JSON cannot construct privileged wrappers such as `Checked`, `Proposal`, `Permit`, `Receipt`, `Reconciled`, capabilities, or secrets.

Minimum initial state:

```json
{
  "schema_version": 1,
  "state": {
    "profile": {
      "known_attendees": []
    },
    "last_event": null
  }
}
```

The initial-state file is validated against the agent state schema. Values present in the file replace source defaults; omitted fields use source defaults only when the specification explicitly permits omission. Reject unknown fields and invalid wrapper values.

Minimum capability grants:

```json
{
  "schema_version": 1,
  "grants": [
    {
      "capability": "ModelUse",
      "arguments": ["planner"]
    },
    {
      "capability": "CalendarRead",
      "arguments": ["user-001"]
    },
    {
      "capability": "CalendarWrite",
      "arguments": ["user-001"]
    },
    {
      "capability": "HumanApproval",
      "arguments": ["user-001"]
    }
  ]
}
```

Design and document the fixture schema. It must provide deterministic matches and usage reservations/results for:

1. `ParseMeeting` using `planner`;
2. `Calendar.free`;
3. human approval for the proposal hash or a safe match template resolved to that hash;
4. `Calendar.create`;
5. `Calendar.lookup`.

Do not hard-code the example into the runtime. The fixture driver must operate through general declarations and typed request/response schemas.

Final state uses canonical JSON and includes `schema_version`.

---

## 23. Mandatory conformance fixtures

Create compile-pass and compile-fail fixtures. Compile-fail tests must assert stable diagnostic code and relevant span, not only substring output.

At minimum include these failures:

### 23.1 Candidate used without validation

```aster
let candidate = (infer ParseMeeting(message = msg.value.text) using @planner)?;
let title = candidate.value.title;
```

Must fail because `Candidate<T>` has no `.value`.

### 23.2 Candidate passed to a write

A candidate or hidden candidate payload must not satisfy a normal argument type.

### 23.3 Write tool observed

```aster
let x = observe Calendar.create(...);
```

Must fail.

### 23.4 Read tool proposed

```aster
let p = propose Calendar.free(...) for purpose;
```

Must fail.

### 23.5 Direct tool call

```aster
let x = Calendar.free(...);
```

Must fail.

### 23.6 Commit without permit

```aster
let receipt = commit proposal;
```

Must fail parse or type check with a specific diagnostic.

### 23.7 Permit/action mismatch

Permit for one action cannot commit another action.

### 23.8 Permit reused

```aster
let first = (commit proposal with permit)?;
let second = (commit proposal with permit)?;
```

Must fail affine-use analysis.

### 23.9 Proposal reused after commit

Must fail affine-use analysis.

### 23.10 Effect in policy

A policy that calls `infer`, `observe`, or an effectful flow must fail purity checking.

### 23.11 Non-total policy

A policy without final `otherwise` must fail.

### 23.12 Missing capability

An agent that observes a tool without the required `requires` capability must fail.

### 23.13 Dynamic prompt instruction

Any syntax attempting interpolation or an expression in `instruction` must fail.

### 23.14 Secret to model

Passing `Secret<Text>` to prompt data must fail.

### 23.15 Secret in persistent state

Must fail.

### 23.16 Recursive function or flow

Direct and mutual recursion must fail.

### 23.17 Unknown/duplicate budget dimension

Must fail.

### 23.18 Write tool without idempotency metadata

Must fail declaration checking.

Pass fixtures must cover valid variants of each safe path, including direct allow and human approval.

---

## 24. Mandatory runtime and replay tests

Create automated tests for all of these:

1. The meeting example checks successfully.
2. Formatter is idempotent and preserves comments.
3. Fixture-backed record run succeeds.
4. Record run invokes expected driver counts:
   - one model;
   - two reads;
   - one approval in approval fixture;
   - one write.
5. Direct-allow fixture invokes zero approval calls.
6. Final state contains the reconciled event.
7. Replay produces byte-identical final state.
8. Replay constructs no driver and records zero driver calls.
9. Tampering with an effect result breaks the hash chain or semantic verification.
10. Recomputing the hash chain after maliciously changing a result still fails semantic replay if the result changes a downstream request or final state.
11. Changing source after recording fails program-hash verification.
12. Changing input or initial state fails fingerprint verification.
13. Reordering two effect entries fails.
14. A proposal hash changes when any argument, intent field, action, capability request, or program hash changes.
15. A permit for proposal A is rejected for proposal B even when both use the same tool.
16. Double commit is rejected dynamically in a low-level runtime test.
17. Expired intent/permit is rejected before write-driver invocation.
18. Missing runtime capability is rejected before driver invocation.
19. Model-call budget exhaustion is rejected before model-driver invocation.
20. Write budget exhaustion is rejected before write-driver invocation.
21. Driver actual usage above declared maximum is rejected and traced.
22. Model output schema mismatch returns a typed error without panic.
23. Tool output schema mismatch returns a typed error without panic.
24. Snapshot serialization round-trips.
25. Resuming from a snapshot and supplying the recorded resolution reaches the same final state.
26. Failed handlers do not publish partial state updates.
27. Secret material and a unique secret test sentinel do not occur in human diagnostics, console output, trace payloads, snapshots, or final state.
28. Malformed source, JSON, trace, and snapshot inputs never panic.
29. Diagnostic JSON ordering and canonical trace serialization are deterministic.
30. CLI exit codes match the specification.

Use synthetic values only.

---

## 25. Documentation requirements

### `README.md`

Include:

- what ASTER is;
- the four-stage core boundary;
- current 0.1 scope and non-goals;
- workspace prerequisites;
- build/test commands;
- exact meeting-example `check`, `run`, and `replay` commands;
- a short source walkthrough;
- security warning that fixture traces may contain private non-secret data;
- project status: experimental, not production-ready.

### `ARCHITECTURE.md`

Give a navigable map of:

- crates and dependency direction;
- compilation pipeline;
- typed wrappers;
- explicit IR;
- runtime/effect driver boundary;
- record/replay flow;
- capability and budget checks;
- state transaction;
- where each invariant is enforced;
- pointers to design docs and tests.

### `docs/spec/aster-0.1.md`

This is normative. Specify:

- lexical grammar;
- concrete grammar or precise EBNF;
- declarations;
- types;
- expression typing;
- wrapper visibility;
- effect typing;
- capability matching;
- affine semantics;
- policy evaluation;
- budgets;
- runtime state transitions;
- trace and snapshot schemas;
- errors and exit codes;
- canonical formatting;
- explicit non-goals.

Mark implementation-defined behavior only where unavoidable. Prefer to decide behavior.

### Design docs

- `core-beliefs.md`: why the LLM is a candidate oracle, not authority.
- `runtime-and-replay.md`: VM, effect yielding, snapshots, trace verification.
- `security-model.md`: assets, trust boundaries, threats, enforced mitigations, residual risks.
- `diagnostics.md`: complete stable-code registry and remediation guidance.

### ADRs

Write the three ADRs listed in the repository layout, with status, context, decision, consequences, and alternatives.

### Execution plan

Keep it accurate through implementation. Do not retroactively pretend deviations did not happen.

---

## 26. Mechanical repository checks

Implement:

### `scripts/check.sh`

Runs all required checks and stops on failure:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
./scripts/check-architecture.sh
./scripts/check-docs.sh
```

It may run additional example/conformance commands.

### `scripts/check-architecture.sh`

Mechanically verifies forbidden crate dependency directions, preferably using `cargo metadata` and a small checked-in script or Rust utility. It must fail with actionable output.

### `scripts/check-docs.sh`

At minimum verifies:

- required documents exist;
- required links/paths from `AGENTS.md`, `README.md`, and `ARCHITECTURE.md` resolve;
- every registered diagnostic code appears in the diagnostics reference;
- every command named in `AGENTS.md` exists;
- no active bootstrap plan remains after completion;
- examples are canonically formatted.

Avoid brittle checks that merely grep arbitrary prose without explaining failures.

### CI

Create a GitHub Actions workflow that installs stable Rust, caches safely, and runs `./scripts/check.sh`. Do not require secrets or network services beyond normal dependency download.

---

## 27. Quality bar

The implementation must exhibit these properties:

- no unsafe Rust;
- no reachable `todo!`, `unimplemented!`, placeholder success, or deliberate panic in production paths;
- typed errors;
- deterministic tests;
- stable diagnostics;
- no flaky wall-clock sleeps;
- no dependence on current locale or timezone;
- no dependence on map iteration order;
- no duplicated semantic checks in the CLI;
- no external I/O from compiler or VM core;
- atomic output file replacement;
- no unbounded source constructs;
- no real secrets or endpoints;
- module and public-item documentation where invariants matter;
- focused files and modules rather than monolithic “everything” files;
- comments explain semantic reasons, not obvious syntax;
- tests verify behavior rather than implementation trivia.

When an invariant can be mechanically checked, prefer a lint, type rule, structural test, or runtime assertion over prose.

---

## 28. Scope boundaries and explicit non-goals

Do not implement these in ASTER 0.1:

- real OpenAI or other model-provider API calls;
- live MCP/OpenAPI integration;
- arbitrary network or shell tools;
- WASM or native FFI;
- packages/import resolution;
- multi-agent spawning;
- vector memory;
- scheduled heartbeat events;
- sagas/automatic compensation;
- distributed execution;
- self-modifying code;
- dynamic prompt instruction promotion;
- user-defined generics;
- concurrency;
- loops or recursion;
- garbage collection beyond ordinary Rust ownership;
- theorem proving;
- production cryptographic key management;
- trace encryption;
- backward compatibility with an earlier ASTER version.

Design extension points only where they make the 0.1 implementation clearer. Do not build unused abstraction forests.

---

## 29. Priority order when trade-offs arise

Do not drop required items casually. When a conflict is unavoidable, use this order:

1. candidate/action/authority separation;
2. write-action transaction and permit binding;
3. deterministic explicit VM;
4. record/replay with zero replay driver calls;
5. capabilities and pre-effect budgets;
6. secret and prompt-instruction boundaries;
7. stable static diagnostics and affine checks;
8. state atomicity and snapshots;
9. CLI ergonomics;
10. formatter polish.

Record any unavoidable limitation in the execution plan, specification, README status section, and final report. A limitation must not be disguised as completed behavior.

---

## 30. Final validation and response

Before finishing:

1. Run `git diff --check`.
2. Run `./scripts/check.sh`.
3. Run the documented meeting example in record mode.
4. Run the documented replay command.
5. Compare record and replay final-state files byte for byte.
6. Run at least one representative compile-fail fixture in JSON diagnostic mode.
7. Inspect Git status.
8. Review trace/snapshot/output files to ensure they are ignored and contain no secret test sentinel.
9. Review the final diff for stale docs and accidental scope expansion.

In the final response, report:

- concise implementation summary;
- architecture and semantic decisions;
- exact commands run and their results;
- record/replay demonstration result;
- notable files;
- known limitations and residual risks;
- Git status/commit status as required by the harness.

Do not claim “all tests pass” unless the full `./scripts/check.sh` completed successfully after the final edit.

Begin by reading `AGENTS.md` and creating the execution plan. Then implement the repository end to end.
