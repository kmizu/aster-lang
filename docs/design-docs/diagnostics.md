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

### ASTER-TYPE-2001 — candidate used before validation

Meaning: candidate data was projected or passed as ordinary data. Cause:
`Candidate<T>` intentionally has no value projection. Remediation: use
`validate candidate with <Validator>` to obtain `Checked<T>`.

Additional required codes are added here in the same patch as their semantic or
runtime rule and conformance test.
