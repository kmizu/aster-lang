use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A half-open UTF-8 byte range with a one-based start line and column.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Span {
    /// Logical source path used in diagnostics.
    pub file: String,
    /// Inclusive UTF-8 byte offset.
    pub start: usize,
    /// Exclusive UTF-8 byte offset.
    pub end: usize,
    /// One-based line containing `start`.
    pub line: usize,
    /// One-based Unicode scalar column containing `start`.
    pub column: usize,
}

/// Controlled failures while constructing a trusted source span.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SpanError {
    /// The range is reversed or extends past the source.
    #[error("span offsets are outside the source")]
    OutOfBounds,
    /// Either endpoint splits a UTF-8 code point.
    #[error("span offsets are not UTF-8 boundaries")]
    InvalidUtf8Boundary,
}

impl Span {
    /// Constructs a span after validating its byte range and computing location.
    ///
    /// The returned line and column are one-based. The method never rounds an
    /// invalid byte offset, because doing so would make diagnostics point at a
    /// different token than the parser observed.
    ///
    /// # Errors
    ///
    /// Returns [`SpanError::OutOfBounds`] for a reversed or oversized range and
    /// [`SpanError::InvalidUtf8Boundary`] when an endpoint splits a code point.
    pub fn from_offsets(
        file: impl Into<String>,
        source: &str,
        start: usize,
        end: usize,
    ) -> Result<Self, SpanError> {
        if start > end || end > source.len() {
            return Err(SpanError::OutOfBounds);
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(SpanError::InvalidUtf8Boundary);
        }

        let prefix = &source[..start];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
        let column = source[line_start..start].chars().count() + 1;

        Ok(Self {
            file: file.into(),
            start,
            end,
            line,
            column,
        })
    }
}
