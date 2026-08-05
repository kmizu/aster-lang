use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Serializable runtime values plus an opaque non-serializable secret handle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    Text(String),
    List(Vec<Self>),
    Record(BTreeMap<String, Self>),
    Option(Option<Box<Self>>),
    Result(Result<Box<Self>, String>),
    Incoming(Box<Self>),
    Untrusted(Box<Self>),
    Candidate(Box<Self>),
    Checked(Box<Self>),
    Observation(Box<Self>),
    Secret(SecretHandle),
}

impl RuntimeValue {
    /// Constructs an opaque secret handle for leakage-focused runtime tests.
    #[must_use]
    pub fn secret_for_test(sentinel: impl Into<String>) -> Self {
        Self::Secret(SecretHandle(sentinel.into()))
    }

    fn contains_secret(&self) -> bool {
        match self {
            Self::Secret(_) => true,
            Self::List(values) => values.iter().any(Self::contains_secret),
            Self::Record(values) => values.values().any(Self::contains_secret),
            Self::Option(Some(value))
            | Self::Result(Ok(value))
            | Self::Incoming(value)
            | Self::Untrusted(value)
            | Self::Candidate(value)
            | Self::Checked(value)
            | Self::Observation(value) => value.contains_secret(),
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Text(_)
            | Self::Option(None)
            | Self::Result(Err(_)) => false,
        }
    }
}

/// Opaque secret identity. Its inner material is intentionally private.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecretHandle(String);

impl std::fmt::Debug for SecretHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretHandle(<redacted>)")
    }
}

/// Rejects secrets before serializing snapshot values.
///
/// # Errors
///
/// Returns a redacted error when any transitive value is secret.
pub fn snapshot_values(values: &BTreeMap<String, RuntimeValue>) -> Result<String, SnapshotError> {
    if values.values().any(RuntimeValue::contains_secret) {
        return Err(SnapshotError::SecretPresent);
    }
    serde_json::to_string(values).map_err(SnapshotError::Serialization)
}

/// Controlled snapshot boundary failure.
#[derive(Debug, Error)]
pub enum SnapshotError {
    /// Secret material cannot cross persistence boundaries.
    #[error("snapshot contains an opaque secret handle")]
    SecretPresent,
    /// Non-secret serialization failed.
    #[error("snapshot serialization failed: {0}")]
    Serialization(serde_json::Error),
}

impl PartialEq for SnapshotError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SecretPresent, Self::SecretPresent)
                | (Self::Serialization(_), Self::Serialization(_))
        )
    }
}

impl Eq for SnapshotError {}
