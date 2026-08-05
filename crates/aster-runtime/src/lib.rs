#![forbid(unsafe_code)]

//! Deterministic ASTER VM, authority ledger, traces, snapshots, and replay.

mod authority;
mod budget;
mod canonical;
mod trace;
mod value;

pub use authority::{AuthorityError, AuthorityLedger, Intent, Permit, Proposal};
pub use budget::{Budget, BudgetDimension, BudgetError, Reservation};
pub use canonical::{CanonicalError, canonical_json, canonical_sha256};
pub use trace::{Trace, TraceEntry, TraceError};
pub use value::{RuntimeValue, SnapshotError, snapshot_values};
