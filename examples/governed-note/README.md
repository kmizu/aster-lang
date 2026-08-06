# Governed note host example

This example demonstrates the ASTER v0.2 host boundary without granting the
model ambient authority. `NoteKeeper` asks a model for typed data, validates
that data, observes a synthetic workspace, obtains human approval, commits one
reversible and idempotent write, and reconciles the result through a final
read.

For a small auditable demonstration, `NoteRules` accepts one non-empty,
statically bounded synthetic note (`ship v0.2\n`). Production programs can use
a richer domain-specific allowlist or structured bounded fields.

The qualified tool names are `Workspace.fetch`, `Workspace.store`, and
`Workspace.lookup`; their governed effect kinds are `read`, `write`, and
`read`. ASTER reserves the source words `read` and `write` for tool modes.

The checked-in JSON uses synthetic identities only. A host must map
`workspace-001` to an isolated workspace under its own control. In particular,
receiving `effect_preview` does not authorize any external action. The host may
perform the requested operation only after receiving the matching durable
`execute_grant`.

The end-to-end contract is exercised by
`cargo test -p aster-cli --test host_cli codex_style_host -- --nocapture`. The
test acts like a coding agent, maps the synthetic workspace to a temporary
`note.txt`, answers all five effects, then proves that replay needs no host and
produces byte-identical final state.
