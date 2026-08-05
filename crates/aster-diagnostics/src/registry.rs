use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated stable ASTER diagnostic identifier.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DiagnosticCode(String);

/// Failure to construct a syntactically valid diagnostic code.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("diagnostic codes must have the form ASTER-FAMILY-NNNN")]
pub struct DiagnosticCodeError;

impl DiagnosticCode {
    /// Validates and constructs a diagnostic code.
    ///
    /// # Errors
    ///
    /// Returns [`DiagnosticCodeError`] unless the value has the exact
    /// `ASTER-FAMILY-NNNN` shape.
    pub fn new(code: impl Into<String>) -> Result<Self, DiagnosticCodeError> {
        let code = code.into();
        let mut parts = code.split('-');
        let valid = parts.next() == Some("ASTER")
            && parts.next().is_some_and(|family| {
                !family.is_empty() && family.chars().all(|c| c.is_ascii_uppercase())
            })
            && parts.next().is_some_and(|number| {
                number.len() == 4 && number.chars().all(|c| c.is_ascii_digit())
            })
            && parts.next().is_none();
        if !valid {
            return Err(DiagnosticCodeError);
        }
        Ok(Self(code))
    }

    /// Returns the stable textual identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Documentation consumed by `aster explain`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Explanation {
    /// Stable identifier.
    pub code: DiagnosticCode,
    /// Default severity.
    pub severity: crate::Severity,
    /// What the diagnostic means.
    pub meaning: &'static str,
    /// Typical cause.
    pub cause: &'static str,
    /// Safe remediation guidance.
    pub remediation: &'static str,
}

/// Looks up checked-in documentation for a registered diagnostic code.
#[must_use]
pub fn explain(code: &str) -> Option<Explanation> {
    let (meaning, cause, remediation) = match code {
        "ASTER-PARSE-0001" => (
            "the source does not conform to the ASTER 0.1 grammar",
            "an unexpected or malformed token was encountered",
            "correct the token at the reported source span",
        ),
        "ASTER-NAME-1001" => (
            "a referenced name has no declaration in its namespace",
            "the name is misspelled or absent",
            "declare the symbol or use an existing declared name",
        ),
        "ASTER-TYPE-2001" => (
            "candidate data was used before validation",
            "Candidate<T> intentionally has no value projection",
            "validate candidate with a compatible validator to obtain Checked<T>",
        ),
        _ => return None,
    };
    let code = DiagnosticCode::new(code).ok()?;
    Some(Explanation {
        code,
        severity: crate::Severity::Error,
        meaning,
        cause,
        remediation,
    })
}
