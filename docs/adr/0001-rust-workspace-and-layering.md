# ADR 0001: Rust Workspace and Layering

Status: accepted

## Context

ASTER needs stable builds, explicit ownership, typed boundaries, and mechanical
prevention of compiler/runtime dependency cycles.

## Decision

Use stable Rust with six crates ordered diagnostics, syntax, semantics, IR,
runtime, and CLI. Every crate forbids unsafe code. Cargo metadata is checked by
`scripts/check-architecture.sh` so dependencies may point only to lower layers.

## Consequences

Semantic ownership remains visible and lower layers cannot call runtime I/O.
Some domain values must be translated at crate boundaries instead of shared
through a convenience utility crate.

## Alternatives

A monolithic crate was rejected because its module boundaries would be easier
to bypass. A larger plugin architecture was rejected as unused 0.1 complexity.
