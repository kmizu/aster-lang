#![forbid(unsafe_code)]

//! Deterministic ASTER VM, authority ledger, traces, snapshots, and replay.

mod authority;
mod budget;
mod canonical;
mod capability;
mod fixture;
mod host;
mod machine;
mod run;
mod trace;
mod value;

pub use authority::{AuthorityError, AuthorityLedger, Intent, Permit, Proposal, ProposalMetadata};
pub use budget::{Budget, BudgetDimension, BudgetError, Reservation};
pub use canonical::{CanonicalError, canonical_json, canonical_sha256};
pub use capability::{CapabilityError, CapabilityGrant, CapabilityGrants};
pub use fixture::{
    DriverError, EffectDriver, FixtureDriver, FixtureEntry, FixturePreview, FixtureSet,
};
pub use host::{
    EffectAdmission, EffectPreview, ExecutionGrant, HOST_PROTOCOL_MAX_LINE_BYTES,
    HOST_PROTOCOL_NAME, HOST_PROTOCOL_VERSION, Hello, HelloAck, HostCompleted,
    HostEffectResolution, HostFailed, HostInboundFrame, HostInboundMessage, HostOutboundFrame,
    HostOutboundMessage, HostProtocolError, decode_host_reply,
};
pub use machine::{
    AuditEvent, EffectKind, EffectRequest, EffectResolution, Machine, MachineError,
    MachineSnapshot, RunOutcome, StartRequest, Step,
};
pub use run::{
    AdmittedEffect, RecordFailure, RecordProgress, RecordResult, RecordSession, ReplayError,
    RunError, budget_settlement_evidence, record_run, record_run_evidenced, replay_run,
};
pub use trace::{Trace, TraceEntry, TraceError};
pub use value::{ProvenancedValue, ReceiptValue, RuntimeValue, SnapshotError, snapshot_values};
