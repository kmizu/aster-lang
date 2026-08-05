use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{CanonicalError, canonical_sha256};

/// One append-only, hash-chained trace record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEntry {
    /// Trace schema version.
    pub schema_version: u32,
    /// Run identity.
    pub run_id: String,
    /// Zero-based sequence.
    pub sequence: u64,
    /// Stable logical entry kind.
    pub kind: String,
    /// Canonicalizable non-secret payload.
    pub payload: Value,
    /// Hash of the preceding complete entry.
    pub previous_entry_hash: String,
    /// Hash of this entry excluding this field.
    pub entry_hash: String,
}

/// In-memory append-only trace prior to atomic persistence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Trace {
    /// Run identity shared by every entry.
    pub run_id: String,
    /// Ordered trace entries.
    pub entries: Vec<TraceEntry>,
}

impl Trace {
    /// Starts an empty versioned trace.
    #[must_use]
    pub fn new(run_id: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            entries: Vec::new(),
        }
    }

    /// Appends and seals one logical event.
    ///
    /// # Errors
    ///
    /// Returns an error if canonical hashing fails or sequence space is exhausted.
    pub fn append(&mut self, kind: impl Into<String>, payload: Value) -> Result<(), TraceError> {
        let sequence = u64::try_from(self.entries.len()).map_err(|_| TraceError::TooLong)?;
        let previous_entry_hash = self
            .entries
            .last()
            .map_or_else(String::new, |entry| entry.entry_hash.clone());
        let mut entry = TraceEntry {
            schema_version: 1,
            run_id: self.run_id.clone(),
            sequence,
            kind: kind.into(),
            payload,
            previous_entry_hash,
            entry_hash: String::new(),
        };
        entry.entry_hash = canonical_sha256(&entry)?;
        self.entries.push(entry);
        Ok(())
    }

    /// Returns the next sequence and current chain head for snapshot binding.
    ///
    /// # Errors
    ///
    /// Rejects a trace whose length cannot be represented by the schema.
    pub fn checkpoint(&self) -> Result<(u64, String), TraceError> {
        Ok((
            u64::try_from(self.entries.len()).map_err(|_| TraceError::TooLong)?,
            self.entries
                .last()
                .map_or_else(String::new, |entry| entry.entry_hash.clone()),
        ))
    }

    /// Verifies schema, sequence, run identity, linkage, and every entry hash.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic divergence.
    pub fn verify(&self) -> Result<(), TraceError> {
        let mut previous = String::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let sequence = u64::try_from(index).map_err(|_| TraceError::TooLong)?;
            if entry.schema_version != 1
                || entry.run_id != self.run_id
                || entry.sequence != sequence
            {
                return Err(TraceError::MetadataMismatch { sequence });
            }
            if entry.previous_entry_hash != previous {
                return Err(TraceError::LinkMismatch { sequence });
            }
            let mut unhashed = entry.clone();
            unhashed.entry_hash.clear();
            if canonical_sha256(&unhashed)? != entry.entry_hash {
                return Err(TraceError::HashMismatch { sequence });
            }
            previous.clone_from(&entry.entry_hash);
        }
        Ok(())
    }

    /// Encodes the trace as canonical append-only JSON Lines.
    ///
    /// # Errors
    ///
    /// Rejects an invalid chain or canonical serialization failure.
    pub fn to_json_lines(&self) -> Result<String, TraceError> {
        self.verify()?;
        let mut output = String::new();
        for entry in &self.entries {
            output.push_str(&crate::canonical_json(entry)?);
            output.push('\n');
        }
        Ok(output)
    }

    /// Decodes and verifies a complete JSON Lines trace.
    ///
    /// # Errors
    ///
    /// Rejects malformed lines, empty traces, or any chain divergence.
    pub fn from_json_lines(input: &str) -> Result<Self, TraceError> {
        let entries = input
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| {
                serde_json::from_str(line).map_err(|_| TraceError::MalformedLine {
                    sequence: u64::try_from(index).unwrap_or(u64::MAX),
                })
            })
            .collect::<Result<Vec<TraceEntry>, _>>()?;
        let run_id = entries
            .first()
            .map(|entry| entry.run_id.clone())
            .ok_or(TraceError::Empty)?;
        let trace = Self { run_id, entries };
        trace.verify()?;
        Ok(trace)
    }
}

/// Trace construction or verification failure.
#[derive(Debug, Error)]
pub enum TraceError {
    /// Canonical hashing failed.
    #[error(transparent)]
    Canonical(#[from] CanonicalError),
    /// Trace length cannot be represented.
    #[error("trace sequence overflow")]
    TooLong,
    /// Entry metadata differs from its position or trace.
    #[error("trace metadata mismatch at sequence {sequence}")]
    MetadataMismatch { sequence: u64 },
    /// Previous hash linkage is broken.
    #[error("trace link mismatch at sequence {sequence}")]
    LinkMismatch { sequence: u64 },
    /// Entry content differs from its claimed hash.
    #[error("trace hash mismatch at sequence {sequence}")]
    HashMismatch { sequence: u64 },
    /// A JSONL record could not be decoded.
    #[error("malformed trace line at sequence {sequence}")]
    MalformedLine { sequence: u64 },
    /// A trace must contain at least its run header.
    #[error("trace is empty")]
    Empty,
}

impl PartialEq for TraceError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Canonical(_), Self::Canonical(_))
            | (Self::TooLong, Self::TooLong)
            | (Self::Empty, Self::Empty) => true,
            (
                Self::MetadataMismatch { sequence: left },
                Self::MetadataMismatch { sequence: right },
            )
            | (Self::LinkMismatch { sequence: left }, Self::LinkMismatch { sequence: right })
            | (Self::HashMismatch { sequence: left }, Self::HashMismatch { sequence: right })
            | (Self::MalformedLine { sequence: left }, Self::MalformedLine { sequence: right }) => {
                left == right
            }
            _ => false,
        }
    }
}

impl Eq for TraceError {}
