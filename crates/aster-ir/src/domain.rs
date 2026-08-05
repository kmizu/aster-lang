use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current persisted IR schema.
pub const IR_SCHEMA_VERSION: u32 = 1;

/// Versioned, deterministic executable ASTER program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Program {
    /// Persisted schema version.
    pub schema_version: u32,
    /// Compiler version that emitted this program.
    pub compiler_version: String,
    /// Source module identity.
    pub module_name: String,
    /// Hash of the normalized syntax tree.
    pub source_hash: String,
    /// Hash of all semantic IR content except this field.
    pub program_hash: String,
    /// Deterministically keyed executable routines.
    pub routines: BTreeMap<String, Routine>,
    /// Deterministically keyed agents and event entry points.
    pub agents: BTreeMap<String, Agent>,
    /// Typed declarations required at runtime boundaries.
    pub catalog: Catalog,
}

impl Program {
    /// Serializes the program using stable struct and `BTreeMap` ordering.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization unexpectedly fails.
    pub fn to_json(&self) -> Result<String, ProgramError> {
        serde_json::to_string(self).map_err(ProgramError::Serialization)
    }

    /// Deserializes and validates a persisted program.
    ///
    /// # Errors
    ///
    /// Rejects malformed JSON, unsupported schemas, and hash mismatches.
    pub fn from_json(json: &str) -> Result<Self, ProgramError> {
        let program: Self = serde_json::from_str(json).map_err(ProgramError::Serialization)?;
        program.validate()?;
        Ok(program)
    }

    /// Returns one agent handler routine.
    #[must_use]
    pub fn handler(&self, agent: &str, event: &str) -> Option<&Routine> {
        let key = self.agents.get(agent)?.handlers.get(event)?;
        self.routines.get(key)
    }

    /// Returns a source function or flow routine by unqualified name.
    #[must_use]
    pub fn routine(&self, name: &str) -> Option<&Routine> {
        self.routines
            .get(&format!("fn:{name}"))
            .or_else(|| self.routines.get(&format!("flow:{name}")))
            .or_else(|| self.routines.get(name))
    }

    pub(crate) fn seal(&mut self) -> Result<(), ProgramError> {
        self.program_hash.clear();
        self.program_hash = self.content_hash()?;
        Ok(())
    }

    fn validate(&self) -> Result<(), ProgramError> {
        if self.schema_version != IR_SCHEMA_VERSION {
            return Err(ProgramError::UnsupportedSchema(self.schema_version));
        }
        let expected = self.content_hash()?;
        if expected != self.program_hash {
            return Err(ProgramError::HashMismatch);
        }
        Ok(())
    }

    fn content_hash(&self) -> Result<String, ProgramError> {
        let mut unhashed = self.clone();
        unhashed.program_hash.clear();
        let bytes = serde_json::to_vec(&unhashed).map_err(ProgramError::Serialization)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

/// Controlled persisted-program validation failures.
#[derive(Debug, Error)]
pub enum ProgramError {
    /// JSON encoding or decoding failed.
    #[error("IR serialization failed: {0}")]
    Serialization(serde_json::Error),
    /// The schema cannot be executed by this build.
    #[error("unsupported IR schema version {0}")]
    UnsupportedSchema(u32),
    /// Persisted semantic content differs from its claimed identity.
    #[error("IR program hash mismatch")]
    HashMismatch,
}

/// One serializable routine with explicit control flow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Routine {
    /// Stable program-local identity.
    pub name: String,
    /// Typed parameters in source order.
    pub parameters: Vec<FieldSpec>,
    /// Declared return type.
    pub return_type: TypeSpec,
    /// Explicit instruction stream.
    pub instructions: Vec<Instruction>,
}

/// One durable agent entry-point table.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Agent {
    /// Agent constructor parameters.
    pub parameters: Vec<FieldSpec>,
    /// Persistent state schema and pure defaults.
    pub state: Vec<StateFieldSpec>,
    /// Fixed per-event budget limits; omitted dimensions are zero.
    pub budget: BTreeMap<String, u64>,
    /// Event name to routine identity.
    pub handlers: BTreeMap<String, String>,
    /// Declared capability requirements.
    pub capabilities: Vec<CapabilitySpec>,
}

/// Runtime-relevant declaration catalog.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Catalog {
    /// Non-generic aliases expanded at external decode boundaries.
    pub aliases: BTreeMap<String, TypeSpec>,
    /// Record schemas.
    pub records: BTreeMap<String, Vec<FieldSpec>>,
    /// Static prompts.
    pub prompts: BTreeMap<String, PromptSpec>,
    /// Tool boundaries.
    pub tools: BTreeMap<String, ToolSpec>,
    /// Pure validators.
    pub validators: BTreeMap<String, ValidatorSpec>,
    /// Ordered total policies.
    pub policies: BTreeMap<String, PolicySpec>,
}

/// A structural type carried by IR signatures and schemas.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TypeSpec {
    /// Constructor or declared type name.
    pub name: String,
    /// Ordered generic arguments.
    pub arguments: Vec<TypeSpec>,
}

/// A named typed field or parameter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FieldSpec {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: TypeSpec,
}

/// Persistent field and its effect-free default computation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateFieldSpec {
    /// Field name.
    pub name: String,
    /// Persisted field type.
    pub ty: TypeSpec,
    /// Pure default expression.
    pub default: PureExpression,
}

/// Static prompt metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptSpec {
    /// Structured data parameters.
    pub parameters: Vec<FieldSpec>,
    /// Decoded candidate type.
    pub result_type: TypeSpec,
    /// Static instruction bytes.
    pub instruction: String,
    /// Admitted structured-data names.
    pub data: Vec<String>,
}

/// Tool request and trust-boundary metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolSpec {
    /// Request schema.
    pub parameters: Vec<FieldSpec>,
    /// Response schema.
    pub result_type: TypeSpec,
    /// Read or write boundary.
    pub mode: ToolMode,
    /// Required capability request template.
    pub capability: Option<CapabilitySpec>,
    /// Optional write idempotency parameter.
    pub idempotency: Option<String>,
    /// Declared risk spelling.
    pub risk: Option<String>,
    /// Declared sensitivity spelling.
    pub sensitivity: Option<String>,
}

/// Tool effect mode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    /// Observation-only tool.
    Read,
    /// Governed mutating tool.
    Write,
}

/// Capability kind plus pure argument expressions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilitySpec {
    /// Capability declaration identity.
    pub name: String,
    /// Scope arguments.
    pub arguments: Vec<NamedExpression>,
}

/// Pure validator requirements.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ValidatorSpec {
    /// Typed validator inputs.
    pub parameters: Vec<FieldSpec>,
    /// Conjunctive pure requirements.
    pub requirements: Vec<PureExpression>,
}

/// Ordered total authorization policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicySpec {
    /// Typed policy inputs.
    pub parameters: Vec<FieldSpec>,
    /// Source-ordered decisions.
    pub rules: Vec<PolicyRuleSpec>,
}

/// One pure policy rule.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PolicyRuleSpec {
    /// Decision on a matching condition.
    pub decision: PolicyDecisionSpec,
    /// `None` is the final otherwise rule.
    pub condition: Option<PureExpression>,
}

/// Pure authorization decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PolicyDecisionSpec {
    /// Issue a permit directly.
    Allow,
    /// Suspend for approval by the evaluated principal.
    Approve(PureExpression),
    /// Reject with a safe reason.
    Deny(PureExpression),
}

/// Stable zero-based instruction identity within one routine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct InstructionId(u32);

impl InstructionId {
    /// Returns the stable routine-local index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Stable value slot identity within one frame.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ValueId(pub u32);

/// One explicit VM instruction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Instruction {
    /// Routine-local stable identity.
    pub id: InstructionId,
    /// Instruction behavior.
    pub kind: InstructionKind,
}

impl Instruction {
    pub(crate) fn new(index: u32, kind: InstructionKind) -> Self {
        Self {
            id: InstructionId(index),
            kind,
        }
    }
}

/// Explicit control, pure evaluation, and governed effect operations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InstructionKind {
    /// Evaluate one effect-free operation into a slot.
    Evaluate {
        target: ValueId,
        expression: PureExpression,
    },
    /// Bind an immutable source local to a slot.
    Bind { name: String, value: ValueId },
    /// Call a statically resolved source routine.
    Call {
        target: ValueId,
        routine: String,
        arguments: Vec<NamedValue>,
    },
    /// Propagate a typed `Result` error or expose its success value.
    UnwrapResult { target: ValueId, result: ValueId },
    /// Conditional jump with explicit targets.
    Branch {
        condition: ValueId,
        then_target: u32,
        else_target: u32,
    },
    /// Unconditional jump.
    Jump { target: u32 },
    /// Finite pattern dispatch.
    Match {
        value: ValueId,
        arms: Vec<MatchTarget>,
    },
    /// Model inference suspension request.
    Infer {
        target: ValueId,
        prompt: String,
        arguments: Vec<NamedValue>,
        model_alias: String,
    },
    /// Pure candidate validation.
    Validate {
        target: ValueId,
        candidate: ValueId,
        validator: String,
    },
    /// Read-tool suspension request.
    Observe {
        target: ValueId,
        action: String,
        arguments: Vec<NamedValue>,
    },
    /// Immutable intent construction.
    ConstructIntent {
        target: ValueId,
        purpose: String,
        fields: Vec<NamedValue>,
    },
    /// Immutable proposal construction.
    ConstructProposal {
        target: ValueId,
        action: String,
        arguments: Vec<NamedValue>,
        intent: ValueId,
    },
    /// Pure policy evaluation which may explicitly suspend for approval.
    Authorize {
        target: ValueId,
        proposal: ValueId,
        policy: String,
        approval_may_suspend: bool,
    },
    /// Governed write-tool suspension request.
    Commit {
        target: ValueId,
        proposal: ValueId,
        permit: ValueId,
    },
    /// Pure post-write world-state reconciliation.
    Reconcile {
        target: ValueId,
        receipt: ValueId,
        observation: ValueId,
        validator: String,
    },
    /// Runtime assertion.
    Require { condition: ValueId },
    /// Pending atomic state delta update.
    UpdateState { fields: Vec<NamedValue> },
    /// Routine completion.
    Return { value: ValueId },
}

impl InstructionKind {
    /// Returns the governed semantic stage exposed by this instruction.
    #[must_use]
    pub const fn stage(&self) -> Option<Stage> {
        match self {
            Self::Infer { .. } => Some(Stage::Inference),
            Self::Validate { .. } => Some(Stage::Validation),
            Self::Observe { .. } => Some(Stage::Observation),
            Self::ConstructIntent { .. } => Some(Stage::Intent),
            Self::ConstructProposal { .. } => Some(Stage::Proposal),
            Self::Authorize { .. } => Some(Stage::Authorization),
            Self::Commit { .. } => Some(Stage::Commit),
            Self::Reconcile { .. } => Some(Stage::Reconciliation),
            Self::UpdateState { .. } => Some(Stage::StateUpdate),
            Self::Return { .. } => Some(Stage::Return),
            _ => None,
        }
    }
}

/// Auditable governed stages.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Inference,
    Validation,
    Observation,
    Intent,
    Proposal,
    Authorization,
    Commit,
    Reconciliation,
    StateUpdate,
    Return,
}

/// An argument already evaluated to a value slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedValue {
    /// Optional source argument label.
    pub name: Option<String>,
    /// Evaluated slot.
    pub value: ValueId,
}

/// A named effect-free expression, used in metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct NamedExpression {
    pub name: Option<String>,
    pub value: PureExpression,
}

/// Finite branch target for a match arm.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchTarget {
    pub pattern: PatternSpec,
    pub target: u32,
}

/// Serializable finite pattern.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatternSpec {
    Wildcard,
    Variant {
        path: String,
        binding: Option<String>,
    },
}

/// Effect-free expression tree. It cannot encode model, tool, policy, commit,
/// or reconciliation effects by construction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PureExpression {
    Unit,
    Bool {
        value: bool,
    },
    Int {
        value: i64,
    },
    Text {
        value: String,
    },
    Path {
        path: String,
    },
    List {
        elements: Vec<PureExpression>,
    },
    Record {
        ty: String,
        fields: Vec<NamedExpression>,
    },
    Field {
        target: Box<PureExpression>,
        field: String,
    },
    Unary {
        operator: String,
        operand: Box<PureExpression>,
    },
    Binary {
        left: Box<PureExpression>,
        operator: String,
        right: Box<PureExpression>,
    },
    Call {
        function: String,
        arguments: Vec<NamedExpression>,
    },
    Slot {
        value: ValueId,
    },
}
