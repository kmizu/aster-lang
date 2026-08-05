use aster_diagnostics::Span;
use serde::{Deserialize, Serialize};

/// Reserved ASTER 0.1 word.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyword {
    /// `module`.
    Module,
    /// `type`.
    Type,
    /// `enum`.
    Enum,
    /// `capability`.
    Capability,
    /// `fn`.
    Fn,
    /// `flow`.
    Flow,
    /// `uses`.
    Uses,
    /// `prompt`.
    Prompt,
    /// `instruction`.
    Instruction,
    /// `data`.
    Data,
    /// `validator`.
    Validator,
    /// `require`.
    Require,
    /// `tool`.
    Tool,
    /// `mode`.
    Mode,
    /// `read`.
    Read,
    /// `write`.
    Write,
    /// `sensitivity`.
    Sensitivity,
    /// `public`.
    Public,
    /// `internal`.
    Internal,
    /// `private`.
    Private,
    /// `secret`.
    Secret,
    /// `risk`.
    Risk,
    /// `reversible`.
    Reversible,
    /// `irreversible`.
    Irreversible,
    /// `idempotency`.
    Idempotency,
    /// `policy`.
    Policy,
    /// `allow`.
    Allow,
    /// `when`.
    When,
    /// `approve`.
    Approve,
    /// `by`.
    By,
    /// `deny`.
    Deny,
    /// `otherwise`.
    Otherwise,
    /// `agent`.
    Agent,
    /// `requires`.
    Requires,
    /// `state`.
    State,
    /// `budget`.
    Budget,
    /// `per_event`.
    PerEvent,
    /// `on`.
    On,
    /// `return`.
    Return,
    /// `let`.
    Let,
    /// `update`.
    Update,
    /// `if`.
    If,
    /// `else`.
    Else,
    /// `match`.
    Match,
    /// `infer`.
    Infer,
    /// `using`.
    Using,
    /// `validate`.
    Validate,
    /// `with`.
    With,
    /// `observe`.
    Observe,
    /// `intent`.
    Intent,
    /// `propose`.
    Propose,
    /// `for`.
    For,
    /// `authorize`.
    Authorize,
    /// `commit`.
    Commit,
    /// `reconcile`.
    Reconcile,
    /// `against`.
    Against,
    /// `true`.
    True,
    /// `false`.
    False,
}

impl Keyword {
    pub(crate) fn from_identifier(identifier: &str) -> Option<Self> {
        Some(match identifier {
            "module" => Self::Module,
            "type" => Self::Type,
            "enum" => Self::Enum,
            "capability" => Self::Capability,
            "fn" => Self::Fn,
            "flow" => Self::Flow,
            "uses" => Self::Uses,
            "prompt" => Self::Prompt,
            "instruction" => Self::Instruction,
            "data" => Self::Data,
            "validator" => Self::Validator,
            "require" => Self::Require,
            "tool" => Self::Tool,
            "mode" => Self::Mode,
            "read" => Self::Read,
            "write" => Self::Write,
            "sensitivity" => Self::Sensitivity,
            "public" => Self::Public,
            "internal" => Self::Internal,
            "private" => Self::Private,
            "secret" => Self::Secret,
            "risk" => Self::Risk,
            "reversible" => Self::Reversible,
            "irreversible" => Self::Irreversible,
            "idempotency" => Self::Idempotency,
            "policy" => Self::Policy,
            "allow" => Self::Allow,
            "when" => Self::When,
            "approve" => Self::Approve,
            "by" => Self::By,
            "deny" => Self::Deny,
            "otherwise" => Self::Otherwise,
            "agent" => Self::Agent,
            "requires" => Self::Requires,
            "state" => Self::State,
            "budget" => Self::Budget,
            "per_event" => Self::PerEvent,
            "on" => Self::On,
            "return" => Self::Return,
            "let" => Self::Let,
            "update" => Self::Update,
            "if" => Self::If,
            "else" => Self::Else,
            "match" => Self::Match,
            "infer" => Self::Infer,
            "using" => Self::Using,
            "validate" => Self::Validate,
            "with" => Self::With,
            "observe" => Self::Observe,
            "intent" => Self::Intent,
            "propose" => Self::Propose,
            "for" => Self::For,
            "authorize" => Self::Authorize,
            "commit" => Self::Commit,
            "reconcile" => Self::Reconcile,
            "against" => Self::Against,
            "true" => Self::True,
            "false" => Self::False,
            _ => return None,
        })
    }
}

/// Punctuation and operators, with multi-byte spellings represented atomically.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Symbol {
    /// `(`.
    LeftParen,
    /// `)`.
    RightParen,
    /// `{`.
    LeftBrace,
    /// `}`.
    RightBrace,
    /// `[`.
    LeftBracket,
    /// `]`.
    RightBracket,
    /// `,`.
    Comma,
    /// `:`.
    Colon,
    /// `;`.
    Semicolon,
    /// `.`.
    Dot,
    /// `?`.
    Question,
    /// `+`.
    Plus,
    /// `-`.
    Minus,
    /// `*`.
    Star,
    /// `/`.
    Slash,
    /// `!`.
    Bang,
    /// `=`.
    Equal,
    /// `<`.
    Less,
    /// `>`.
    Greater,
    /// `->`.
    Arrow,
    /// `=>`.
    FatArrow,
    /// `==`.
    EqualEqual,
    /// `!=`.
    BangEqual,
    /// `<=`.
    LessEqual,
    /// `>=`.
    GreaterEqual,
    /// `&&`.
    AndAnd,
    /// `||`.
    OrOr,
}

/// One lossless lexical token.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TokenKind {
    /// Reserved word.
    Keyword(Keyword),
    /// User-defined ASCII identifier.
    Identifier(String),
    /// `@`-prefixed model alias without the prefix.
    ModelAlias(String),
    /// Signed-range decimal magnitude; negation is a separate token.
    Integer(i64),
    /// Decoded JSON-style string.
    String(String),
    /// Raw semantic contents between triple quotes.
    BlockString(String),
    /// Punctuation or operator.
    Symbol(Symbol),
    /// Whitespace retained for lossless formatting.
    Whitespace(String),
    /// Entire `//` comment excluding its newline.
    LineComment(String),
    /// Entire possibly nested `/* */` comment.
    BlockComment(String),
    /// End of source.
    Eof,
}

impl TokenKind {
    /// Returns whether this token is retained trivia rather than grammar input.
    #[must_use]
    pub const fn is_trivia(&self) -> bool {
        matches!(
            self,
            Self::Whitespace(_) | Self::LineComment(_) | Self::BlockComment(_)
        )
    }
}

/// A token paired with its exact source extent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Token {
    /// Lexical value.
    pub kind: TokenKind,
    /// Exact source span.
    pub span: Span,
}
