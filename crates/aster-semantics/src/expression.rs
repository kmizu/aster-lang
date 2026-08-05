use std::collections::{BTreeMap, BTreeSet};

use aster_diagnostics::{Diagnostic, KnownDiagnosticCode, Span};
use aster_syntax::{
    Argument, BinaryOperator, Block, Expression, ExpressionKind, Parameter, Path, StatementKind,
    ToolMode, UnaryOperator,
};

use crate::{Type, model::Model};

#[derive(Clone)]
struct Binding {
    ty: Type,
    moved: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Environment {
    bindings: BTreeMap<String, Binding>,
}

impl Environment {
    pub(crate) fn insert(&mut self, name: impl Into<String>, ty: Type) {
        self.bindings
            .insert(name.into(), Binding { ty, moved: false });
    }

    fn get(&self, name: &str) -> Option<&Binding> {
        self.bindings.get(name)
    }

    fn mark_moved(&mut self, name: &str) {
        if let Some(binding) = self.bindings.get_mut(name) {
            binding.moved = true;
        }
    }
}

pub(crate) struct CheckContext {
    pub(crate) pure: bool,
    pub(crate) allowed_capabilities: BTreeSet<String>,
    pub(crate) return_type: Type,
    pub(crate) agent: Option<String>,
}

pub(crate) struct ExpressionChecker<'a, 'm> {
    model: &'m Model<'a>,
    diagnostics: &'m mut Vec<Diagnostic>,
}

impl<'a, 'm> ExpressionChecker<'a, 'm> {
    pub(crate) fn new(model: &'m Model<'a>, diagnostics: &'m mut Vec<Diagnostic>) -> Self {
        Self { model, diagnostics }
    }

    pub(crate) fn check_block(
        &mut self,
        block: &Block,
        environment: &mut Environment,
        context: &CheckContext,
    ) {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let { name, ty, value } => {
                    let actual = self.check_expression(value, environment, context);
                    let bound = ty.as_ref().map_or_else(
                        || actual.clone(),
                        |annotation| {
                            let expected = self.model.resolve_type(annotation);
                            self.expect_type(&expected, &actual, &value.span);
                            expected
                        },
                    );
                    environment.insert(name, bound);
                }
                StatementKind::Require { condition } => {
                    let actual = self.check_expression(condition, environment, context);
                    self.expect_type(&Type::Bool, &actual, &condition.span);
                }
                StatementKind::UpdateState { fields } => {
                    if let Some(agent) = &context.agent {
                        for field in fields {
                            let actual = self.check_expression(&field.value, environment, context);
                            let expected = self
                                .model
                                .field_type(&Type::AgentState(agent.clone()), &field.name)
                                .unwrap_or(Type::Unknown);
                            self.expect_type(&expected, &actual, &field.value.span);
                        }
                    }
                }
                StatementKind::Return { value } => {
                    let actual = self.check_expression(value, environment, context);
                    self.expect_type(&context.return_type, &actual, &value.span);
                }
                StatementKind::Expression { expression } => {
                    self.check_expression(expression, environment, context);
                }
            }
        }
    }

    pub(crate) fn check_expression(
        &mut self,
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        match &expression.kind {
            ExpressionKind::Unit => Type::Unit,
            ExpressionKind::Bool { .. } => Type::Bool,
            ExpressionKind::Int { .. } => Type::Int,
            ExpressionKind::Text { .. } => Type::Text,
            ExpressionKind::Path { path } => self.resolve_path(path, environment),
            ExpressionKind::List { elements } => {
                let mut element_type = Type::Unknown;
                for element in elements {
                    let actual = self.check_expression(element, environment, context);
                    if element_type == Type::Unknown {
                        element_type = actual;
                    } else {
                        self.expect_type(&element_type, &actual, &element.span);
                    }
                }
                Type::List(Box::new(element_type))
            }
            ExpressionKind::Record { path, fields } => {
                let ty = Type::Named(path.as_string());
                for field in fields {
                    let actual = self.check_expression(&field.value, environment, context);
                    let expected = self
                        .model
                        .field_type(&ty, &field.name)
                        .unwrap_or(Type::Unknown);
                    self.expect_type(&expected, &actual, &field.value.span);
                }
                ty
            }
            ExpressionKind::Call { callee, arguments } => {
                self.check_call(callee, arguments, environment, context)
            }
            ExpressionKind::Field { target, field } => {
                let target_type = self.check_expression(target, environment, context);
                self.project_type(&target_type, field, &expression.span)
            }
            ExpressionKind::Unary { operator, operand } => {
                let actual = self.check_expression(operand, environment, context);
                let expected = match operator {
                    UnaryOperator::Not => Type::Bool,
                    UnaryOperator::Negate => Type::Int,
                };
                self.expect_type(&expected, &actual, &operand.span);
                expected
            }
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => self.check_binary(left, *operator, right, environment, context),
            ExpressionKind::Try { value } => self.check_try_expression(value, environment, context),
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            } => self.check_if_expression(condition, then_block, else_block, environment, context),
            ExpressionKind::Match { value, arms } => {
                self.check_match_expression(value, arms, environment, context)
            }
            ExpressionKind::Infer {
                prompt, arguments, ..
            } => self.check_infer(prompt, arguments, expression, environment, context),
            ExpressionKind::Validate {
                candidate,
                validator,
            } => self.check_validate(candidate, validator, environment, context),
            ExpressionKind::Observe { action, arguments } => {
                self.check_observe(action, arguments, expression, environment, context)
            }
            ExpressionKind::Intent { purpose, fields } => {
                for field in fields {
                    self.check_expression(&field.value, environment, context);
                }
                Type::Intent(purpose.as_string())
            }
            ExpressionKind::Propose {
                action,
                arguments,
                intent,
            } => self.check_propose(action, arguments, intent, expression, environment, context),
            ExpressionKind::Authorize { proposal, policy } => {
                self.check_authorize(proposal, policy, environment, context)
            }
            ExpressionKind::Commit { proposal, permit } => {
                self.check_commit(proposal, permit, environment, context)
            }
            ExpressionKind::Reconcile {
                receipt,
                observation,
                validator,
            } => self.check_reconcile(receipt, observation, validator, environment, context),
        }
    }

    fn check_try_expression(
        &mut self,
        value: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let actual = self.check_expression(value, environment, context);
        if let Type::Result(ok, error) = self.model.normalized(&actual) {
            self.expect_type(&Type::Error, &error, &value.span);
            *ok
        } else {
            self.type_mismatch("postfix `?` requires Result<T, Error>", &value.span);
            Type::Unknown
        }
    }

    fn check_if_expression(
        &mut self,
        condition: &Expression,
        then_block: &Block,
        else_block: &Block,
        environment: &Environment,
        context: &CheckContext,
    ) -> Type {
        let mut condition_environment = environment.clone();
        let actual = self.check_expression(condition, &mut condition_environment, context);
        self.expect_type(&Type::Bool, &actual, &condition.span);
        self.check_block(then_block, &mut environment.clone(), context);
        self.check_block(else_block, &mut environment.clone(), context);
        Type::Unknown
    }

    fn check_match_expression(
        &mut self,
        value: &Expression,
        arms: &[aster_syntax::MatchArm],
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        self.check_expression(value, environment, context);
        let mut result = Type::Unknown;
        for arm in arms {
            let actual = self.check_expression(&arm.value, environment, context);
            if result == Type::Unknown {
                result = actual;
            } else {
                self.expect_type(&result, &actual, &arm.value.span);
            }
        }
        result
    }

    fn check_call(
        &mut self,
        callee: &Expression,
        arguments: &[Argument],
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let Some(path) = expression_path(callee) else {
            self.type_mismatch("callee is not a statically resolved function", &callee.span);
            return Type::Unknown;
        };
        let name = path.as_string();
        if self.model.tools.contains_key(&name) {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::DirectToolCall.into(),
                    "tools cannot be called as ordinary functions",
                    path.span.clone(),
                )
                .with_help("use `observe` for reads or the governed write pipeline"),
            );
            return Type::Unknown;
        }
        if let Some(function) = self.model.functions.get(&name) {
            self.check_arguments(&function.parameters, arguments, environment, context);
            return self.model.resolve_type(&function.return_type);
        }
        if let Some(flow) = self.model.flows.get(&name) {
            self.effect(context, None, &callee.span);
            for capability in &flow.uses {
                self.effect(context, Some(&capability.path.as_string()), &callee.span);
            }
            self.check_arguments(&flow.parameters, arguments, environment, context);
            return self.model.resolve_type(&flow.return_type);
        }
        if let Some((enum_name, payload)) = self.model.enum_variant(&name) {
            let values = arguments
                .iter()
                .map(|argument| self.check_expression(&argument.value, environment, context))
                .collect::<Vec<_>>();
            match (payload, values.as_slice()) {
                (None, []) => {}
                (Some(expected), [actual]) => {
                    self.expect_type(&expected, actual, &callee.span);
                }
                _ => self.type_mismatch("enum constructor argument mismatch", &callee.span),
            }
            return Type::Named(enum_name);
        }
        self.check_builtin(&name, arguments, environment, context, &callee.span)
    }

    fn check_builtin(
        &mut self,
        name: &str,
        arguments: &[Argument],
        environment: &mut Environment,
        context: &CheckContext,
        span: &Span,
    ) -> Type {
        let values: Vec<_> = arguments
            .iter()
            .map(|argument| self.check_expression(&argument.value, environment, context))
            .collect();
        match (name, values.as_slice()) {
            ("len", [Type::List(_)]) => Type::Int,
            ("first", [Type::List(inner)]) => Type::Result(inner.clone(), Box::new(Type::Error)),
            ("contains" | "subset", [_, _]) => Type::Bool,
            ("provenance", [_]) => Type::ProvenanceRef,
            ("add_seconds", [instant, seconds]) => {
                self.expect_type(&Type::Instant, instant, span);
                self.expect_type(&Type::Int, seconds, span);
                Type::Instant
            }
            ("Some", [inner]) => Type::Option(Box::new(inner.clone())),
            ("Ok", [inner]) => Type::Result(Box::new(inner.clone()), Box::new(Type::Unknown)),
            ("Err", [error]) => Type::Result(Box::new(Type::Unknown), Box::new(error.clone())),
            ("Human", [_]) => Type::Text,
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::UnknownName.into(),
                        format!("unknown function or constructor `{name}`"),
                        span.clone(),
                    )
                    .with_help("declare the function or use a documented built-in"),
                );
                Type::Unknown
            }
        }
    }

    fn check_binary(
        &mut self,
        left: &Expression,
        operator: BinaryOperator,
        right: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let left_type = self.check_expression(left, environment, context);
        let right_type = self.check_expression(right, environment, context);
        match operator {
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide => {
                self.expect_type(&Type::Int, &left_type, &left.span);
                self.expect_type(&Type::Int, &right_type, &right.span);
                Type::Int
            }
            BinaryOperator::And | BinaryOperator::Or => {
                self.expect_type(&Type::Bool, &left_type, &left.span);
                self.expect_type(&Type::Bool, &right_type, &right.span);
                Type::Bool
            }
            _ => {
                self.expect_type(&left_type, &right_type, &right.span);
                Type::Bool
            }
        }
    }

    fn check_infer(
        &mut self,
        prompt: &Path,
        arguments: &[Argument],
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        self.effect(context, Some("ModelUse"), &expression.span);
        let Some(declaration) = self.model.prompts.get(&prompt.as_string()) else {
            return Type::Unknown;
        };
        let argument_types =
            self.check_arguments(&declaration.parameters, arguments, environment, context);
        for (argument, ty) in arguments.iter().zip(argument_types) {
            if self.model.contains_secret(&ty) {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::SecretToModel.into(),
                        "secret data cannot enter prompt data",
                        argument.value.span.clone(),
                    )
                    .with_help("pass a non-secret validated summary"),
                );
            }
        }
        Type::Result(
            Box::new(Type::Candidate(Box::new(
                self.model.resolve_type(&declaration.result_type),
            ))),
            Box::new(Type::Error),
        )
    }

    fn check_validate(
        &mut self,
        candidate: &Expression,
        validator: &Path,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let candidate_type = self.check_expression(candidate, environment, context);
        let Type::Candidate(inner) = self.model.normalized(&candidate_type) else {
            self.type_mismatch("validate requires Candidate<T>", &candidate.span);
            return Type::Unknown;
        };
        if let Some(declaration) = self.model.validators.get(&validator.as_string())
            && let Some(parameter) = declaration.parameters.first()
        {
            let expected = self.model.resolve_type(&parameter.ty);
            self.expect_type(&expected, &inner, &candidate.span);
        }
        Type::Result(Box::new(Type::Checked(inner)), Box::new(Type::Error))
    }

    fn check_observe(
        &mut self,
        action: &Path,
        arguments: &[Argument],
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let name = action.as_string();
        let Some(tool) = self.model.tools.get(&name) else {
            return Type::Unknown;
        };
        if tool.metadata.mode == Some(ToolMode::Write) {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::WriteToolObserved.into(),
                    "write tools cannot be observed",
                    action.span.clone(),
                )
                .with_help("use intent, propose, authorize, and commit"),
            );
            return Type::Unknown;
        }
        let capability = tool
            .metadata
            .capability
            .as_ref()
            .map(|value| value.path.as_string());
        self.effect(context, capability.as_deref(), &expression.span);
        self.check_arguments(&tool.parameters, arguments, environment, context);
        Type::Result(
            Box::new(Type::Observation(Box::new(
                self.model.resolve_type(&tool.return_type),
            ))),
            Box::new(Type::Error),
        )
    }

    fn check_propose(
        &mut self,
        action: &Path,
        arguments: &[Argument],
        intent: &Expression,
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let name = action.as_string();
        let Some(tool) = self.model.tools.get(&name) else {
            return Type::Unknown;
        };
        if tool.metadata.mode == Some(ToolMode::Read) {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::ReadToolProposed.into(),
                    "read tools cannot be proposed",
                    action.span.clone(),
                )
                .with_help("use `observe` for read tools"),
            );
            return Type::Unknown;
        }
        let capability = tool
            .metadata
            .capability
            .as_ref()
            .map(|value| value.path.as_string());
        self.effect(context, capability.as_deref(), &expression.span);
        self.check_arguments(&tool.parameters, arguments, environment, context);
        let intent_type = self.check_expression(intent, environment, context);
        if !matches!(intent_type, Type::Intent(_)) && intent_type != Type::Unknown {
            self.type_mismatch("propose requires Intent<P>", &intent.span);
        }
        Type::Proposal(name)
    }

    fn check_authorize(
        &mut self,
        proposal: &Expression,
        policy: &Path,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let proposal_type = self.check_expression(proposal, environment, context);
        let Type::Proposal(action) = proposal_type else {
            return Type::Unknown;
        };
        if let Some(declaration) = self.model.policies.get(&policy.as_string()) {
            if declaration
                .rules
                .iter()
                .any(|rule| matches!(rule.decision, aster_syntax::PolicyDecision::Approve(_)))
            {
                self.effect(context, Some("HumanApproval"), &proposal.span);
            }
            if let Some(parameter) = declaration.parameters.first() {
                let expected = self.model.resolve_type(&parameter.ty);
                self.expect_type(&expected, &Type::Proposal(action.clone()), &proposal.span);
            }
        }
        Type::Result(Box::new(Type::Permit(action)), Box::new(Type::Error))
    }

    fn check_commit(
        &mut self,
        proposal: &Expression,
        permit: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let proposal_type = self.check_expression(proposal, environment, context);
        let permit_type = self.check_expression(permit, environment, context);
        let (Type::Proposal(proposal_action), Type::Permit(permit_action)) =
            (&proposal_type, &permit_type)
        else {
            return Type::Unknown;
        };
        if proposal_action != permit_action {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::PermitActionMismatch.into(),
                    "permit action does not match proposal action",
                    permit.span.clone(),
                )
                .with_help("authorize and commit the same proposal"),
            );
        }
        mark_expression_moved(proposal, environment);
        mark_expression_moved(permit, environment);
        Type::Result(
            Box::new(Type::Receipt(proposal_action.clone())),
            Box::new(Type::Error),
        )
    }

    fn check_reconcile(
        &mut self,
        receipt: &Expression,
        observation: &Expression,
        validator: &Path,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let receipt_type = self.check_expression(receipt, environment, context);
        let observation_type = self.check_expression(observation, environment, context);
        let Type::Receipt(action) = receipt_type else {
            return Type::Unknown;
        };
        let Type::Observation(actual) = self.model.normalized(&observation_type) else {
            return Type::Unknown;
        };
        if let Some(declaration) = self.model.validators.get(&validator.as_string())
            && declaration.parameters.len() == 2
        {
            let expected = self.model.tool_result(&action);
            self.expect_type(&expected, &actual, &observation.span);
        }
        Type::Result(Box::new(Type::Reconciled(action)), Box::new(Type::Error))
    }

    fn check_arguments(
        &mut self,
        parameters: &[Parameter],
        arguments: &[Argument],
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Vec<Type> {
        let mut types = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            let actual = self.check_expression(&argument.value, environment, context);
            let parameter = argument.name.as_ref().map_or_else(
                || parameters.get(index),
                |name| parameters.iter().find(|parameter| parameter.name == *name),
            );
            if let Some(parameter) = parameter {
                let expected = self.model.resolve_type(&parameter.ty);
                self.expect_type(&expected, &actual, &argument.value.span);
            }
            types.push(actual);
        }
        types
    }

    fn resolve_path(&mut self, path: &Path, environment: &Environment) -> Type {
        if let Some((enum_name, None)) = self.model.enum_variant(&path.as_string()) {
            return Type::Named(enum_name);
        }
        let Some(first) = path.segments.first() else {
            return Type::Unknown;
        };
        let mut ty = if let Some(binding) = environment.get(first) {
            if binding.moved {
                let code = match &binding.ty {
                    Type::Permit(_) => KnownDiagnosticCode::PermitUseAfterMove,
                    Type::Proposal(_) => KnownDiagnosticCode::ProposalUseAfterMove,
                    _ => KnownDiagnosticCode::TypeMismatch,
                };
                self.diagnostics.push(
                    Diagnostic::error(
                        code.into(),
                        "affine value used after commit",
                        path.span.clone(),
                    )
                    .with_help("construct and authorize a fresh proposal"),
                );
            }
            binding.ty.clone()
        } else {
            match first.as_str() {
                "Unit" => Type::Unit,
                "None" => Type::Option(Box::new(Type::Unknown)),
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            KnownDiagnosticCode::UnknownName.into(),
                            format!("unknown name `{first}`"),
                            path.span.clone(),
                        )
                        .with_help("declare this name before using it"),
                    );
                    return Type::Unknown;
                }
            }
        };
        for field in path.segments.iter().skip(1) {
            ty = self.project_type(&ty, field, &path.span);
        }
        ty
    }

    fn project_type(&mut self, ty: &Type, field: &str, span: &Span) -> Type {
        match self.model.normalized(ty) {
            Type::Candidate(_) if field == "value" => {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::CandidateBeforeValidation.into(),
                        "candidate data must be validated before use",
                        span.clone(),
                    )
                    .with_help("use `validate candidate with <Validator>`"),
                );
                Type::Unknown
            }
            Type::Incoming(inner)
            | Type::Untrusted(inner)
            | Type::Checked(inner)
            | Type::Observation(inner)
                if field == "value" =>
            {
                *inner
            }
            Type::Receipt(action) | Type::Reconciled(action) if field == "value" => {
                self.model.tool_result(&action)
            }
            Type::Proposal(action) if field == "args" => Type::ToolArguments(action),
            other => self.model.field_type(&other, field).unwrap_or_else(|| {
                self.type_mismatch(format!("type has no field `{field}`"), span);
                Type::Unknown
            }),
        }
    }

    fn effect(&mut self, context: &CheckContext, capability: Option<&str>, span: &Span) {
        if context.pure {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::EffectInPureContext.into(),
                    "external effect is not allowed in a pure context",
                    span.clone(),
                )
                .with_help("move this operation to a flow or event handler"),
            );
            return;
        }
        if let Some(capability) = capability
            && !context.allowed_capabilities.contains(capability)
        {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::MissingCapability.into(),
                    format!("missing declared capability `{capability}`"),
                    span.clone(),
                )
                .with_help("add the capability kind to `uses` or `requires`"),
            );
        }
    }

    pub(crate) fn expect_type(&mut self, expected: &Type, actual: &Type, span: &Span) {
        if actual.contains_candidate() && !matches!(expected, Type::Candidate(_)) {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::CandidateBeforeValidation.into(),
                    "candidate data must be validated before ordinary use",
                    span.clone(),
                )
                .with_help("validate the candidate before passing its data"),
            );
            return;
        }
        if !self.compatible(expected, actual) {
            self.type_mismatch(format!("expected {expected:?}, found {actual:?}"), span);
        }
    }

    fn compatible(&self, expected: &Type, actual: &Type) -> bool {
        let expected = self.model.normalized(expected);
        let actual = self.model.normalized(actual);
        match (&expected, &actual) {
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            (Type::Option(left), Type::Option(right))
            | (Type::List(left), Type::List(right))
            | (Type::Incoming(left), Type::Incoming(right))
            | (Type::Untrusted(left), Type::Untrusted(right))
            | (Type::Candidate(left), Type::Candidate(right))
            | (Type::Checked(left), Type::Checked(right))
            | (Type::Observation(left), Type::Observation(right))
            | (Type::Secret(left), Type::Secret(right)) => self.compatible(left, right),
            (Type::Result(left_ok, left_error), Type::Result(right_ok, right_error)) => {
                self.compatible(left_ok, right_ok) && self.compatible(left_error, right_error)
            }
            _ => expected == actual,
        }
    }

    fn type_mismatch(&mut self, message: impl Into<String>, span: &Span) {
        self.diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::TypeMismatch.into(),
                message,
                span.clone(),
            )
            .with_help("make the expression and required type match exactly"),
        );
    }
}

fn expression_path(expression: &Expression) -> Option<&Path> {
    let ExpressionKind::Path { path } = &expression.kind else {
        return None;
    };
    Some(path)
}

fn mark_expression_moved(expression: &Expression, environment: &mut Environment) {
    if let Some(path) = expression_path(expression)
        && path.segments.len() == 1
        && let Some(name) = path.segments.first()
    {
        environment.mark_moved(name);
    }
}

pub(crate) fn environment_from_parameters(
    model: &Model<'_>,
    parameters: &[Parameter],
) -> Environment {
    let mut environment = Environment::default();
    for parameter in parameters {
        environment.insert(&parameter.name, model.resolve_type(&parameter.ty));
    }
    environment
}
