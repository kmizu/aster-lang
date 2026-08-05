use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{CanonicalError, canonical_sha256};

/// One exact runtime-issued capability scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrant {
    /// Capability declaration identity.
    pub capability: String,
    /// Exact typed scope arguments.
    pub arguments: Vec<Value>,
}

/// Versioned runtime grants loaded only at the external boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrants {
    /// Grant file schema version.
    pub schema_version: u32,
    /// Runtime-issued exact grants.
    pub grants: Vec<CapabilityGrant>,
}

impl CapabilityGrants {
    /// Validates and produces opaque request hashes plus a set fingerprint.
    ///
    /// # Errors
    ///
    /// Rejects unsupported schemas and canonical hashing failures.
    pub(crate) fn compile(self) -> Result<CompiledGrants, CapabilityError> {
        if self.schema_version != 1 {
            return Err(CapabilityError::SchemaMismatch);
        }
        let fingerprint = canonical_sha256(&self)?;
        let request_hashes: BTreeSet<String> = self
            .grants
            .iter()
            .map(|grant| {
                canonical_sha256(&json!({
                    "capability": grant.capability,
                    "arguments": grant.arguments,
                }))
            })
            .collect::<Result<_, _>>()?;
        if request_hashes.len() != self.grants.len() {
            return Err(CapabilityError::DuplicateGrant);
        }
        Ok(CompiledGrants {
            fingerprint,
            request_hashes,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CompiledGrants {
    pub(crate) fingerprint: String,
    pub(crate) request_hashes: BTreeSet<String>,
}

impl CompiledGrants {
    pub(crate) fn permits(&self, request: &Value) -> Result<bool, CapabilityError> {
        Ok(self.request_hashes.contains(&canonical_sha256(request)?))
    }
}

/// Runtime capability file or exact-match failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CapabilityError {
    /// Unsupported grant schema.
    #[error("capability grant schema mismatch")]
    SchemaMismatch,
    /// Canonical hashing failed.
    #[error("capability canonicalization failed")]
    Canonical,
    /// The same exact grant appeared more than once.
    #[error("duplicate exact capability grant")]
    DuplicateGrant,
}

impl From<CanonicalError> for CapabilityError {
    fn from(_: CanonicalError) -> Self {
        Self::Canonical
    }
}
