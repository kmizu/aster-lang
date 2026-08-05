use aster_diagnostics::Span;
use serde::Serialize;

/// A parsed ASTER module with lossless comments retained separately by span.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Module {
    /// Dotted module identity.
    pub name: Path,
    /// Declarations in source order.
    pub declarations: Vec<Declaration>,
    /// Comments in source order.
    pub comments: Vec<Comment>,
    /// Extent of the complete module.
    pub span: Span,
}

impl Module {
    /// Serializes the syntax tree to deterministic JSON.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization error if a future AST field is not
    /// representable by `serde_json`.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes semantic syntax shape without source locations or comments.
    ///
    /// This representation is used to prove parse-format-parse preservation;
    /// it is not the public `aster ast --json` schema.
    ///
    /// # Errors
    ///
    /// Returns a JSON serialization error if the AST cannot be represented.
    pub fn normalized_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        strip_source_metadata(&mut value);
        serde_json::to_string(&value)
    }
}

fn strip_source_metadata(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                strip_source_metadata(value);
            }
        }
        serde_json::Value::Object(values) => {
            values.remove("span");
            values.remove("comments");
            for value in values.values_mut() {
                strip_source_metadata(value);
            }
        }
        _ => {}
    }
}

/// A dotted statically resolved source path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Path {
    /// Identifier segments without punctuation.
    pub segments: Vec<String>,
    /// Exact path span.
    pub span: Span,
}

impl Path {
    /// Returns the canonical dotted spelling.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.segments.join(".")
    }
}

/// A retained source comment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Comment {
    /// Comment including delimiters.
    pub text: String,
    /// Exact source span.
    pub span: Span,
}

/// One top-level declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Declaration {
    /// Exact declaration span.
    pub span: Span,
    /// Declaration-specific syntax.
    #[serde(flatten)]
    pub kind: DeclarationKind,
}

/// Supported declaration forms.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeclarationKind {
    /// Alias or record declaration.
    Type(TypeDeclaration),
    /// Enum declaration.
    Enum(EnumDeclaration),
    /// Capability signature.
    Capability(SignatureDeclaration),
    /// Pure function.
    Function(FunctionDeclaration),
    /// Effectful reusable flow.
    Flow(FunctionDeclaration),
    /// Static prompt declaration.
    Prompt(PromptDeclaration),
    /// Pure validator.
    Validator(ValidatorDeclaration),
    /// Typed tool boundary metadata.
    Tool(ToolDeclaration),
    /// Total ordered policy.
    Policy(PolicyDeclaration),
    /// Durable agent declaration.
    Agent(AgentDeclaration),
}

impl DeclarationKind {
    /// Returns a stable broad category used by syntax consumers.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::Type(_) => "type",
            Self::Enum(_) => "enum",
            Self::Capability(_) => "capability",
            Self::Function(_) => "function",
            Self::Flow(_) => "flow",
            Self::Prompt(_) => "prompt",
            Self::Validator(_) => "validator",
            Self::Tool(_) => "tool",
            Self::Policy(_) => "policy",
            Self::Agent(_) => "agent",
        }
    }
}

/// A type alias or record declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TypeDeclaration {
    /// Declared name.
    pub name: String,
    /// Alias or record body.
    pub definition: TypeDefinition,
}

/// The body of a non-generic user type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "definition", content = "value", rename_all = "snake_case")]
pub enum TypeDefinition {
    /// Alias to another type expression.
    Alias(TypeReference),
    /// Ordered record fields.
    Record(Vec<TypeField>),
}

/// A named typed field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TypeField {
    /// Field name.
    pub name: String,
    /// Declared field type.
    pub ty: TypeReference,
    /// Exact field span.
    pub span: Span,
}

/// A path type with optional compiler-known generic arguments.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TypeReference {
    /// Type constructor or user type path.
    pub path: Path,
    /// Ordered type arguments.
    pub arguments: Vec<TypeReference>,
    /// Exact type expression span.
    pub span: Span,
}

/// A non-generic enum declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnumDeclaration {
    /// Declared enum name.
    pub name: String,
    /// Variants in source order.
    pub variants: Vec<EnumVariant>,
}

/// A nullary or single-payload enum variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnumVariant {
    /// Variant name.
    pub name: String,
    /// Optional single payload type.
    pub payload: Option<TypeReference>,
    /// Exact variant span.
    pub span: Span,
}

/// A named parameter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Parameter {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub ty: TypeReference,
    /// Exact parameter span.
    pub span: Span,
}

/// A capability signature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SignatureDeclaration {
    /// Capability name.
    pub name: String,
    /// Ordered parameters.
    pub parameters: Vec<Parameter>,
}

/// A pure function or effectful flow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FunctionDeclaration {
    /// Declaration name.
    pub name: String,
    /// Ordered parameters.
    pub parameters: Vec<Parameter>,
    /// Declared return type.
    pub return_type: TypeReference,
    /// Flow capability upper bound; empty for pure functions.
    pub uses: Vec<CapabilityExpression>,
    /// Executable body.
    pub body: Block,
}

/// A static prompt declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PromptDeclaration {
    /// Prompt symbol.
    pub name: String,
    /// Typed runtime data parameters.
    pub parameters: Vec<Parameter>,
    /// Expected decoded result type.
    pub result_type: TypeReference,
    /// Raw contents of the one static block string.
    pub instruction: String,
    /// Ordered parameter names admitted to structured data.
    pub data: Vec<String>,
}

/// A pure validation declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidatorDeclaration {
    /// Validator symbol.
    pub name: String,
    /// One parameter for candidate validation or two for reconciliation.
    pub parameters: Vec<Parameter>,
    /// Requirements in source order.
    pub requirements: Vec<Expression>,
}

/// A tool declaration and its mandatory boundary metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolDeclaration {
    /// Dotted action identity.
    pub path: Path,
    /// Typed request parameters.
    pub parameters: Vec<Parameter>,
    /// Typed response.
    pub return_type: TypeReference,
    /// Metadata body.
    pub metadata: ToolMetadata,
}

/// Read or write mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    /// Observation-only tool.
    Read,
    /// Mutating tool requiring proposal and permit.
    Write,
}

/// Declared data sensitivity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    /// Safe public data.
    Public,
    /// Organization-internal data.
    Internal,
    /// Private non-secret data.
    Private,
    /// Opaque secret data.
    Secret,
}

/// Declared write reversibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// The action has a defined reversal outside ASTER 0.1.
    Reversible,
    /// The action cannot reliably be reversed.
    Irreversible,
}

/// Parsed tool metadata; semantic analysis enforces mode-specific presence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ToolMetadata {
    /// Optional at syntax level so declaration checking can issue a stable code.
    pub mode: Option<ToolMode>,
    /// Required capability request.
    pub capability: Option<CapabilityExpression>,
    /// Data sensitivity.
    pub sensitivity: Option<Sensitivity>,
    /// Write risk.
    pub risk: Option<Risk>,
    /// Write idempotency parameter name.
    pub idempotency: Option<String>,
}

/// A capability constructor and arguments.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityExpression {
    /// Capability declaration path.
    pub path: Path,
    /// Typed argument expressions.
    pub arguments: Vec<Argument>,
    /// Exact expression span.
    pub span: Span,
}

/// An ordered authorization policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolicyDeclaration {
    /// Policy symbol.
    pub name: String,
    /// Proposal and explicit state/value parameters.
    pub parameters: Vec<Parameter>,
    /// Decision rules in source order.
    pub rules: Vec<PolicyRule>,
}

/// One policy decision rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolicyRule {
    /// Decision produced on match.
    pub decision: PolicyDecision,
    /// Optional condition; `None` represents final `otherwise`.
    pub condition: Option<Expression>,
    /// Exact rule span.
    pub span: Span,
}

/// Policy decision syntax.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "decision", content = "value", rename_all = "snake_case")]
pub enum PolicyDecision {
    /// Direct authorization.
    Allow,
    /// Approval principal expression.
    Approve(Expression),
    /// Safe denial reason.
    Deny(Expression),
}

/// A durable agent declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentDeclaration {
    /// Agent symbol.
    pub name: String,
    /// Runtime constructor arguments.
    pub parameters: Vec<Parameter>,
    /// Declared capability requirements.
    pub requires: Vec<CapabilityExpression>,
    /// Persistent state schema and defaults.
    pub state: Vec<StateField>,
    /// Per-event budget entries.
    pub budget: Vec<BudgetEntry>,
    /// Event handlers.
    pub handlers: Vec<HandlerDeclaration>,
}

/// Persistent agent state field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StateField {
    /// Field name.
    pub name: String,
    /// Persistent type.
    pub ty: TypeReference,
    /// Source default expression.
    pub default: Expression,
    /// Exact field span.
    pub span: Span,
}

/// One named non-negative budget limit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BudgetEntry {
    /// Dimension name.
    pub dimension: String,
    /// Inclusive per-event maximum.
    pub limit: i64,
    /// Exact entry span.
    pub span: Span,
}

/// An agent event handler.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HandlerDeclaration {
    /// Event name.
    pub event: String,
    /// Typed event parameters.
    pub parameters: Vec<Parameter>,
    /// Declared handler return type.
    pub return_type: TypeReference,
    /// Executable body.
    pub body: Block,
    /// Exact handler span.
    pub span: Span,
}

/// A statement block.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Block {
    /// Statements in execution order.
    pub statements: Vec<Statement>,
    /// Exact brace-delimited span.
    pub span: Span,
}

/// One executable statement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Statement {
    /// Statement-specific syntax.
    #[serde(flatten)]
    pub kind: StatementKind,
    /// Exact statement span.
    pub span: Span,
}

/// Supported statement forms.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "statement", rename_all = "snake_case")]
pub enum StatementKind {
    /// Immutable local binding.
    Let {
        /// Binding name.
        name: String,
        /// Optional explicit type.
        ty: Option<TypeReference>,
        /// Initial value.
        value: Expression,
    },
    /// Runtime/validator requirement.
    Require {
        /// Required boolean expression.
        condition: Expression,
    },
    /// Pending atomic state update.
    UpdateState {
        /// Ordered field assignments.
        fields: Vec<FieldInitializer>,
    },
    /// Function, flow, or handler return.
    Return {
        /// Returned expression.
        value: Expression,
    },
    /// Effect or pure expression statement.
    Expression {
        /// Evaluated expression.
        expression: Expression,
    },
}

/// A typed expression with an exact source span.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Expression {
    /// Expression-specific syntax.
    #[serde(flatten)]
    pub kind: ExpressionKind,
    /// Exact expression span.
    pub span: Span,
}

/// Supported expression forms.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "expression", rename_all = "snake_case")]
pub enum ExpressionKind {
    /// Unit literal.
    Unit,
    /// Boolean literal.
    Bool { value: bool },
    /// Integer literal.
    Int { value: i64 },
    /// Text literal.
    Text { value: String },
    /// Statically resolved path reference.
    Path { path: Path },
    /// List literal.
    List { elements: Vec<Expression> },
    /// Record construction.
    Record {
        /// Record type path.
        path: Path,
        /// Ordered field initializers.
        fields: Vec<FieldInitializer>,
    },
    /// Function or constructor call.
    Call {
        /// Callee expression.
        callee: Box<Expression>,
        /// Positional or named arguments.
        arguments: Vec<Argument>,
    },
    /// Field projection.
    Field {
        /// Projected value.
        target: Box<Expression>,
        /// Field name.
        field: String,
    },
    /// Unary operator.
    Unary {
        /// Operator.
        operator: UnaryOperator,
        /// Operand.
        operand: Box<Expression>,
    },
    /// Binary operator.
    Binary {
        /// Left operand.
        left: Box<Expression>,
        /// Operator.
        operator: BinaryOperator,
        /// Right operand.
        right: Box<Expression>,
    },
    /// `Result` propagation.
    Try { value: Box<Expression> },
    /// Conditional expression.
    If {
        /// Boolean condition.
        condition: Box<Expression>,
        /// Selected when true.
        then_block: Block,
        /// Selected when false.
        else_block: Block,
    },
    /// Finite pattern match.
    Match {
        /// Scrutinee.
        value: Box<Expression>,
        /// Ordered arms.
        arms: Vec<MatchArm>,
    },
    /// Model inference effect request.
    Infer {
        /// Prompt symbol.
        prompt: Path,
        /// Structured prompt arguments.
        arguments: Vec<Argument>,
        /// Model alias without `@`.
        model_alias: String,
    },
    /// Candidate validation.
    Validate {
        /// Opaque candidate expression.
        candidate: Box<Expression>,
        /// Validator symbol.
        validator: Path,
    },
    /// Read tool observation.
    Observe {
        /// Read action path.
        action: Path,
        /// Typed request arguments.
        arguments: Vec<Argument>,
    },
    /// Immutable intent construction.
    Intent {
        /// Static purpose symbol.
        purpose: Path,
        /// Required intent fields.
        fields: Vec<FieldInitializer>,
    },
    /// Immutable write proposal construction.
    Propose {
        /// Write action path.
        action: Path,
        /// Typed action arguments.
        arguments: Vec<Argument>,
        /// Intent expression.
        intent: Box<Expression>,
    },
    /// Policy authorization.
    Authorize {
        /// Proposal expression.
        proposal: Box<Expression>,
        /// Policy symbol.
        policy: Path,
    },
    /// Write commit.
    Commit {
        /// Proposal expression.
        proposal: Box<Expression>,
        /// Matching permit expression.
        permit: Box<Expression>,
    },
    /// Receipt/observation reconciliation.
    Reconcile {
        /// Write receipt.
        receipt: Box<Expression>,
        /// Follow-up observation.
        observation: Box<Expression>,
        /// Two-parameter validator.
        validator: Path,
    },
}

/// Unary expression operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOperator {
    /// Boolean negation.
    Not,
    /// Numeric negation.
    Negate,
}

/// Binary expression operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOperator {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Equality.
    Equal,
    /// Inequality.
    NotEqual,
    /// Less than.
    Less,
    /// Less than or equal.
    LessEqual,
    /// Greater than.
    Greater,
    /// Greater than or equal.
    GreaterEqual,
    /// Boolean conjunction.
    And,
    /// Boolean disjunction.
    Or,
}

/// A positional or named call argument.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Argument {
    /// Present for `name = expression` arguments.
    pub name: Option<String>,
    /// Argument value.
    pub value: Expression,
    /// Exact argument span.
    pub span: Span,
}

/// A `name = expression` initializer.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FieldInitializer {
    /// Field name.
    pub name: String,
    /// Field value.
    pub value: Expression,
    /// Exact initializer span.
    pub span: Span,
}

/// One match arm.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatchArm {
    /// Restricted finite pattern.
    pub pattern: Pattern,
    /// Arm result.
    pub value: Expression,
    /// Exact arm span.
    pub span: Span,
}

/// Enum, option, result, or wildcard pattern.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "pattern", rename_all = "snake_case")]
pub enum Pattern {
    /// `_` catch-all.
    Wildcard,
    /// Variant path with optional single payload binding.
    Variant {
        /// Constructor path.
        path: Path,
        /// Optional payload binding.
        binding: Option<String>,
    },
}
