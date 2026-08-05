use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{EffectKind, EffectRequest, EffectResolution};

/// Versioned deterministic fixture collection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FixtureSet {
    /// Fixture schema version.
    pub schema_version: u32,
    /// Source-ordered deterministic responses.
    pub entries: Vec<FixtureEntry>,
}

/// One exact effect kind/identity plus explicit request-field match.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FixtureEntry {
    /// External effect category.
    pub kind: EffectKind,
    /// Prompt, tool, or policy identity.
    pub identity: String,
    /// Recursive subset that the complete request payload must contain.
    pub match_request: Value,
    /// Typed synthetic response payload.
    pub response: Value,
    /// Variable usage reserved before driver invocation.
    pub max_usage: BTreeMap<String, u64>,
    /// Deterministic actual usage settled after the response.
    pub actual_usage: BTreeMap<String, u64>,
}

/// Pure pre-driver fixture match result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixturePreview {
    index: usize,
    /// Maximum variable resource usage.
    pub max_usage: BTreeMap<String, u64>,
}

/// The only ASTER 0.1 production driver: deterministic synthetic fixtures.
pub struct FixtureDriver {
    fixtures: FixtureSet,
    consumed: BTreeSet<usize>,
    calls: BTreeMap<EffectKind, u64>,
}

/// Narrow external effect boundary used only after admission checks.
pub trait EffectDriver {
    /// Finds an exact fixture without performing or counting an external effect.
    ///
    /// # Errors
    ///
    /// Rejects missing, ambiguous, or already consumed fixture matches.
    fn preview(&self, request: &EffectRequest) -> Result<FixturePreview, DriverError>;

    /// Resolves a previously admitted request and counts the invocation.
    ///
    /// # Errors
    ///
    /// Rejects stale previews and actual usage above the declared maximum.
    fn resolve(
        &mut self,
        request: &EffectRequest,
        preview: &FixturePreview,
    ) -> Result<EffectResolution, DriverError>;
}

impl FixtureDriver {
    /// Validates and constructs a deterministic fixture driver.
    ///
    /// # Errors
    ///
    /// Rejects unsupported schemas and empty match objects that would select
    /// fixtures too loosely.
    pub fn new(fixtures: FixtureSet) -> Result<Self, DriverError> {
        if fixtures.schema_version != 1 {
            return Err(DriverError::SchemaMismatch);
        }
        if fixtures.entries.iter().any(|entry| {
            entry
                .match_request
                .as_object()
                .is_none_or(serde_json::Map::is_empty)
        }) {
            return Err(DriverError::LooseFixture);
        }
        Ok(Self {
            fixtures,
            consumed: BTreeSet::new(),
            calls: BTreeMap::new(),
        })
    }

    /// Returns actual driver invocation count by effect kind.
    #[must_use]
    pub fn call_count(&self, kind: EffectKind) -> u64 {
        self.calls.get(&kind).copied().unwrap_or(0)
    }

    fn matching_indices(&self, request: &EffectRequest) -> Vec<usize> {
        self.fixtures
            .entries
            .iter()
            .enumerate()
            .filter(|(index, entry)| {
                !self.consumed.contains(index)
                    && entry.kind == request.kind
                    && entry.identity == request.identity
                    && is_json_subset(&entry.match_request, &request.payload)
            })
            .map(|(index, _)| index)
            .collect()
    }
}

impl EffectDriver for FixtureDriver {
    fn preview(&self, request: &EffectRequest) -> Result<FixturePreview, DriverError> {
        let matches = self.matching_indices(request);
        let index = *matches.first().ok_or(DriverError::NoMatchingFixture)?;
        if matches.len() > 1 {
            let first_match = &self.fixtures.entries[index].match_request;
            if matches
                .iter()
                .skip(1)
                .any(|candidate| self.fixtures.entries[*candidate].match_request != *first_match)
            {
                return Err(DriverError::AmbiguousFixture);
            }
        }
        Ok(FixturePreview {
            index,
            max_usage: self.fixtures.entries[index].max_usage.clone(),
        })
    }

    fn resolve(
        &mut self,
        request: &EffectRequest,
        preview: &FixturePreview,
    ) -> Result<EffectResolution, DriverError> {
        if !self.matching_indices(request).contains(&preview.index) {
            return Err(DriverError::PreviewMismatch);
        }
        self.consumed.insert(preview.index);
        *self.calls.entry(request.kind).or_default() += 1;
        let entry = &self.fixtures.entries[preview.index];
        if entry
            .actual_usage
            .iter()
            .any(|(name, actual)| *actual > entry.max_usage.get(name).copied().unwrap_or(0))
        {
            return Err(DriverError::ActualExceedsMaximum);
        }
        Ok(EffectResolution {
            request_hash: request.request_hash.clone(),
            payload: entry.response.clone(),
            actual_usage: entry.actual_usage.clone(),
        })
    }
}

fn is_json_subset(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual| is_json_subset(value, actual))
        }),
        (Value::Array(expected), Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| is_json_subset(expected, actual))
        }
        _ => expected == actual,
    }
}

/// Fixture matching or driver response failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DriverError {
    #[error("fixture schema mismatch")]
    SchemaMismatch,
    #[error("fixture request match must contain explicit fields")]
    LooseFixture,
    #[error("no matching fixture")]
    NoMatchingFixture,
    #[error("fixture request match is ambiguous")]
    AmbiguousFixture,
    #[error("fixture preview no longer matches")]
    PreviewMismatch,
    #[error("driver actual usage exceeds declared maximum")]
    ActualExceedsMaximum,
}
