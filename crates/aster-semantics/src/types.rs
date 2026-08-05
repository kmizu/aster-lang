use serde::Serialize;

/// A resolved ASTER value type used by checking and IR lowering.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "arguments", rename_all = "snake_case")]
pub enum Type {
    /// Recovery-only type that prevents cascading diagnostics.
    Unknown,
    /// `Unit`.
    Unit,
    /// `Bool`.
    Bool,
    /// `Int`.
    Int,
    /// `Text`.
    Text,
    /// `Instant`.
    Instant,
    /// `Duration`.
    Duration,
    /// `ProvenanceRef`.
    ProvenanceRef,
    /// `Error`.
    Error,
    /// User-defined alias, record, or enum identity.
    Named(String),
    /// `Option<T>`.
    Option(Box<Type>),
    /// `Result<T,E>`.
    Result(Box<Type>, Box<Type>),
    /// `List<T>`.
    List(Box<Type>),
    /// `Incoming<T>`.
    Incoming(Box<Type>),
    /// `Untrusted<T>`.
    Untrusted(Box<Type>),
    /// Opaque `Candidate<T>`.
    Candidate(Box<Type>),
    /// `Checked<T>`.
    Checked(Box<Type>),
    /// `Observation<T>`.
    Observation(Box<Type>),
    /// Opaque `Secret<T>`.
    Secret(Box<Type>),
    /// `Intent<P>` with a static purpose symbol.
    Intent(String),
    /// `Proposal<A>` with a static action symbol.
    Proposal(String),
    /// `Permit<A>` with a static action symbol.
    Permit(String),
    /// `Receipt<A>` with a static action symbol.
    Receipt(String),
    /// `Reconciled<A>` with a static action symbol.
    Reconciled(String),
    /// Internal read-only view of a proposal action's named arguments.
    ToolArguments(String),
    /// Internal read-only view of an agent's persistent state.
    AgentState(String),
    /// Runtime-provided event metadata.
    Event,
}

impl Type {
    /// Returns whether this type transitively carries opaque candidate data.
    #[must_use]
    pub fn contains_candidate(&self) -> bool {
        match self {
            Self::Candidate(_) => true,
            Self::Option(inner)
            | Self::List(inner)
            | Self::Incoming(inner)
            | Self::Untrusted(inner)
            | Self::Checked(inner)
            | Self::Observation(inner)
            | Self::Secret(inner) => inner.contains_candidate(),
            Self::Result(ok, error) => ok.contains_candidate() || error.contains_candidate(),
            _ => false,
        }
    }

    /// Returns whether this type directly or transitively contains `Secret`.
    #[must_use]
    pub fn contains_secret(&self) -> bool {
        match self {
            Self::Secret(_) => true,
            Self::Option(inner)
            | Self::List(inner)
            | Self::Incoming(inner)
            | Self::Untrusted(inner)
            | Self::Candidate(inner)
            | Self::Checked(inner)
            | Self::Observation(inner) => inner.contains_secret(),
            Self::Result(ok, error) => ok.contains_secret() || error.contains_secret(),
            _ => false,
        }
    }
}
