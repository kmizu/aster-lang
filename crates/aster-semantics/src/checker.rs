use std::collections::{BTreeMap, BTreeSet};

use aster_diagnostics::{Diagnostic, KnownDiagnosticCode};
use aster_syntax::{
    AgentDeclaration, Block, DeclarationKind, Expression, ExpressionKind, FunctionDeclaration,
    Module, PolicyDecision, SourceFile, StatementKind, ToolMode, parse,
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

fn check_duplicate_declarations(module: &Module, diagnostics: &mut Vec<Diagnostic>) {
    let mut seen = BTreeSet::new();
    for declaration in &module.declarations {
        let identity = declaration_identity(&declaration.kind);
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

fn check_declaration_metadata(
    module: &Module,
    model: &Model<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Prompt(prompt) => {
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
            DeclarationKind::Tool(tool)
                if tool.metadata.mode == Some(ToolMode::Write)
                    && tool.metadata.idempotency.is_none() =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::MissingIdempotency.into(),
                        "write tool is missing idempotency metadata",
                        declaration.span.clone(),
                    )
                    .with_help("name a deterministic request parameter with `idempotency`"),
                );
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
            }
            DeclarationKind::Agent(agent) => check_agent_metadata(agent, model, diagnostics),
            _ => {}
        }
    }
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
                            checker.check_expression(value, &mut environment, &context);
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
    defaults.insert("self", Type::AgentState(agent.name.clone()));
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
