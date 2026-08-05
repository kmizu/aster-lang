#![forbid(unsafe_code)]

//! Stable, serializable diagnostics shared by every ASTER layer.

mod diagnostic;
mod registry;
mod span;

pub use diagnostic::{Diagnostic, Severity};
pub use registry::{
    DiagnosticCode, DiagnosticCodeError, Explanation, KnownDiagnosticCode, explain,
};
pub use span::{Span, SpanError};
