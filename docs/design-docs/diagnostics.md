# Diagnostic Registry

Diagnostic codes are stable public behavior. A code is never reused for a new
meaning. Human rendering includes a source excerpt; JSON rendering follows the
schema defined in the language specification.

## Registered codes

### ASTER-PARSE-0001 — invalid source syntax

Meaning: source does not conform to the ASTER 0.1 grammar. Cause: an unexpected,
unknown, malformed, or unterminated token. Remediation: correct the token at the
reported span.

### ASTER-PARSE-0002 — invalid string escape

Meaning: a string contains an invalid JSON-style escape. Cause: the escape is
unknown, incomplete, or has invalid Unicode digits. Remediation: use a valid
JSON escape.

### ASTER-PARSE-0003 — unknown token

Meaning: a source character has no ASTER lexical meaning. Cause: it is outside
the 0.1 token grammar. Remediation: remove it or replace it with supported
punctuation.

### ASTER-PARSE-0004 — invalid integer

Meaning: a decimal literal is outside the signed 64-bit ASTER `Int` range.
Remediation: use a representable value.

### ASTER-PARSE-0005 — unterminated block comment

Meaning: one or more nested `/*` delimiters lack a matching `*/`. Remediation:
close every nested block comment.

### ASTER-PARSE-0006 — unterminated string

Meaning: a JSON-style string lacks its closing quote. Remediation: close it on
the same source line.

### ASTER-PARSE-0007 — unterminated block string

Meaning: a triple-quoted instruction string lacks its closing delimiter.
Remediation: add the matching triple quote.

### ASTER-NAME-1001 — unknown name

Meaning: a referenced name has no declaration in its namespace. Cause: a
misspelled or absent declaration. Remediation: declare the symbol or use an
existing declared name.

### ASTER-NAME-1002 — duplicate declaration

Meaning: a declaration identity is repeated in the same module namespace.
Remediation: rename or remove the later declaration.

### ASTER-TYPE-2001 — candidate used before validation

Meaning: candidate data was projected or passed as ordinary data. Cause:
`Candidate<T>` intentionally has no value projection. Remediation: use
`validate candidate with <Validator>` to obtain `Checked<T>`.

### ASTER-TYPE-2002 — type mismatch

An expression does not match the exact type required by its call, binding,
field, operator, or return position. Change the expression or declaration.

### ASTER-TYPE-2003 — commit without permit

`commit` omitted its mandatory `with <permit>` clause. Authorize the proposal
and supply the returned permit.

### ASTER-TYPE-2004 — write tool without idempotency

A write declaration does not name a deterministic serializable idempotency
parameter. Add `idempotency <parameter>;`.

### ASTER-TYPE-2005 — permit/action mismatch

The proposal and permit action phantom types differ. Authorize and commit the
same immutable proposal.

### ASTER-EFFECT-3001 — write tool observed

`observe` targeted a write tool. Use the governed write pipeline.

### ASTER-EFFECT-3002 — read tool proposed

`propose` targeted a read tool. Use `observe`.

### ASTER-EFFECT-3003 — direct tool call

A tool declaration was called as a pure function. Select the read or write
effect syntax instead.

### ASTER-EFFECT-3004 — effect in pure context

A function, validator, or policy contains an external effect. Move it to a flow
or handler and pass the result explicitly.

### ASTER-EFFECT-3005 — recursion

A direct or mutual function/flow call cycle violates 0.1 finite-computation
rules. Rewrite it without recursion.

### ASTER-POLICY-4001 — non-total policy

The policy does not have exactly one `otherwise` rule in final position. Remove
any early `otherwise` and end with one deterministic fallback decision.

### ASTER-AFFINE-5001 — permit used after move

A commit already consumed this single-use permit. Obtain a permit for a new
proposal.

### ASTER-AFFINE-5002 — proposal used after move

A commit already consumed this proposal. Construct and authorize a new one.

### ASTER-CAP-6001 — missing capability requirement

An effect's capability kind is absent from the enclosing flow `uses` or agent
`requires` list. Declare it explicitly.

### ASTER-CAP-6002 — invalid runtime capability grant

The versioned grant set is unsupported or lacks the exact canonical typed
scope requested by the agent. Issue the exact capability and arguments.

### ASTER-PROMPT-7001 — dynamic prompt instruction

Prompt instruction syntax is not one static triple-quoted block string. Move
runtime values into the structured data block.

### ASTER-SECRET-8001 — secret to model

A `Secret` value would enter prompt data. Pass a non-secret validated summary.

### ASTER-SECRET-8002 — secret in persistent state

Persistent state transitively contains `Secret`. Keep secrets inside an opaque
sensitivity-secret tool boundary.

### ASTER-BUDGET-11001 — unknown budget dimension

The dimension is outside the six fixed 0.1 resources. Use a specified budget
name.

### ASTER-BUDGET-11002 — duplicate budget dimension

One dimension has multiple limits. Keep exactly one deterministic limit.

### ASTER-BUDGET-11003 — runtime budget exhausted

The fixed effect cost or fixture-declared maximum cannot be reserved. Increase
the per-event limit or reduce maximum usage; the driver was not invoked.

### ASTER-RUNTIME-9001 — typed runtime failure

External JSON, fixture data, authority, or a VM transition violated its typed
boundary. Correct the artifact and retry from verified state.

### ASTER-REPLAY-10001 — semantic replay divergence

A recomputed request, governance decision, budget transition, or outcome does
not match the trace. Use the exact original inputs and trace.

### ASTER-REPLAY-10002 — trace or program mismatch

The trace schema/hash chain is invalid or belongs to another program. Restore
the unmodified JSON Lines trace and matching source.

### ASTER-INTERNAL-9901 — protected invariant failure

ASTER could not represent a compiler/runtime invariant safely. Preserve the
inputs and report the deterministic failure context.
