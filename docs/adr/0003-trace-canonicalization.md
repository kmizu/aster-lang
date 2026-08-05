# ADR 0003: Trace Canonicalization

Status: accepted

## Context

Trace integrity, replay divergence, proposal binding, and deterministic tests
need byte-stable hashes across executions.

## Decision

Serialize versioned JSON values with object keys sorted recursively, arrays kept
in semantic order, integral numbers rendered in canonical decimal form, and no
insignificant whitespace. Hash the resulting UTF-8 bytes with SHA-256. Trace
entries include the previous entry hash, and proposal hashes include schema and
program identity in addition to the complete action and intent.

## Consequences

Observable hashes do not depend on Rust map iteration. Every schema evolution
must be explicit and incompatible versions are rejected rather than guessed.

## Alternatives

Default map serialization was rejected as order-sensitive. Hashing only fixture
results was rejected because it would not bind requests, authority, or program
identity. A binary format was deferred because canonical JSON is easier to audit
for the 0.1 vertical slice.
