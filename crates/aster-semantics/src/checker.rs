use std::collections::{BTreeMap, BTreeSet};

use aster_diagnostics::{Diagnostic, KnownDiagnosticCode};
use aster_syntax::{
    AgentDeclaration, Block, CapabilityExpression, DeclarationKind, Expression, ExpressionKind,
    FunctionDeclaration, Module, PolicyDecision, Sensitivity, SourceFile, StatementKind,
    ToolDeclaration, ToolMode, TypeDefinition, TypeReference, parse,
};

use crate::{
    Type,
    expression::{CheckContext, ExpressionChecker, environment_from_parameters},
    model::Model,
};

const BUDGET_DIMENSIONS: [&str; 6] = [
    "model_calls",
    "model_tokens",
    "external_reads",
    "external_writes",
    "approvals",
    "money_microunits",
];

/// A module that passed ASTER's deterministic static checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedProgram {
    module: Module,
}

impl CheckedProgram {
    /// Returns the checked module's dotted identity.
    #[must_use]
    pub fn module_name(&self) -> String {
        self.module.name.as_string()
    }

    /// Returns agent names in deterministic source order.
    #[must_use]
    pub fn agent_names(&self) -> Vec<&str> {
        self.module
            .declarations
            .iter()
            .filter_map(|declaration| match &declaration.kind {
                DeclarationKind::Agent(agent) => Some(agent.name.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Borrows the checked syntax tree.
    #[must_use]
    pub const fn module(&self) -> &Module {
        &self.module
    }
}

/// Parses and statically checks one source file.
///
/// # Errors
///
/// Returns stable parse or semantic diagnostics and never a partially trusted
/// program when any error is present.
pub fn check_source(source: &SourceFile) -> Result<CheckedProgram, Vec<Diagnostic>> {
    parse(source).and_then(check)
}

/// Statically checks an already parsed module.
///
/// # Errors
///
/// Returns every deterministically discovered semantic diagnostic.
pub fn check(module: Module) -> Result<CheckedProgram, Vec<Diagnostic>> {
    let model = Model::new(&module);
    let mut diagnostics = Vec::new();

    check_duplicate_declarations(&module, &mut diagnostics);
    check_type_well_formedness(&module, &model, &mut diagnostics);
    check_secret_type_placement(&module, &model, &mut diagnostics);
    check_capability_requests(&module, &model, &mut diagnostics);
    check_declaration_metadata(&module, &model, &mut diagnostics);
    check_recursion(&module, &mut diagnostics);
    check_executable_bodies(&module, &model, &mut diagnostics);

    if diagnostics.is_empty() {
        Ok(CheckedProgram { module })
    } else {
        diagnostics.sort_by(|left, right| {
            left.primary_span
                .start
                .cmp(&right.primary_span.start)
                .then_with(|| left.code.cmp(&right.code))
        });
        Err(diagnostics)
    }
}

fn check_type_well_formedness(
    module: &Module,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Type(value) => match &value.definition {
                TypeDefinition::Alias(ty) => {
                    check_type_reference(ty, model, diagnostics);
                    if alias_reaches(&value.name, &value.name, model, &mut BTreeSet::new()) {
                        diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::TypeMismatch.into(),
                                format!("cyclic type alias `{}`", value.name),
                                ty.span.clone(),
                            )
                            .with_help("break the alias cycle with a concrete declared type"),
                        );
                    }
                }
                TypeDefinition::Record(fields) => {
                    check_duplicate_names(
                        fields.iter().map(|field| (&field.name, &field.span)),
                        "record field",
                        diagnostics,
                    );
                    for field in fields {
                        check_type_reference(&field.ty, model, diagnostics);
                    }
                }
            },
            DeclarationKind::Enum(value) => {
                check_duplicate_names(
                    value
                        .variants
                        .iter()
                        .map(|variant| (&variant.name, &variant.span)),
                    "enum variant",
                    diagnostics,
                );
                for variant in &value.variants {
                    if let Some(payload) = &variant.payload {
                        check_type_reference(payload, model, diagnostics);
                    }
                }
            }
            DeclarationKind::Capability(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Function(value) | DeclarationKind::Flow(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
                check_type_reference(&value.return_type, model, diagnostics);
            }
            DeclarationKind::Prompt(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
                check_type_reference(&value.result_type, model, diagnostics);
            }
            DeclarationKind::Validator(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Tool(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
                check_type_reference(&value.return_type, model, diagnostics);
            }
            DeclarationKind::Policy(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Agent(value) => {
                check_parameter_types(&value.parameters, model, diagnostics);
                check_duplicate_names(
                    value.state.iter().map(|field| (&field.name, &field.span)),
                    "state field",
                    diagnostics,
                );
                check_duplicate_names(
                    value
                        .handlers
                        .iter()
                        .map(|handler| (&handler.event, &handler.span)),
                    "event handler",
                    diagnostics,
                );
                for field in &value.state {
                    check_type_reference(&field.ty, model, diagnostics);
                }
                for handler in &value.handlers {
                    check_parameter_types(&handler.parameters, model, diagnostics);
                    check_type_reference(&handler.return_type, model, diagnostics);
                }
            }
        }
    }
}

fn check_secret_type_placement(
    module: &Module,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Type(value) => match &value.definition {
                TypeDefinition::Alias(ty) => {
                    reject_secret_outside_tool_parameter(ty, model, diagnostics);
                }
                TypeDefinition::Record(fields) => {
                    for field in fields {
                        reject_secret_outside_tool_parameter(&field.ty, model, diagnostics);
                    }
                }
            },
            DeclarationKind::Enum(value) => {
                for payload in value
                    .variants
                    .iter()
                    .filter_map(|variant| variant.payload.as_ref())
                {
                    reject_secret_outside_tool_parameter(payload, model, diagnostics);
                }
            }
            DeclarationKind::Capability(value) => {
                reject_secret_parameters(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Function(value) | DeclarationKind::Flow(value) => {
                reject_secret_parameters(&value.parameters, model, diagnostics);
                reject_secret_outside_tool_parameter(&value.return_type, model, diagnostics);
            }
            DeclarationKind::Prompt(_) => {}
            DeclarationKind::Validator(value) => {
                reject_secret_parameters(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Tool(value) => {
                if value.metadata.sensitivity != Some(Sensitivity::Secret) {
                    reject_secret_parameters(&value.parameters, model, diagnostics);
                }
                reject_secret_outside_tool_parameter(&value.return_type, model, diagnostics);
            }
            DeclarationKind::Policy(value) => {
                reject_secret_parameters(&value.parameters, model, diagnostics);
            }
            DeclarationKind::Agent(value) => {
                reject_secret_parameters(&value.parameters, model, diagnostics);
                for handler in &value.handlers {
                    reject_secret_parameters(&handler.parameters, model, diagnostics);
                    reject_secret_outside_tool_parameter(&handler.return_type, model, diagnostics);
                }
            }
        }
    }
}

fn reject_secret_parameters(
    parameters: &[aster_syntax::Parameter],
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        reject_secret_outside_tool_parameter(&parameter.ty, model, diagnostics);
    }
}

fn reject_secret_outside_tool_parameter(
    reference: &TypeReference,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if model.contains_secret(&model.resolve_type(reference)) {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::SecretInState.into(),
                "Secret types are restricted to sensitivity-secret tool parameters",
                reference.span.clone(),
            )
            .with_help("move the Secret handle to a tool parameter and mark the tool secret"),
        );
    }
}

fn check_parameter_types(
    parameters: &[aster_syntax::Parameter],
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_duplicate_names(
        parameters
            .iter()
            .map(|parameter| (&parameter.name, &parameter.span)),
        "parameter",
        diagnostics,
    );
    for parameter in parameters {
        check_type_reference(&parameter.ty, model, diagnostics);
    }
}

fn alias_reaches(
    current: &str,
    target: &str,
    model: &Model<'_>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if !visited.insert(current.to_owned()) {
        return false;
    }
    let result = model.types.get(current).is_some_and(|declaration| {
        let TypeDefinition::Alias(reference) = &declaration.definition else {
            return false;
        };
        type_reference_reaches(reference, target, model, visited)
    });
    visited.remove(current);
    result
}

fn type_reference_reaches(
    reference: &TypeReference,
    target: &str,
    model: &Model<'_>,
    visited: &mut BTreeSet<String>,
) -> bool {
    let name = reference.path.as_string();
    name == target
        || reference
            .arguments
            .iter()
            .any(|argument| type_reference_reaches(argument, target, model, visited))
        || (model.types.contains_key(&name) && alias_reaches(&name, target, model, visited))
}

fn check_type_reference(
    reference: &TypeReference,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = reference.path.as_string();
    let expected_arity = match name.as_str() {
        "Unit" | "Bool" | "Int" | "Text" | "Instant" | "Duration" | "ProvenanceRef" | "Error" => {
            Some(0)
        }
        "Option" | "List" | "Incoming" | "Untrusted" | "Candidate" | "Checked" | "Observation"
        | "Secret" | "Intent" | "Proposal" | "Permit" | "Receipt" | "Reconciled" => Some(1),
        "Result" => Some(2),
        _ if reference.arguments.is_empty()
            && (model.types.contains_key(&name)
                || model.enums.contains_key(&name)
                || (name.ends_with(".State")
                    && model.agents.contains_key(name.trim_end_matches(".State")))) =>
        {
            Some(0)
        }
        _ => None,
    };
    let Some(expected_arity) = expected_arity else {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::UnknownName.into(),
                format!("unknown type `{name}`"),
                reference.span.clone(),
            )
            .with_help("declare the type or use a documented ASTER 0.1 type"),
        );
        return;
    };
    if reference.arguments.len() != expected_arity {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                format!(
                    "type `{name}` expects {expected_arity} argument(s), found {}",
                    reference.arguments.len()
                ),
                reference.span.clone(),
            )
            .with_help("use the declared type constructor arity"),
        );
        return;
    }
    if !matches!(
        name.as_str(),
        "Intent" | "Proposal" | "Permit" | "Receipt" | "Reconciled"
    ) {
        for argument in &reference.arguments {
            check_type_reference(argument, model, diagnostics);
        }
    } else if matches!(
        name.as_str(),
        "Proposal" | "Permit" | "Receipt" | "Reconciled"
    ) {
        let action = reference.arguments[0].path.as_string();
        if model
            .tools
            .get(&action)
            .is_none_or(|tool| tool.metadata.mode != Some(ToolMode::Write))
        {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::UnknownName.into(),
                    format!("governance action `{action}` is not a declared write tool"),
                    reference.arguments[0].span.clone(),
                )
                .with_help("use the path of a declared write-mode tool"),
            );
        }
    }
}

fn check_duplicate_names<'a>(
    values: impl IntoIterator<Item = (&'a String, &'a aster_diagnostics::Span)>,
    identity: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen = BTreeSet::new();
    for (name, span) in values {
        if !seen.insert(name.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::DuplicateDeclaration.into(),
                    format!("duplicate {identity} `{name}`"),
                    span.clone(),
                )
                .with_help(format!("keep one {identity} with this name")),
            );
        }
    }
}

fn check_duplicate_declarations(module: &Module, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen = BTreeSet::new();
    for declaration in &module.declarations {
        let identity = declaration_identity(&declaration.kind);
        let shadows_builtin = match &declaration.kind {
            DeclarationKind::Type(value) => is_builtin_type_name(&value.name),
            DeclarationKind::Enum(value) => is_builtin_type_name(&value.name),
            _ => false,
        };
        if shadows_builtin {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::DuplicateDeclaration.into(),
                    format!("built-in type cannot be shadowed by `{identity}`"),
                    declaration.span.clone(),
                )
                .with_help("rename the user-defined type"),
            );
        }
        if !seen.insert(identity.clone()) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::DuplicateDeclaration.into(),
                    format!("duplicate declaration `{identity}`"),
                    declaration.span.clone(),
                )
                .with_help("rename or remove the later declaration"),
            );
        }
    }
}

fn is_builtin_type_name(name: &str) -> bool {
    matches!(
        name,
        "Unit"
            | "Bool"
            | "Int"
            | "Text"
            | "Instant"
            | "Duration"
            | "ProvenanceRef"
            | "Error"
            | "Option"
            | "Result"
            | "List"
            | "Incoming"
            | "Untrusted"
            | "Candidate"
            | "Checked"
            | "Observation"
            | "Secret"
            | "Intent"
            | "Proposal"
            | "Permit"
            | "Receipt"
            | "Reconciled"
    )
}

fn declaration_identity(declaration: &DeclarationKind) -> String {
    match declaration {
        DeclarationKind::Type(value) => format!("type: {}", value.name),
        DeclarationKind::Enum(value) => format!("type: {}", value.name),
        DeclarationKind::Capability(value) => format!("capability: {}", value.name),
        DeclarationKind::Function(value) | DeclarationKind::Flow(value) => {
            format!("callable: {}", value.name)
        }
        DeclarationKind::Prompt(value) => format!("prompt: {}", value.name),
        DeclarationKind::Validator(value) => format!("validator: {}", value.name),
        DeclarationKind::Tool(value) => format!("tool: {}", value.path.as_string()),
        DeclarationKind::Policy(value) => format!("policy: {}", value.name),
        DeclarationKind::Agent(value) => format!("agent: {}", value.name),
    }
}

fn check_capability_requests(
    module: &Module,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &module.declarations {
        let (parameters, capabilities): (_, &[CapabilityExpression]) = match &declaration.kind {
            DeclarationKind::Function(function) | DeclarationKind::Flow(function) => {
                (&function.parameters, &function.uses)
            }
            DeclarationKind::Tool(tool) => (&tool.parameters, tool.metadata.capability.as_slice()),
            DeclarationKind::Agent(agent) => (&agent.parameters, &agent.requires),
            _ => continue,
        };
        let mut environment = environment_from_parameters(model, parameters);
        for capability in capabilities {
            check_capability_request(capability, &mut environment, model, diagnostics);
        }
    }
}

fn check_declaration_metadata(
    module: &Module,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Prompt(prompt) => {
                let mut data = BTreeSet::new();
                let parameters: BTreeSet<_> = prompt
                    .parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect();
                for name in &prompt.data {
                    if !data.insert(name.as_str()) {
                        diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::DuplicateDeclaration.into(),
                                format!("duplicate prompt data name `{name}`"),
                                declaration.span.clone(),
                            )
                            .with_help("list each prompt parameter exactly once"),
                        );
                    }
                    if !parameters.contains(name.as_str()) {
                        diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::UnknownName.into(),
                                format!("prompt data name `{name}` is not a parameter"),
                                declaration.span.clone(),
                            )
                            .with_help("list only declared prompt parameters"),
                        );
                    }
                }
                for parameter in parameters.difference(&data) {
                    diagnostics.push(
                        Diagnostic::error(
                            KnownDiagnosticCode::TypeMismatch.into(),
                            format!("prompt data omits parameter `{parameter}`"),
                            declaration.span.clone(),
                        )
                        .with_help("list every prompt parameter exactly once"),
                    );
                }
                for parameter in &prompt.parameters {
                    if model.contains_secret(&model.resolve_type(&parameter.ty)) {
                        diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::SecretToModel.into(),
                                "prompt data cannot contain Secret values",
                                parameter.span.clone(),
                            )
                            .with_help("pass a non-secret validated summary"),
                        );
                    }
                }
            }
            DeclarationKind::Validator(validator)
                if !(1..=2).contains(&validator.parameters.len()) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::TypeMismatch.into(),
                        "validator must accept one validation value or two reconciliation values",
                        declaration.span.clone(),
                    )
                    .with_help("declare exactly one or two validator parameters"),
                );
            }
            DeclarationKind::Tool(tool) => {
                check_tool_metadata(tool, &declaration.span, model, diagnostics);
            }
            DeclarationKind::Policy(policy)
                if policy
                    .rules
                    .last()
                    .is_none_or(|rule| rule.condition.is_some()) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::NonTotalPolicy.into(),
                        "policy has no final otherwise rule",
                        declaration.span.clone(),
                    )
                    .with_help("end the policy with an `otherwise` decision"),
                );
                check_policy_signature(policy, &declaration.span, model, diagnostics);
            }
            DeclarationKind::Policy(policy) => {
                check_policy_signature(policy, &declaration.span, model, diagnostics);
            }
            DeclarationKind::Agent(agent) => check_agent_metadata(agent, model, diagnostics),
            _ => {}
        }
    }
}

fn check_policy_signature(
    policy: &aster_syntax::PolicyDeclaration,
    span: &aster_diagnostics::Span,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !(1..=2).contains(&policy.parameters.len()) {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                "policy must accept a proposal and at most one agent state snapshot",
                span.clone(),
            )
            .with_help("declare `(proposal: Proposal<Action>)` with optional `state: Agent.State`"),
        );
    }
    if policy
        .parameters
        .first()
        .is_some_and(|parameter| !matches!(model.resolve_type(&parameter.ty), Type::Proposal(_)))
    {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                "policy first parameter must be Proposal<Action>",
                policy.parameters[0].span.clone(),
            )
            .with_help("use the governed write action's proposal type"),
        );
    }
    if policy
        .parameters
        .get(1)
        .is_some_and(|parameter| !matches!(model.resolve_type(&parameter.ty), Type::AgentState(_)))
    {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                "policy second parameter must be Agent.State",
                policy.parameters[1].span.clone(),
            )
            .with_help("use the authorizing agent's immutable state type"),
        );
    }
}

fn check_tool_metadata(
    tool: &ToolDeclaration,
    declaration_span: &aster_diagnostics::Span,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if tool.metadata.mode.is_none() {
        missing_tool_metadata("mode", declaration_span, diagnostics);
    }
    if tool.metadata.capability.is_none() {
        missing_tool_metadata("capability", declaration_span, diagnostics);
    }
    if tool.metadata.sensitivity.is_none() {
        missing_tool_metadata("sensitivity", declaration_span, diagnostics);
    }
    if tool.metadata.mode == Some(ToolMode::Write) {
        if tool.metadata.risk.is_none() {
            missing_tool_metadata("risk", declaration_span, diagnostics);
        }
        match tool.metadata.idempotency.as_deref() {
            None => diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::MissingIdempotency.into(),
                    "write tool is missing idempotency metadata",
                    declaration_span.clone(),
                )
                .with_help("name a deterministic request parameter with `idempotency`"),
            ),
            Some(name)
                if !tool
                    .parameters
                    .iter()
                    .any(|parameter| parameter.name == name) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::MissingIdempotency.into(),
                        format!("idempotency parameter `{name}` is not declared"),
                        declaration_span.clone(),
                    )
                    .with_help("name one of the write tool's declared parameters"),
                );
            }
            Some(name) => {
                let parameter = tool
                    .parameters
                    .iter()
                    .find(|parameter| parameter.name == name);
                if parameter.is_some_and(|parameter| {
                    !model.is_deterministically_serializable(&model.resolve_type(&parameter.ty))
                }) {
                    diagnostics.push(
                        Diagnostic::error(
                            KnownDiagnosticCode::MissingIdempotency.into(),
                            format!(
                                "idempotency parameter `{name}` is not deterministically serializable"
                            ),
                            declaration_span.clone(),
                        )
                        .with_help("use an ordinary deterministic data value as the key"),
                    );
                }
            }
        }
    } else if tool.metadata.mode == Some(ToolMode::Read) {
        if tool.metadata.risk.is_some() {
            unexpected_tool_metadata("risk", declaration_span, diagnostics);
        }
        if tool.metadata.idempotency.is_some() {
            unexpected_tool_metadata("idempotency", declaration_span, diagnostics);
        }
    }
}

fn check_capability_request(
    capability: &CapabilityExpression,
    environment: &mut crate::expression::Environment,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = capability.path.as_string();
    let Some(declaration) = model.capabilities.get(&name) else {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::UnknownName.into(),
                format!("unknown capability `{name}`"),
                capability.path.span.clone(),
            )
            .with_help("declare the capability before requiring it"),
        );
        return;
    };
    let context = pure_context(Type::Unit);
    ExpressionChecker::new(model, diagnostics).check_arguments(
        &declaration.parameters,
        &capability.arguments,
        environment,
        &context,
        &capability.span,
    );
}

fn missing_tool_metadata(
    name: &str,
    span: &aster_diagnostics::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(
        Diagnostic::error(
            KnownDiagnosticCode::TypeMismatch.into(),
            format!("tool is missing required `{name}` metadata"),
            span.clone(),
        )
        .with_help(format!("declare exactly one `{name}` metadata entry")),
    );
}

fn unexpected_tool_metadata(
    name: &str,
    span: &aster_diagnostics::Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(
        Diagnostic::error(
            KnownDiagnosticCode::TypeMismatch.into(),
            format!("read tool cannot declare write-only `{name}` metadata"),
            span.clone(),
        )
        .with_help(format!("remove the `{name}` metadata entry")),
    );
}

fn check_agent_metadata(
    agent: &AgentDeclaration,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in &agent.state {
        if model.contains_secret(&model.resolve_type(&field.ty)) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::SecretInState.into(),
                    "persistent agent state cannot contain Secret values",
                    field.ty.span.clone(),
                )
                .with_help("keep secrets behind sensitivity-secret tool boundaries"),
            );
        }
    }

    let mut seen = BTreeSet::new();
    for entry in &agent.budget {
        if !BUDGET_DIMENSIONS.contains(&entry.dimension.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::UnknownBudgetDimension.into(),
                    format!("unknown budget dimension `{}`", entry.dimension),
                    entry.span.clone(),
                )
                .with_help("use a fixed ASTER 0.1 budget dimension"),
            );
        } else if !seen.insert(entry.dimension.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::DuplicateBudgetDimension.into(),
                    format!("duplicate budget dimension `{}`", entry.dimension),
                    entry.span.clone(),
                )
                .with_help("declare each budget dimension at most once"),
            );
        }
    }
}

fn check_executable_bodies(module: &Module, model: &Model<'_>, diagnostics: &mut Vec<Diagnostic>) {
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Function(function) => {
                check_function(function, true, None, model, diagnostics);
            }
            DeclarationKind::Flow(flow) => {
                check_function(flow, false, None, model, diagnostics);
            }
            DeclarationKind::Validator(validator) => {
                let mut environment = environment_from_parameters(model, &validator.parameters);
                let context = pure_context(Type::Unit);
                let mut checker = ExpressionChecker::new(model, diagnostics);
                for requirement in &validator.requirements {
                    let actual = checker.check_expression(requirement, &mut environment, &context);
                    checker.expect_type(&Type::Bool, &actual, &requirement.span);
                }
            }
            DeclarationKind::Policy(policy) => {
                let mut environment = environment_from_parameters(model, &policy.parameters);
                let context = pure_context(Type::Unit);
                let mut checker = ExpressionChecker::new(model, diagnostics);
                for rule in &policy.rules {
                    if let Some(condition) = &rule.condition {
                        let actual =
                            checker.check_expression(condition, &mut environment, &context);
                        checker.expect_type(&Type::Bool, &actual, &condition.span);
                    }
                    match &rule.decision {
                        PolicyDecision::Approve(value) | PolicyDecision::Deny(value) => {
                            let actual =
                                checker.check_expression(value, &mut environment, &context);
                            checker.expect_type(&Type::Text, &actual, &value.span);
                        }
                        PolicyDecision::Allow => {}
                    }
                }
            }
            DeclarationKind::Agent(agent) => {
                check_agent_bodies(agent, model, diagnostics);
            }
            _ => {}
        }
    }
}

fn check_function(
    function: &FunctionDeclaration,
    pure: bool,
    agent: Option<String>,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(
        function
            .body
            .statements
            .last()
            .map(|statement| &statement.kind),
        Some(StatementKind::Return { .. })
    ) {
        diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                format!("callable `{}` has no final return statement", function.name),
                function.body.span.clone(),
            )
            .with_help("end every function or flow with an explicit `return`"),
        );
    }
    let mut environment = environment_from_parameters(model, &function.parameters);
    let context = CheckContext {
        pure,
        allowed_capabilities: function
            .uses
            .iter()
            .map(|capability| capability.path.as_string())
            .collect(),
        return_type: model.resolve_type(&function.return_type),
        agent,
    };
    ExpressionChecker::new(model, diagnostics).check_block(
        &function.body,
        &mut environment,
        &context,
    );
}

fn check_agent_bodies(
    agent: &AgentDeclaration,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut defaults = environment_from_parameters(model, &agent.parameters);
    defaults.insert("event", Type::Event);
    let default_context = CheckContext {
        pure: true,
        allowed_capabilities: BTreeSet::new(),
        return_type: Type::Unit,
        agent: Some(agent.name.clone()),
    };
    let mut checker = ExpressionChecker::new(model, diagnostics);
    for field in &agent.state {
        let actual = checker.check_expression(&field.default, &mut defaults, &default_context);
        checker.expect_type(&model.resolve_type(&field.ty), &actual, &field.default.span);
    }

    for handler in &agent.handlers {
        if !matches!(
            handler
                .body
                .statements
                .last()
                .map(|statement| &statement.kind),
            Some(StatementKind::Return { .. })
        ) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::TypeMismatch.into(),
                    format!("handler `{}` has no final return statement", handler.event),
                    handler.body.span.clone(),
                )
                .with_help("end every event handler with an explicit `return`"),
            );
        }
        let mut environment = environment_from_parameters(model, &agent.parameters);
        for parameter in &handler.parameters {
            environment.insert(&parameter.name, model.resolve_type(&parameter.ty));
        }
        environment.insert("self", Type::AgentState(agent.name.clone()));
        environment.insert("event", Type::Event);
        let context = CheckContext {
            pure: false,
            allowed_capabilities: agent
                .requires
                .iter()
                .map(|capability| capability.path.as_string())
                .collect(),
            return_type: model.resolve_type(&handler.return_type),
            agent: Some(agent.name.clone()),
        };
        ExpressionChecker::new(model, diagnostics).check_block(
            &handler.body,
            &mut environment,
            &context,
        );
    }
}

fn pure_context(return_type: Type) -> CheckContext {
    CheckContext {
        pure: true,
        allowed_capabilities: BTreeSet::new(),
        return_type,
        agent: None,
    }
}

fn check_recursion(module: &Module, diagnostics: &mut Vec<Diagnostic>) {
    let declarations: BTreeMap<_, _> = module
        .declarations
        .iter()
        .filter_map(|declaration| match &declaration.kind {
            DeclarationKind::Function(function) | DeclarationKind::Flow(function) => {
                Some((function.name.as_str(), (function, declaration)))
            }
            _ => None,
        })
        .collect();
    let names: BTreeSet<_> = declarations.keys().copied().collect();
    let graph: BTreeMap<_, _> = declarations
        .iter()
        .map(|(name, (function, _))| {
            let mut calls = BTreeSet::new();
            collect_block_calls(&function.body, &names, &mut calls);
            (*name, calls)
        })
        .collect();

    for (name, (_, declaration)) in declarations {
        if reaches(name, name, &graph, &mut BTreeSet::new()) {
            diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::Recursion.into(),
                    format!("recursive call cycle includes `{name}`"),
                    declaration.span.clone(),
                )
                .with_help("replace recursion with finite non-recursive computation"),
            );
        }
    }
}

fn reaches<'a>(
    current: &'a str,
    target: &str,
    graph: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if !visited.insert(current) {
        return false;
    }
    graph.get(current).is_some_and(|next| {
        next.contains(target)
            || next
                .iter()
                .any(|name| reaches(name, target, graph, visited))
    })
}

fn collect_block_calls<'a>(
    block: &'a Block,
    names: &BTreeSet<&str>,
    calls: &mut BTreeSet<&'a str>,
) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let { value, .. } | StatementKind::Return { value } => {
                collect_expression_calls(value, names, calls);
            }
            StatementKind::Require { condition } => {
                collect_expression_calls(condition, names, calls);
            }
            StatementKind::UpdateState { fields } => {
                for field in fields {
                    collect_expression_calls(&field.value, names, calls);
                }
            }
            StatementKind::Expression { expression } => {
                collect_expression_calls(expression, names, calls);
            }
        }
    }
}

fn collect_expression_calls<'a>(
    expression: &'a Expression,
    names: &BTreeSet<&str>,
    calls: &mut BTreeSet<&'a str>,
) {
    match &expression.kind {
        ExpressionKind::Call { callee, arguments } => {
            if let ExpressionKind::Path { path } = &callee.kind
                && path.segments.len() == 1
            {
                let name = path.segments[0].as_str();
                if names.contains(name) {
                    calls.insert(name);
                }
            }
            collect_expression_calls(callee, names, calls);
            for argument in arguments {
                collect_expression_calls(&argument.value, names, calls);
            }
        }
        ExpressionKind::List { elements } => {
            for value in elements {
                collect_expression_calls(value, names, calls);
            }
        }
        ExpressionKind::Record { fields, .. } | ExpressionKind::Intent { fields, .. } => {
            for field in fields {
                collect_expression_calls(&field.value, names, calls);
            }
        }
        ExpressionKind::Field { target, .. }
        | ExpressionKind::Unary {
            operand: target, ..
        }
        | ExpressionKind::Try { value: target }
        | ExpressionKind::Validate {
            candidate: target, ..
        } => collect_expression_calls(target, names, calls),
        ExpressionKind::Binary { left, right, .. } => {
            collect_expression_calls(left, names, calls);
            collect_expression_calls(right, names, calls);
        }
        ExpressionKind::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expression_calls(condition, names, calls);
            collect_block_calls(then_block, names, calls);
            collect_block_calls(else_block, names, calls);
        }
        ExpressionKind::Match { value, arms } => {
            collect_expression_calls(value, names, calls);
            for arm in arms {
                collect_expression_calls(&arm.value, names, calls);
            }
        }
        ExpressionKind::Infer { arguments, .. } | ExpressionKind::Observe { arguments, .. } => {
            for argument in arguments {
                collect_expression_calls(&argument.value, names, calls);
            }
        }
        ExpressionKind::Propose {
            arguments, intent, ..
        } => {
            for argument in arguments {
                collect_expression_calls(&argument.value, names, calls);
            }
            collect_expression_calls(intent, names, calls);
        }
        ExpressionKind::Authorize { proposal, .. } => {
            collect_expression_calls(proposal, names, calls);
        }
        ExpressionKind::Commit { proposal, permit } => {
            collect_expression_calls(proposal, names, calls);
            collect_expression_calls(permit, names, calls);
        }
        ExpressionKind::Reconcile {
            receipt,
            observation,
            ..
        } => {
            collect_expression_calls(receipt, names, calls);
            collect_expression_calls(observation, names, calls);
        }
        ExpressionKind::Unit
        | ExpressionKind::Bool { .. }
        | ExpressionKind::Int { .. }
        | ExpressionKind::Text { .. }
        | ExpressionKind::Path { .. } => {}
    }
}
