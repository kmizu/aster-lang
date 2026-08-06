# Core Beliefs

An LLM is useful precisely where outputs are nondeterministic and require
judgment. That makes it a poor security principal. ASTER therefore treats model
output as an opaque `Candidate`, never as ambient authority or an executable
command.

Validation is a visible transition from uncertain candidate data to checked
typed data. A desired write is an immutable proposal, not an effect. Authority
is a narrow, expiring, single-use permit bound to that proposal. A successful
driver response is still only a receipt; a separate observation and validator
reconcile the intended action with reality.

This separation keeps the deterministic runtime responsible for control flow,
budgets, capabilities, state, audit evidence, and replay. It also makes failures
legible to humans and future coding agents: each transition has a type, an owner,
and an inspectable representation.

An external host is an adapter, not a principal inside the language. ASTER may
show it an exact request for admission, but the host does not receive a
sequencing grant until ASTER has reserved bounded usage and durably captured
the continuation. That grant cannot mint a capability, alter a proposal, or
replace a permit. Replay needs neither the adapter nor its ambient authority.
