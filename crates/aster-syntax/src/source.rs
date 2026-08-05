/// An already UTF-8-validated source file and its logical diagnostic path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    path: String,
    text: String,
}

impl SourceFile {
    /// Constructs source from a Rust string, which is valid UTF-8 by type.
    #[must_use]
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
        }
    }

    /// Returns the logical path used in source diagnostics.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the complete source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}
