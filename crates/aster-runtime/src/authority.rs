use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{CanonicalError, canonical_sha256};

/// Immutable intent attached to one proposal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    /// Static purpose identity.
    pub purpose: String,
    /// Canonically ordered intent fields.
    pub fields: BTreeMap<String, Value>,
    /// Normalized UTC expiry instant.
    pub expires_at: String,
}

/// Immutable tool and program metadata incorporated into a proposal hash.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProposalMetadata {
    /// Declared write risk.
    pub risk: String,
    /// Declared data sensitivity.
    pub sensitivity: String,
    /// Exact runtime capability request.
    pub capability_request: Value,
    /// Deterministic write request key.
    pub idempotency_key: String,
    /// Normalized source/IR identity.
    pub program_hash: String,
}

/// Immutable, canonically hashed write proposal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
    /// Declared data sensitivity at the write boundary.
    pub sensitivity: String,
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
        metadata: ProposalMetadata,
    ) -> Result<Self, CanonicalError> {
        let mut proposal = Self {
            schema_version: 1,
            action: action.into(),
            arguments,
            intent,
            risk: metadata.risk,
            sensitivity: metadata.sensitivity,
            capability_request: metadata.capability_request,
            idempotency_key: metadata.idempotency_key,
            program_hash: metadata.program_hash,
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

    /// Verifies that deserialized proposal content still matches its digest.
    ///
    /// # Errors
    ///
    /// Rejects modified or forged proposal content.
    pub fn validate(&self) -> Result<(), CanonicalError> {
        let mut unhashed = self.clone();
        unhashed.digest.clear();
        if canonical_sha256(&unhashed)? == self.digest {
            Ok(())
        } else {
            Err(CanonicalError::DigestMismatch)
        }
    }
}

/// Runtime-issued, proposal-bound affine authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permit {
    id: String,
    sequence: u64,
    proposal_hash: String,
    action: String,
    grant_fingerprint: String,
    policy: String,
    issued_at: String,
    expires_at: String,
    decision_evidence: String,
}

/// Runtime permit issuance and consumption state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityLedger {
    next_sequence: u64,
    issued: BTreeMap<String, Permit>,
    consumed: BTreeSet<String>,
}

impl AuthorityLedger {
    /// Issues an affine permit cryptographically bound to one sealed proposal.
    ///
    /// # Errors
    ///
    /// Rejects a proposal whose authority-relevant fields no longer match its
    /// canonical digest.
    pub fn issue(
        &mut self,
        proposal: &Proposal,
        policy: &str,
        grant_fingerprint: &str,
        issued_at: &str,
        expires_at: &str,
        decision_evidence: &str,
    ) -> Result<Permit, AuthorityError> {
        if proposal.validate().is_err() {
            return Err(AuthorityError::ProposalMismatch);
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let material = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{sequence}",
            proposal.hash(),
            policy,
            grant_fingerprint,
            issued_at,
            expires_at,
            decision_evidence,
        );
        let id = hex::encode(sha2::Sha256::digest(material.as_bytes()));
        let permit = Permit {
            id: id.clone(),
            sequence,
            proposal_hash: proposal.hash().to_owned(),
            action: proposal.action.clone(),
            grant_fingerprint: grant_fingerprint.to_owned(),
            policy: policy.to_owned(),
            issued_at: issued_at.to_owned(),
            expires_at: expires_at.to_owned(),
            decision_evidence: decision_evidence.to_owned(),
        };
        self.issued.insert(id, permit.clone());
        Ok(permit)
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
        if proposal.validate().is_err() {
            return Err(AuthorityError::ProposalMismatch);
        }
        if permit.computed_id() != permit.id || self.issued.get(&permit.id) != Some(permit) {
            return Err(AuthorityError::ForgedPermit);
        }
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

    /// Validates all deserialized permit identities and ledger relationships.
    ///
    /// # Errors
    ///
    /// Rejects forged permits, invalid sequence state, or unknown consumption.
    pub fn validate(&self) -> Result<(), AuthorityError> {
        if self.issued.iter().any(|(id, permit)| {
            id != &permit.id
                || permit.computed_id() != permit.id
                || permit.sequence >= self.next_sequence
        }) || !self.consumed.iter().all(|id| self.issued.contains_key(id))
        {
            return Err(AuthorityError::ForgedPermit);
        }
        Ok(())
    }
}

impl Permit {
    fn computed_id(&self) -> String {
        let material = format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            self.proposal_hash,
            self.policy,
            self.grant_fingerprint,
            self.issued_at,
            self.expires_at,
            self.decision_evidence,
            self.sequence,
        );
        hex::encode(sha2::Sha256::digest(material.as_bytes()))
    }
}

/// Permit validation failure before a write effect.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityError {
    /// Permit identity is not present in the issuance ledger or its digest is invalid.
    #[error("permit is forged or was not issued by this authority ledger")]
    ForgedPermit,
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
