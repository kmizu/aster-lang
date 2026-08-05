use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical JSON conversion failure.
#[derive(Debug, Error)]
pub enum CanonicalError {
    /// A typed value could not be represented as JSON.
    #[error("canonical JSON conversion failed: {0}")]
    Serialization(serde_json::Error),
}

/// Serializes a JSON value with recursively sorted object keys and no spacing.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn canonical_json(value: &Value) -> Result<String, CanonicalError> {
    serde_json::to_string(&canonicalize(value)).map_err(CanonicalError::Serialization)
}

/// Hashes any serializable value through recursively key-sorted JSON.
///
/// # Errors
///
/// Returns an error if conversion or serialization fails.
pub fn canonical_sha256<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    let value = serde_json::to_value(value).map_err(CanonicalError::Serialization)?;
    let bytes = canonical_json(&value)?;
    Ok(hex::encode(Sha256::digest(bytes.as_bytes())))
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
        Value::Object(values) => {
            let mut entries: Vec<_> = values.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonicalize(value)))
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}
