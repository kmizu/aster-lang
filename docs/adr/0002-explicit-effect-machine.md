# ADR 0002: Explicit Effect Machine

Status: accepted

## Context

Record/replay and durable resume require external effects and continuations to
be inspectable and serializable.

## Decision

Lower checked effectful source to a typed instruction machine. Pure stepping
never performs I/O; effect instructions yield typed requests resolved only by
the runtime driver boundary. Frames, locals, instruction pointers, state deltas,
budgets, and affine ledgers are serializable domain data.

## Consequences

Snapshots can resume without host closures and replay can compare requests
before injecting results. Lowering and VM code are more explicit than a direct
recursive AST interpreter.

## Alternatives

A recursive effectful interpreter was rejected because control state and driver
calls would be hidden in host stack frames. Async host futures were rejected for
the same reason and because ASTER 0.1 has no concurrency.
