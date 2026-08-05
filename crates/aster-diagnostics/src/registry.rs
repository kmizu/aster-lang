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
    /// A declaration identity appeared twice in one module.
    DuplicateDeclaration,
    /// Projection or ordinary use of opaque candidate data.
    CandidateBeforeValidation,
    /// Ordinary type mismatch.
    TypeMismatch,
    /// Commit syntax omitted the mandatory permit.
    CommitWithoutPermit,
    /// Write tool declaration omitted idempotency metadata.
    MissingIdempotency,
    /// Proposal and permit action parameters differ.
    PermitActionMismatch,
    /// Write tool used through `observe`.
    WriteToolObserved,
    /// Read tool used through `propose`.
    ReadToolProposed,
    /// Tool called like a pure function.
    DirectToolCall,
    /// Effect occurred in a pure declaration.
    EffectInPureContext,
    /// Direct or mutual recursion.
    Recursion,
    /// Policy omitted a final otherwise rule.
    NonTotalPolicy,
    /// Permit variable used after consumption.
    PermitUseAfterMove,
    /// Proposal variable used after consumption.
    ProposalUseAfterMove,
    /// Agent or flow omitted a required capability kind.
    MissingCapability,
    /// Runtime grant file is invalid or lacks the exact scope.
    InvalidCapabilityGrant,
    /// Prompt instruction was not a static block string.
    DynamicPromptInstruction,
    /// Secret entered model data.
    SecretToModel,
    /// Secret entered persistent state.
    SecretInState,
    /// Unknown budget dimension.
    UnknownBudgetDimension,
    /// Duplicate budget dimension.
    DuplicateBudgetDimension,
    /// Runtime budget admission failed before an effect.
    BudgetExhausted,
    /// Typed runtime boundary or deterministic VM transition failed.
    RuntimeFailure,
    /// Replay semantics diverged from recorded evidence.
    ReplayDivergence,
    /// Trace schema, chain, or program identity is invalid.
    TraceMismatch,
    /// An implementation invariant failed safely.
    InternalFailure,
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
            Self::DuplicateDeclaration => "ASTER-NAME-1002",
            Self::CandidateBeforeValidation => "ASTER-TYPE-2001",
            Self::TypeMismatch => "ASTER-TYPE-2002",
            Self::CommitWithoutPermit => "ASTER-TYPE-2003",
            Self::MissingIdempotency => "ASTER-TYPE-2004",
            Self::PermitActionMismatch => "ASTER-TYPE-2005",
            Self::WriteToolObserved => "ASTER-EFFECT-3001",
            Self::ReadToolProposed => "ASTER-EFFECT-3002",
            Self::DirectToolCall => "ASTER-EFFECT-3003",
            Self::EffectInPureContext => "ASTER-EFFECT-3004",
            Self::Recursion => "ASTER-EFFECT-3005",
            Self::NonTotalPolicy => "ASTER-POLICY-4001",
            Self::PermitUseAfterMove => "ASTER-AFFINE-5001",
            Self::ProposalUseAfterMove => "ASTER-AFFINE-5002",
            Self::MissingCapability => "ASTER-CAP-6001",
            Self::InvalidCapabilityGrant => "ASTER-CAP-6002",
            Self::DynamicPromptInstruction => "ASTER-PROMPT-7001",
            Self::SecretToModel => "ASTER-SECRET-8001",
            Self::SecretInState => "ASTER-SECRET-8002",
            Self::UnknownBudgetDimension => "ASTER-BUDGET-11001",
            Self::DuplicateBudgetDimension => "ASTER-BUDGET-11002",
            Self::BudgetExhausted => "ASTER-BUDGET-11003",
            Self::RuntimeFailure => "ASTER-RUNTIME-9001",
            Self::ReplayDivergence => "ASTER-REPLAY-10001",
            Self::TraceMismatch => "ASTER-REPLAY-10002",
            Self::InternalFailure => "ASTER-INTERNAL-9901",
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
#[error("diagnostic codes must have the form ASTER-FAMILY-NNNN or ASTER-FAMILY-NNNNN")]
pub struct DiagnosticCodeError;

impl DiagnosticCode {
    /// Validates and constructs a diagnostic code.
    ///
    /// # Errors
    ///
    /// Returns [`DiagnosticCodeError`] unless the value has the exact
    /// `ASTER-FAMILY-NNNN` or `ASTER-FAMILY-NNNNN` shape.
    pub fn new(code: impl Into<String>) -> Result<Self, DiagnosticCodeError> {
        let code = code.into();
        let mut parts = code.split('-');
        let valid = parts.next() == Some("ASTER")
            && parts.next().is_some_and(|family| {
                !family.is_empty() && family.chars().all(|c| c.is_ascii_uppercase())
            })
            && parts.next().is_some_and(|number| {
                (4..=5).contains(&number.len()) && number.chars().all(|c| c.is_ascii_digit())
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

type ExplanationText = (&'static str, &'static str, &'static str);

fn parse_explanation(code: &str) -> Option<ExplanationText> {
    Some(match code {
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
        _ => return None,
    })
}

fn governance_explanation(code: &str) -> Option<ExplanationText> {
    Some(match code {
        "ASTER-POLICY-4001" => (
            "a policy is not total",
            "the policy does not have exactly one otherwise rule in final position",
            "remove any early otherwise rule and end with one otherwise decision",
        ),
        "ASTER-AFFINE-5001" => (
            "a permit was used after commit consumed it",
            "permits are affine and single-use",
            "obtain a new permit for a new proposal",
        ),
        "ASTER-AFFINE-5002" => (
            "a proposal was used after commit consumed it",
            "committed proposals are affine resources",
            "construct and authorize a new immutable proposal",
        ),
        "ASTER-CAP-6001" => (
            "an effect lacks a declared capability requirement",
            "the enclosing flow uses or agent requires list omits the capability kind",
            "declare the exact capability requirement before using the effect",
        ),
        "ASTER-CAP-6002" => (
            "a runtime capability grant is invalid or out of scope",
            "the versioned grant set does not exactly match a typed capability request",
            "issue the exact capability kind and canonical arguments required by the agent",
        ),
        "ASTER-PROMPT-7001" => (
            "prompt instruction is not a static block string",
            "runtime expressions cannot be promoted to instructions",
            "place all runtime values in the structured data block",
        ),
        "ASTER-SECRET-8001" => (
            "secret data would enter a model request",
            "Secret values are forbidden from prompt data",
            "pass a non-secret validated summary instead",
        ),
        "ASTER-SECRET-8002" => (
            "persistent state contains a Secret value",
            "secret handles cannot cross snapshot or state boundaries",
            "keep the secret in a sensitivity-secret tool boundary only",
        ),
        "ASTER-BUDGET-11001" => (
            "an unknown budget dimension was declared",
            "the dimension is outside the fixed ASTER 0.1 budget set",
            "use one of the six specified per-event dimensions",
        ),
        "ASTER-BUDGET-11002" => (
            "a budget dimension was declared more than once",
            "duplicate limits would make reservation semantics ambiguous",
            "keep exactly one limit for the dimension",
        ),
        "ASTER-BUDGET-11003" => (
            "an external effect exceeded its admitted budget",
            "no capacity remained for the fixed or declared maximum usage",
            "increase the declared per-event budget or reduce the requested usage",
        ),
        "ASTER-RUNTIME-9001" => (
            "a typed runtime boundary or deterministic VM transition failed",
            "external data, a fixture, authority, or machine state violated its schema",
            "correct the boundary artifact and retry from a verified snapshot",
        ),
        "ASTER-REPLAY-10001" => (
            "semantic replay diverged from recorded evidence",
            "a request, governance decision, budget transition, or outcome changed",
            "replay with the exact source, inputs, state, grants, and unmodified trace",
        ),
        "ASTER-REPLAY-10002" => (
            "the trace schema, hash chain, or program identity is invalid",
            "trace bytes were malformed, reordered, tampered with, or recorded for another program",
            "restore the original complete JSON Lines trace and matching source",
        ),
        "ASTER-INTERNAL-9901" => (
            "ASTER stopped at a protected implementation invariant",
            "a compiler or runtime state could not be represented safely",
            "preserve the inputs and report the deterministic failure context",
        ),
        _ => return None,
    })
}

/// Looks up checked-in documentation for a registered diagnostic code.
#[must_use]
pub fn explain(code: &str) -> Option<Explanation> {
    let (meaning, cause, remediation) = parse_explanation(code)
        .or_else(|| governance_explanation(code))
        .or_else(|| {
            Some(match code {
                "ASTER-NAME-1001" => (
                    "a referenced name has no declaration in its namespace",
                    "the name is misspelled or absent",
                    "declare the symbol or use an existing declared name",
                ),
                "ASTER-NAME-1002" => (
                    "a declaration identity appears more than once",
                    "two declarations occupy the same module namespace and name",
                    "rename or remove the later declaration",
                ),
                "ASTER-TYPE-2001" => (
                    "candidate data was used before validation",
                    "Candidate<T> intentionally has no value projection",
                    "validate candidate with a compatible validator to obtain Checked<T>",
                ),
                "ASTER-TYPE-2002" => (
                    "an expression type does not match its required type",
                    "a call, binding, field, return, or operator received an incompatible value",
                    "change the expression or its declared type so they match exactly",
                ),
                "ASTER-TYPE-2003" => (
                    "commit requires an explicit permit",
                    "the mandatory `with <permit>` clause is absent",
                    "authorize the proposal and commit it with the returned permit",
                ),
                "ASTER-TYPE-2004" => (
                    "a write tool has no idempotency parameter metadata",
                    "write tools must identify a deterministic request key",
                    "add `idempotency <parameter>;` for a serializable parameter",
                ),
                "ASTER-TYPE-2005" => (
                    "a permit action does not match the committed proposal action",
                    "the proposal and permit have different action phantom types",
                    "authorize and commit the same immutable proposal",
                ),
                "ASTER-EFFECT-3001" => (
                    "a write tool was invoked through observe",
                    "observe is restricted to read-mode tools",
                    "use intent, propose, authorize, and commit for writes",
                ),
                "ASTER-EFFECT-3002" => (
                    "a read tool was used to construct a proposal",
                    "propose is restricted to write-mode tools",
                    "invoke the read tool through observe",
                ),
                "ASTER-EFFECT-3003" => (
                    "a tool was called as an ordinary function",
                    "tool declarations are effect metadata, not pure functions",
                    "use observe for reads or the governed write pipeline for writes",
                ),
                "ASTER-EFFECT-3004" => (
                    "an external effect appears in a pure context",
                    "functions, validators, and policies cannot yield effects",
                    "move the effect to a flow or event handler and pass its result explicitly",
                ),
                "ASTER-EFFECT-3005" => (
                    "a direct or mutual call cycle was found",
                    "ASTER 0.1 source computation must be finite",
                    "replace recursion with a finite non-recursive expression",
                ),
                _ => return None,
            })
        })?;
    let code = DiagnosticCode::new(code).ok()?;
    Some(Explanation {
        code,
        severity: crate::Severity::Error,
        meaning,
        cause,
        remediation,
    })
}
