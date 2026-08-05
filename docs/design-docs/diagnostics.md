# Diagnostic Registry

Diagnostic codes are stable public behavior. A code is never reused for a new
meaning. Human rendering includes a source excerpt; JSON rendering follows the
schema defined in the language specification.

## Registered codes

### ASTER-PARSE-0001 — invalid source syntax

Meaning: source does not conform to the ASTER 0.1 grammar. Cause: an unexpected,
unknown, malformed, or unterminated token. Remediation: correct the token at the
reported span.

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
