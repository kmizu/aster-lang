use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated stable ASTER diagnostic identifier.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DiagnosticCode(String);

/// Compile-time-selected codes used by compiler and runtime implementations.
///
/// Adding a variant is an API change that must be accompanied by registry
/// documentation and a conformance test.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnownDiagnosticCode {
    /// Generic unexpected syntax.
    ParseError,
    /// Invalid JSON-style escape in a string.
    InvalidStringEscape,
    /// Character with no lexical meaning.
    UnknownToken,
    /// Decimal integer outside the ASTER `Int` range.
    InvalidInteger,
    /// Nested block comment without a closing delimiter.
    UnterminatedBlockComment,
    /// JSON-style string without a closing quote.
    UnterminatedString,
    /// Triple-quoted block string without a closing delimiter.
    UnterminatedBlockString,
    /// Missing declaration in the relevant namespace.
    UnknownName,
    /// Projection or ordinary use of opaque candidate data.
    CandidateBeforeValidation,
}

impl KnownDiagnosticCode {
    /// Returns the stable textual identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParseError => "ASTER-PARSE-0001",
            Self::InvalidStringEscape => "ASTER-PARSE-0002",
            Self::UnknownToken => "ASTER-PARSE-0003",
            Self::InvalidInteger => "ASTER-PARSE-0004",
            Self::UnterminatedBlockComment => "ASTER-PARSE-0005",
            Self::UnterminatedString => "ASTER-PARSE-0006",
            Self::UnterminatedBlockString => "ASTER-PARSE-0007",
            Self::UnknownName => "ASTER-NAME-1001",
            Self::CandidateBeforeValidation => "ASTER-TYPE-2001",
        }
    }
}

impl From<KnownDiagnosticCode> for DiagnosticCode {
    fn from(code: KnownDiagnosticCode) -> Self {
        Self(code.as_str().to_owned())
    }
}

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
        "ASTER-PARSE-0002" => (
            "a string contains an invalid JSON-style escape",
            "the escape is unknown, incomplete, or has invalid Unicode digits",
            "use a valid JSON escape such as \\n, \\\\, or \\u followed by four hex digits",
        ),
        "ASTER-PARSE-0003" => (
            "the lexer encountered a character with no ASTER meaning",
            "the character is not part of the ASTER 0.1 lexical grammar",
            "remove the character or replace it with a supported token",
        ),
        "ASTER-PARSE-0004" => (
            "a decimal integer is outside the ASTER Int range",
            "the literal does not fit a signed 64-bit integer",
            "use a value from -9223372036854775808 through 9223372036854775807",
        ),
        "ASTER-PARSE-0005" => (
            "a nested block comment is unterminated",
            "one or more /* delimiters have no matching */",
            "close every nested block comment",
        ),
        "ASTER-PARSE-0006" => (
            "a JSON-style string literal is unterminated",
            "the closing quote is missing",
            "add the closing quote without crossing a source line",
        ),
        "ASTER-PARSE-0007" => (
            "a triple-quoted block string is unterminated",
            "the closing triple quote is missing",
            "add a matching triple quote after the static instruction text",
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
