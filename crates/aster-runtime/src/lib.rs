#![forbid(unsafe_code)]

//! Deterministic ASTER VM, authority ledger, traces, snapshots, and replay.

mod authority;
mod budget;
mod canonical;
mod fixture;
mod machine;
mod run;
mod trace;
mod value;

pub use authority::{AuthorityError, AuthorityLedger, Intent, Permit, Proposal};
pub use budget::{Budget, BudgetDimension, BudgetError, Reservation};
pub use canonical::{CanonicalError, canonical_json, canonical_sha256};
pub use fixture::{
    DriverError, EffectDriver, FixtureDriver, FixtureEntry, FixturePreview, FixtureSet,
};
pub use machine::{
    EffectKind, EffectRequest, EffectResolution, Machine, MachineError, MachineSnapshot,
    RunOutcome, StartRequest, Step,
};
pub use run::{RecordResult, ReplayError, RunError, record_run, replay_run};
pub use trace::{Trace, TraceEntry, TraceError};
pub use value::{ReceiptValue, RuntimeValue, SnapshotError, snapshot_values};
