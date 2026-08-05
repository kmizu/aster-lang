use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{CanonicalError, canonical_sha256};

/// Immutable intent attached to one proposal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Intent {
    /// Static purpose identity.
    pub purpose: String,
    /// Canonically ordered intent fields.
    pub fields: BTreeMap<String, Value>,
    /// Normalized UTC expiry instant.
    pub expires_at: String,
}

/// Immutable, canonically hashed write proposal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Proposal {
    /// Proposal schema version.
    pub schema_version: u32,
    /// Write action identity.
    pub action: String,
    /// Complete immutable tool arguments.
    pub arguments: BTreeMap<String, Value>,
    /// Human-legible governed purpose.
    pub intent: Intent,
    /// Declared write risk.
    pub risk: String,
    /// Exact runtime capability request.
    pub capability_request: Value,
    /// Deterministic write request key.
    pub idempotency_key: String,
    /// Normalized source/IR identity.
    pub program_hash: String,
    digest: String,
}

impl Proposal {
    /// Constructs and seals a proposal over every authority-relevant field.
    ///
    /// # Errors
    ///
    /// Returns an error if canonical hashing fails.
    pub fn new(
        action: impl Into<String>,
        arguments: BTreeMap<String, Value>,
        intent: Intent,
        risk: impl Into<String>,
        capability_request: Value,
        idempotency_key: impl Into<String>,
        program_hash: impl Into<String>,
    ) -> Result<Self, CanonicalError> {
        let mut proposal = Self {
            schema_version: 1,
            action: action.into(),
            arguments,
            intent,
            risk: risk.into(),
            capability_request,
            idempotency_key: idempotency_key.into(),
            program_hash: program_hash.into(),
            digest: String::new(),
        };
        proposal.digest = canonical_sha256(&proposal)?;
        Ok(proposal)
    }

    /// Returns the immutable canonical proposal identity.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.digest
    }
}

/// Runtime-issued, proposal-bound affine authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Permit {
    id: String,
    proposal_hash: String,
    action: String,
    grant_fingerprint: String,
    policy: String,
    expires_at: String,
}

/// Runtime permit issuance and consumption state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuthorityLedger {
    next_sequence: u64,
    consumed: BTreeSet<String>,
}

impl AuthorityLedger {
    /// Issues an affine permit cryptographically bound to one proposal.
    #[must_use]
    pub fn issue(
        &mut self,
        proposal: &Proposal,
        policy: &str,
        grant_fingerprint: &str,
        expires_at: &str,
    ) -> Permit {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let material = format!(
            "{}\0{}\0{}\0{}\0{sequence}",
            proposal.hash(),
            policy,
            grant_fingerprint,
            expires_at
        );
        let id = hex::encode(sha2::Sha256::digest(material.as_bytes()));
        Permit {
            id,
            proposal_hash: proposal.hash().to_owned(),
            action: proposal.action.clone(),
            grant_fingerprint: grant_fingerprint.to_owned(),
            policy: policy.to_owned(),
            expires_at: expires_at.to_owned(),
        }
    }

    /// Atomically validates and consumes a permit before driver invocation.
    ///
    /// # Errors
    ///
    /// Rejects proposal/action/grant mismatches, expiry, and double use.
    pub fn consume(
        &mut self,
        proposal: &Proposal,
        permit: &Permit,
        grant_fingerprint: &str,
        now: &str,
    ) -> Result<(), AuthorityError> {
        if permit.proposal_hash != proposal.hash() || permit.action != proposal.action {
            return Err(AuthorityError::ProposalMismatch);
        }
        if permit.grant_fingerprint != grant_fingerprint {
            return Err(AuthorityError::GrantMismatch);
        }
        if now > permit.expires_at.as_str() || now > proposal.intent.expires_at.as_str() {
            return Err(AuthorityError::Expired);
        }
        if !self.consumed.insert(permit.id.clone()) {
            return Err(AuthorityError::AlreadyConsumed);
        }
        Ok(())
    }
}

/// Permit validation failure before a write effect.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityError {
    /// Permit does not bind this immutable proposal.
    #[error("permit does not match proposal")]
    ProposalMismatch,
    /// Runtime grants changed after authorization.
    #[error("permit capability fingerprint mismatch")]
    GrantMismatch,
    /// Intent or permit is expired.
    #[error("permit or intent expired")]
    Expired,
    /// Permit has already authorized a commit.
    #[error("permit already consumed")]
    AlreadyConsumed,
}

use sha2::Digest as _;
