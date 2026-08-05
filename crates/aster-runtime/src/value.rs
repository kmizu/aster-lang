use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

use crate::{CanonicalError, Intent, Permit, Proposal};

/// Typed successful write response retained for reconciliation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptValue {
    /// Committed action identity.
    pub action: String,
    /// Bound proposal hash.
    pub proposal_hash: String,
    /// Decoded write result.
    pub value: Box<RuntimeValue>,
}

/// A typed value paired with a stable, non-secret boundary identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenancedValue {
    /// Hidden or wrapper-visible typed payload.
    pub value: Box<RuntimeValue>,
    /// Stable reference derived from the event or effect request.
    pub provenance: String,
}

/// Serializable runtime values plus an opaque non-serializable secret handle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    Text(String),
    List(Vec<Self>),
    Record(BTreeMap<String, Self>),
    Enum {
        variant: String,
        payload: Option<Box<Self>>,
    },
    Option(Option<Box<Self>>),
    Result(Result<Box<Self>, String>),
    Incoming(ProvenancedValue),
    Untrusted(ProvenancedValue),
    Candidate(ProvenancedValue),
    Checked(ProvenancedValue),
    Observation(ProvenancedValue),
    Intent(Intent),
    Proposal(Proposal),
    Permit(Permit),
    Receipt(ReceiptValue),
    Reconciled(ReceiptValue),
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
            Self::Enum {
                payload: Some(value),
                ..
            }
            | Self::Option(Some(value))
            | Self::Result(Ok(value))
            | Self::Incoming(ProvenancedValue { value, .. })
            | Self::Untrusted(ProvenancedValue { value, .. })
            | Self::Candidate(ProvenancedValue { value, .. })
            | Self::Checked(ProvenancedValue { value, .. })
            | Self::Observation(ProvenancedValue { value, .. }) => value.contains_secret(),
            Self::Receipt(receipt) | Self::Reconciled(receipt) => receipt.value.contains_secret(),
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Text(_)
            | Self::Enum { payload: None, .. }
            | Self::Option(None)
            | Self::Result(Err(_))
            | Self::Intent(_)
            | Self::Proposal(_)
            | Self::Permit(_) => false,
        }
    }

    pub(crate) fn validate_governance(&self) -> Result<(), CanonicalError> {
        match self {
            Self::Proposal(proposal) => proposal.validate(),
            Self::List(values) => values.iter().try_for_each(Self::validate_governance),
            Self::Record(values) => values.values().try_for_each(Self::validate_governance),
            Self::Enum {
                payload: Some(value),
                ..
            }
            | Self::Option(Some(value))
            | Self::Result(Ok(value))
            | Self::Incoming(ProvenancedValue { value, .. })
            | Self::Untrusted(ProvenancedValue { value, .. })
            | Self::Candidate(ProvenancedValue { value, .. })
            | Self::Checked(ProvenancedValue { value, .. })
            | Self::Observation(ProvenancedValue { value, .. }) => value.validate_governance(),
            Self::Receipt(receipt) | Self::Reconciled(receipt) => {
                receipt.value.validate_governance()
            }
            Self::Unit
            | Self::Bool(_)
            | Self::Int(_)
            | Self::Text(_)
            | Self::Enum { payload: None, .. }
            | Self::Option(None)
            | Self::Result(Err(_))
            | Self::Intent(_)
            | Self::Permit(_)
            | Self::Secret(_) => Ok(()),
        }
    }
}

/// Opaque secret identity. Its inner material is intentionally private.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretHandle(String);

impl Serialize for SecretHandle {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(
            "opaque secret handles cannot be serialized",
        ))
    }
}

impl<'de> Deserialize<'de> for SecretHandle {
    fn deserialize<D>(_: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(
            "opaque secret handles cannot be deserialized",
        ))
    }
}

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
