use serde::{Deserialize, Serialize};

use crate::{DiagnosticCode, Span};

/// Diagnostic severity. ASTER 0.1 currently emits errors only, while the
/// serialized enum reserves warning support without changing the schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Compilation or runtime cannot continue safely.
    Error,
    /// Non-fatal guidance.
    Warning,
}

/// A stable diagnostic safe to render as JSON or human-readable text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    /// Stable registry code.
    pub code: DiagnosticCode,
    /// Severity of the report.
    pub severity: Severity,
    /// Secret-free summary.
    pub message: String,
    /// Primary source range.
    pub primary_span: Span,
    /// Ordered labels attached to the primary context.
    pub labels: Vec<String>,
    /// Ordered explanatory notes.
    pub notes: Vec<String>,
    /// Optional actionable remediation.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Constructs an error diagnostic without secondary context.
    #[must_use]
    pub fn error(code: DiagnosticCode, message: impl Into<String>, primary_span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            primary_span,
            labels: Vec::new(),
            notes: Vec::new(),
            help: None,
        }
    }

    /// Appends a label while preserving source order.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Appends a note while preserving source order.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Sets actionable remediation text.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Serializes the diagnostic with the field order declared by this schema.
    ///
    /// # Errors
    ///
    /// Returns the underlying JSON serialization error if a future diagnostic
    /// field cannot be represented by `serde_json`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Renders a deterministic source excerpt without reading the filesystem.
    #[must_use]
    pub fn render_human(&self, source: &str) -> String {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let source_line = source
            .lines()
            .nth(self.primary_span.line.saturating_sub(1))
            .unwrap_or("");
        let mut rendered = format!(
            "{severity}[{}]: {}\n --> {}:{}:{}\n{} | {}\n",
            self.code.as_str(),
            self.message,
            self.primary_span.file,
            self.primary_span.line,
            self.primary_span.column,
            self.primary_span.line,
            source_line
        );
        for label in &self.labels {
            rendered.push_str("label: ");
            rendered.push_str(label);
            rendered.push('\n');
        }
        for note in &self.notes {
            rendered.push_str("note: ");
            rendered.push_str(note);
            rendered.push('\n');
        }
        if let Some(help) = &self.help {
            rendered.push_str("help: ");
            rendered.push_str(help);
            rendered.push('\n');
        }
        rendered
    }
}
