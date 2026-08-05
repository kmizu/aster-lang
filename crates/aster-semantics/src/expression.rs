use std::collections::{BTreeMap, BTreeSet};

use aster_diagnostics::{Diagnostic, KnownDiagnosticCode, Span};
use aster_syntax::{
    Argument, BinaryOperator, Block, Expression, ExpressionKind, Parameter, Path, Pattern,
    StatementKind, ToolMode, TypeDefinition, UnaryOperator,
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

    fn join_moved(&mut self, branches: &[Self]) {
        for (name, binding) in &mut self.bindings {
            binding.moved |= branches
                .iter()
                .filter_map(|branch| branch.bindings.get(name))
                .any(|branch_binding| branch_binding.moved);
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
            self.check_statement(&statement.kind, environment, context);
        }
    }

    fn check_statement(
        &mut self,
        statement: &StatementKind,
        environment: &mut Environment,
        context: &CheckContext,
    ) {
        match statement {
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
                if self.is_affine(&actual, &mut BTreeSet::new()) {
                    mark_expression_moved(value, environment);
                }
                environment.insert(name, bound);
            }
            StatementKind::Require { condition } => {
                let actual = self.check_expression(condition, environment, context);
                self.expect_type(&Type::Bool, &actual, &condition.span);
            }
            StatementKind::UpdateState { fields } => {
                self.check_state_update(fields, environment, context);
            }
            StatementKind::Return { value } => {
                let actual = self.check_expression(value, environment, context);
                self.expect_type(&context.return_type, &actual, &value.span);
                if self.is_affine(&actual, &mut BTreeSet::new()) {
                    mark_expression_moved(value, environment);
                }
            }
            StatementKind::Expression { expression } => {
                self.check_expression(expression, environment, context);
            }
        }
    }

    fn check_state_update(
        &mut self,
        fields: &[aster_syntax::FieldInitializer],
        environment: &mut Environment,
        context: &CheckContext,
    ) {
        let Some(agent_name) = &context.agent else {
            for field in fields {
                self.check_expression(&field.value, environment, context);
            }
            if let Some(field) = fields.first() {
                self.type_mismatch(
                    "state can be updated only inside an agent handler",
                    &field.span,
                );
            }
            return;
        };
        let Some(agent) = self.model.agents.get(agent_name) else {
            return;
        };
        let mut seen = BTreeSet::new();
        for field in fields {
            let actual = self.check_expression(&field.value, environment, context);
            if !seen.insert(field.name.as_str()) {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::DuplicateDeclaration.into(),
                        format!("state field `{}` is updated more than once", field.name),
                        field.span.clone(),
                    )
                    .with_help("update each state field at most once per transaction"),
                );
            }
            let Some(declaration) = agent.state.iter().find(|value| value.name == field.name)
            else {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::UnknownName.into(),
                        format!("unknown mutable state field `{}`", field.name),
                        field.span.clone(),
                    )
                    .with_help("update a field declared in the agent state block"),
                );
                continue;
            };
            let expected = self.model.resolve_type(&declaration.ty);
            self.expect_type(&expected, &actual, &field.value.span);
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
            ExpressionKind::List { elements } => self.check_list(elements, environment, context),
            ExpressionKind::Record { path, fields } => {
                self.check_record(path, fields, expression, environment, context)
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
                self.check_intent(purpose, fields, expression, environment, context)
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

    fn check_list(
        &mut self,
        elements: &[Expression],
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let mut element_type = Type::Unknown;
        for element in elements {
            let actual = self.check_expression(element, environment, context);
            if element_type == Type::Unknown {
                element_type = actual.clone();
            } else {
                self.expect_type(&element_type, &actual, &element.span);
            }
            if self.is_affine(&actual, &mut BTreeSet::new()) {
                mark_expression_moved(element, environment);
            }
        }
        Type::List(Box::new(element_type))
    }

    fn check_record(
        &mut self,
        path: &Path,
        fields: &[aster_syntax::FieldInitializer],
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let name = path.as_string();
        let ty = Type::Named(name.clone());
        let Some(declaration) = self.model.types.get(&name) else {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::UnknownName.into(),
                    format!("unknown record type `{name}`"),
                    path.span.clone(),
                )
                .with_help("construct a declared record type"),
            );
            for field in fields {
                self.check_expression(&field.value, environment, context);
            }
            return Type::Unknown;
        };
        let TypeDefinition::Record(declared_fields) = &declaration.definition else {
            self.type_mismatch("record construction requires a record type", &path.span);
            return Type::Unknown;
        };
        let expected: BTreeMap<_, _> = declared_fields
            .iter()
            .map(|field| (field.name.as_str(), self.model.resolve_type(&field.ty)))
            .collect();
        let mut seen = BTreeSet::new();
        for field in fields {
            let actual = self.check_expression(&field.value, environment, context);
            if self.is_affine(&actual, &mut BTreeSet::new()) {
                mark_expression_moved(&field.value, environment);
            }
            if !seen.insert(field.name.as_str()) {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::DuplicateDeclaration.into(),
                        format!("duplicate record field `{}`", field.name),
                        field.span.clone(),
                    )
                    .with_help("initialize each record field exactly once"),
                );
            }
            if let Some(expected) = expected.get(field.name.as_str()) {
                self.expect_type(expected, &actual, &field.value.span);
            } else {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::UnknownName.into(),
                        format!("unknown field `{}` for record `{name}`", field.name),
                        field.span.clone(),
                    )
                    .with_help("remove the field or use a declared record field"),
                );
            }
        }
        let missing = expected
            .keys()
            .copied()
            .filter(|field| !seen.contains(field))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::TypeMismatch.into(),
                    format!("record `{name}` is missing fields: {}", missing.join(", ")),
                    expression.span.clone(),
                )
                .with_help("initialize every declared record field exactly once"),
            );
        }
        ty
    }

    fn check_try_expression(
        &mut self,
        value: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let actual = self.check_expression(value, environment, context);
        let result_context = matches!(
            self.model.normalized(&context.return_type),
            Type::Result(_, error) if self.compatible(&Type::Error, &error)
        );
        if !result_context {
            self.type_mismatch(
                "postfix `?` requires the enclosing callable to return Result<T, Error>",
                &value.span,
            );
        }
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
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let actual = self.check_expression(condition, environment, context);
        self.expect_type(&Type::Bool, &actual, &condition.span);
        let mut then_environment = environment.clone();
        let mut else_environment = environment.clone();
        let then_type = self.check_block_value(then_block, &mut then_environment, context);
        let else_type = self.check_block_value(else_block, &mut else_environment, context);
        self.expect_type(&then_type, &else_type, &else_block.span);
        environment.join_moved(&[then_environment, else_environment]);
        then_type
    }

    fn check_block_value(
        &mut self,
        block: &Block,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let Some((last, prefix)) = block.statements.split_last() else {
            return Type::Unit;
        };
        for statement in prefix {
            self.check_statement(&statement.kind, environment, context);
        }
        if let StatementKind::Expression { expression } = &last.kind {
            let ty = self.check_expression(expression, environment, context);
            if self.is_affine(&ty, &mut BTreeSet::new()) {
                mark_expression_moved(expression, environment);
            }
            ty
        } else {
            self.check_statement(&last.kind, environment, context);
            Type::Unit
        }
    }

    fn check_match_expression(
        &mut self,
        value: &Expression,
        arms: &[aster_syntax::MatchArm],
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        let matched_type = self.check_expression(value, environment, context);
        let Some((variants, enum_name)) = self.match_variants(&matched_type) else {
            self.type_mismatch(
                "match requires an enum, Option, or Result value",
                &value.span,
            );
            return Type::Unknown;
        };
        let mut result = Type::Unknown;
        let mut covered = BTreeSet::new();
        let mut wildcard = false;
        let mut invalid_pattern = false;
        let mut arm_environments = Vec::new();
        for arm in arms {
            let mut arm_environment = environment.clone();
            if wildcard {
                self.type_mismatch("match arm after wildcard is unreachable", &arm.span);
            }
            match &arm.pattern {
                Pattern::Wildcard => wildcard = true,
                Pattern::Variant { path, binding } => {
                    if !self.pattern_belongs_to(path, &enum_name) {
                        invalid_pattern = true;
                        continue;
                    }
                    let variant = path.segments.last().map(String::as_str).unwrap_or_default();
                    let Some(payload) = variants.get(variant) else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::UnknownName.into(),
                                format!("unknown variant `{}` for `{enum_name}`", path.as_string()),
                                path.span.clone(),
                            )
                            .with_help("use a variant of the matched type"),
                        );
                        invalid_pattern = true;
                        continue;
                    };
                    if !covered.insert(variant.to_owned()) {
                        self.diagnostics.push(
                            Diagnostic::error(
                                KnownDiagnosticCode::DuplicateDeclaration.into(),
                                format!("duplicate match arm for `{variant}`"),
                                arm.span.clone(),
                            )
                            .with_help("match each variant at most once"),
                        );
                    }
                    match (payload, binding) {
                        (Some(ty), Some(name)) => arm_environment.insert(name, ty.clone()),
                        (None, None) => {}
                        (Some(_), None) => self.type_mismatch(
                            format!("payload variant `{variant}` requires a binding"),
                            &arm.span,
                        ),
                        (None, Some(_)) => self.type_mismatch(
                            format!("nullary variant `{variant}` cannot bind a payload"),
                            &arm.span,
                        ),
                    }
                }
            }
            let actual = self.check_expression(&arm.value, &mut arm_environment, context);
            if self.is_affine(&actual, &mut BTreeSet::new()) {
                mark_expression_moved(&arm.value, &mut arm_environment);
            }
            if result == Type::Unknown {
                result = actual;
            } else {
                self.expect_type(&result, &actual, &arm.value.span);
            }
            arm_environments.push(arm_environment);
        }
        if !wildcard && !invalid_pattern {
            let missing = variants
                .keys()
                .filter(|variant| !covered.contains(*variant))
                .cloned()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::TypeMismatch.into(),
                        format!("non-exhaustive match; missing {}", missing.join(", ")),
                        value.span.clone(),
                    )
                    .with_help("cover every variant or add a wildcard arm"),
                );
            }
        }
        environment.join_moved(&arm_environments);
        result
    }

    fn pattern_belongs_to(&mut self, path: &Path, enum_name: &str) -> bool {
        let qualifier = path
            .segments
            .get(..path.segments.len().saturating_sub(1))
            .unwrap_or_default();
        if qualifier.is_empty() || (qualifier.len() == 1 && qualifier[0] == enum_name) {
            return true;
        }
        self.diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::UnknownName.into(),
                format!(
                    "pattern `{}` does not belong to `{enum_name}`",
                    path.as_string()
                ),
                path.span.clone(),
            )
            .with_help("use a variant of the matched type"),
        );
        false
    }

    fn match_variants(&self, ty: &Type) -> Option<(BTreeMap<String, Option<Type>>, String)> {
        match self.model.normalized(ty) {
            Type::Named(name) => self.model.enums.get(&name).map(|declaration| {
                (
                    declaration
                        .variants
                        .iter()
                        .map(|variant| {
                            (
                                variant.name.clone(),
                                variant
                                    .payload
                                    .as_ref()
                                    .map(|payload| self.model.resolve_type(payload)),
                            )
                        })
                        .collect(),
                    name,
                )
            }),
            Type::Option(inner) => Some((
                BTreeMap::from([("None".to_owned(), None), ("Some".to_owned(), Some(*inner))]),
                "Option".to_owned(),
            )),
            Type::Result(ok, error) => Some((
                BTreeMap::from([
                    ("Err".to_owned(), Some(*error)),
                    ("Ok".to_owned(), Some(*ok)),
                ]),
                "Result".to_owned(),
            )),
            _ => None,
        }
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
            self.check_arguments(
                &function.parameters,
                arguments,
                environment,
                context,
                &callee.span,
            );
            return self.model.resolve_type(&function.return_type);
        }
        if let Some(flow) = self.model.flows.get(&name) {
            self.effect(context, None, &callee.span);
            for capability in &flow.uses {
                self.effect(context, Some(&capability.path.as_string()), &callee.span);
            }
            self.check_arguments(
                &flow.parameters,
                arguments,
                environment,
                context,
                &callee.span,
            );
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
        for (argument, ty) in arguments.iter().zip(&values) {
            if self.is_affine(ty, &mut BTreeSet::new()) {
                mark_expression_moved(&argument.value, environment);
            }
        }
        match (name, values.as_slice()) {
            ("len", [Type::List(_)]) => Type::Int,
            ("first", [Type::List(inner)]) => Type::Result(inner.clone(), Box::new(Type::Error)),
            ("contains", [Type::List(inner), value]) => {
                self.expect_type(inner, value, span);
                if !self.is_equatable(inner, &mut BTreeSet::new()) {
                    self.type_mismatch("contains requires equatable list elements", span);
                }
                Type::Bool
            }
            ("subset", [Type::List(left), Type::List(right)]) => {
                self.expect_type(left, right, span);
                if !self.is_equatable(left, &mut BTreeSet::new()) {
                    self.type_mismatch("subset requires equatable list elements", span);
                }
                Type::Bool
            }
            ("provenance", [value])
                if matches!(
                    self.model.normalized(value),
                    Type::Incoming(_)
                        | Type::Untrusted(_)
                        | Type::Candidate(_)
                        | Type::Checked(_)
                        | Type::Observation(_)
                        | Type::Receipt(_)
                        | Type::Reconciled(_)
                ) =>
            {
                Type::ProvenanceRef
            }
            ("add_seconds", [instant, seconds]) => {
                self.expect_type(&Type::Instant, instant, span);
                self.expect_type(&Type::Int, seconds, span);
                Type::Instant
            }
            ("Some", [inner]) => Type::Option(Box::new(inner.clone())),
            ("Ok", [inner]) => Type::Result(Box::new(inner.clone()), Box::new(Type::Unknown)),
            ("Err", [error]) => Type::Result(Box::new(Type::Unknown), Box::new(error.clone())),
            ("Human", [_]) => Type::Text,
            _ if matches!(
                name,
                "len"
                    | "first"
                    | "contains"
                    | "subset"
                    | "provenance"
                    | "add_seconds"
                    | "Some"
                    | "Ok"
                    | "Err"
                    | "Human"
            ) =>
            {
                self.type_mismatch(format!("invalid arguments for built-in `{name}`"), span);
                Type::Unknown
            }
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
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                self.expect_type(&left_type, &right_type, &right.span);
                if !self.is_equatable(&left_type, &mut BTreeSet::new()) {
                    self.type_mismatch("type is not structurally equatable", &left.span);
                }
                Type::Bool
            }
            BinaryOperator::Less
            | BinaryOperator::LessEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterEqual => {
                self.expect_type(&Type::Int, &left_type, &left.span);
                self.expect_type(&Type::Int, &right_type, &right.span);
                Type::Bool
            }
        }
    }

    fn is_equatable(&self, ty: &Type, seen: &mut BTreeSet<String>) -> bool {
        match self.model.normalized(ty) {
            Type::Unit
            | Type::Bool
            | Type::Int
            | Type::Text
            | Type::Instant
            | Type::Duration
            | Type::ProvenanceRef
            | Type::Error => true,
            Type::Option(inner) | Type::List(inner) => self.is_equatable(&inner, seen),
            Type::Result(ok, error) => {
                self.is_equatable(&ok, seen) && self.is_equatable(&error, seen)
            }
            Type::Named(name) => {
                if !seen.insert(name.clone()) {
                    return true;
                }
                let result = if let Some(declaration) = self.model.types.get(&name) {
                    match &declaration.definition {
                        TypeDefinition::Alias(reference) => {
                            self.is_equatable(&self.model.resolve_type(reference), seen)
                        }
                        TypeDefinition::Record(fields) => fields.iter().all(|field| {
                            self.is_equatable(&self.model.resolve_type(&field.ty), seen)
                        }),
                    }
                } else if let Some(declaration) = self.model.enums.get(&name) {
                    declaration.variants.iter().all(|variant| {
                        variant.payload.as_ref().is_none_or(|payload| {
                            self.is_equatable(&self.model.resolve_type(payload), seen)
                        })
                    })
                } else {
                    false
                };
                seen.remove(&name);
                result
            }
            _ => false,
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
            self.unknown_reference("prompt", prompt);
            return Type::Unknown;
        };
        let argument_types = self.check_arguments(
            &declaration.parameters,
            arguments,
            environment,
            context,
            &expression.span,
        );
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
        let name = validator.as_string();
        let Some(declaration) = self.model.validators.get(&name) else {
            self.diagnostics.push(
                Diagnostic::error(
                    KnownDiagnosticCode::UnknownName.into(),
                    format!("unknown validator `{name}`"),
                    validator.span.clone(),
                )
                .with_help("declare a one-parameter validator"),
            );
            return Type::Result(Box::new(Type::Checked(inner)), Box::new(Type::Error));
        };
        if declaration.parameters.len() == 1 {
            let expected = self.model.resolve_type(&declaration.parameters[0].ty);
            self.expect_type(&expected, &inner, &candidate.span);
        } else {
            self.type_mismatch(
                "candidate validation requires a one-parameter validator",
                &validator.span,
            );
        }
        Type::Result(Box::new(Type::Checked(inner)), Box::new(Type::Error))
    }

    fn check_intent(
        &mut self,
        purpose: &Path,
        fields: &[aster_syntax::FieldInitializer],
        expression: &Expression,
        environment: &mut Environment,
        context: &CheckContext,
    ) -> Type {
        const REQUIRED: [&str; 5] = ["actor", "beneficiary", "basis", "expected", "expires_at"];
        let mut seen = BTreeSet::new();
        for field in fields {
            let actual = self.check_expression(&field.value, environment, context);
            if !seen.insert(field.name.as_str()) {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::DuplicateDeclaration.into(),
                        format!("duplicate intent field `{}`", field.name),
                        field.span.clone(),
                    )
                    .with_help("initialize each intent field exactly once"),
                );
            }
            match field.name.as_str() {
                "actor" | "beneficiary" | "expected" => {}
                "basis" => {
                    self.expect_type(
                        &Type::List(Box::new(Type::ProvenanceRef)),
                        &actual,
                        &field.value.span,
                    );
                    if matches!(&field.value.kind, ExpressionKind::List { elements } if elements.is_empty())
                    {
                        self.type_mismatch("intent basis must be non-empty", &field.value.span);
                    }
                }
                "expires_at" => {
                    self.expect_type(&Type::Instant, &actual, &field.value.span);
                }
                _ => self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::UnknownName.into(),
                        format!("unknown intent field `{}`", field.name),
                        field.span.clone(),
                    )
                    .with_help("use the five required intent fields"),
                ),
            }
        }
        let missing = REQUIRED
            .into_iter()
            .filter(|name| !seen.contains(name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.type_mismatch(
                format!("intent is missing fields: {}", missing.join(", ")),
                &expression.span,
            );
        }
        Type::Intent(purpose.as_string())
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
            self.unknown_reference("tool", action);
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
        self.check_arguments(
            &tool.parameters,
            arguments,
            environment,
            context,
            &expression.span,
        );
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
            self.unknown_reference("tool", action);
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
        self.require_capability(context, capability.as_deref(), &expression.span);
        self.check_arguments(
            &tool.parameters,
            arguments,
            environment,
            context,
            &expression.span,
        );
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
        self.effect(context, None, &proposal.span);
        if let Some(declaration) = self.model.policies.get(&policy.as_string()) {
            if declaration
                .rules
                .iter()
                .any(|rule| matches!(rule.decision, aster_syntax::PolicyDecision::Approve(_)))
            {
                self.require_capability(context, Some("HumanApproval"), &proposal.span);
            }
            if let Some(parameter) = declaration.parameters.first() {
                let expected = self.model.resolve_type(&parameter.ty);
                self.expect_type(&expected, &Type::Proposal(action.clone()), &proposal.span);
            }
        } else {
            self.unknown_reference("policy", policy);
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
        let capability = self
            .model
            .tools
            .get(proposal_action)
            .and_then(|tool| tool.metadata.capability.as_ref())
            .map(|capability| capability.path.as_string());
        self.effect(context, capability.as_deref(), &proposal.span);
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
        let Some(declaration) = self.model.validators.get(&validator.as_string()) else {
            self.unknown_reference("validator", validator);
            return Type::Result(Box::new(Type::Reconciled(action)), Box::new(Type::Error));
        };
        if declaration.parameters.len() != 2 {
            self.type_mismatch(
                "reconciliation requires a two-parameter validator",
                &validator.span,
            );
            return Type::Result(Box::new(Type::Reconciled(action)), Box::new(Type::Error));
        }
        let expected_result = self.model.tool_result(&action);
        let expected_parameter = self.model.resolve_type(&declaration.parameters[0].ty);
        let actual_parameter = self.model.resolve_type(&declaration.parameters[1].ty);
        self.expect_type(&expected_parameter, &expected_result, &receipt.span);
        self.expect_type(&actual_parameter, &actual, &observation.span);
        Type::Result(Box::new(Type::Reconciled(action)), Box::new(Type::Error))
    }

    pub(crate) fn check_arguments(
        &mut self,
        parameters: &[Parameter],
        arguments: &[Argument],
        environment: &mut Environment,
        context: &CheckContext,
        span: &Span,
    ) -> Vec<Type> {
        let mut types = Vec::with_capacity(arguments.len());
        let mut assigned = BTreeSet::new();
        let mut positional = 0;
        for argument in arguments {
            let actual = self.check_expression(&argument.value, environment, context);
            let parameter_index = if let Some(name) = &argument.name {
                parameters
                    .iter()
                    .position(|parameter| parameter.name == *name)
            } else {
                let value = (positional < parameters.len()).then_some(positional);
                positional += 1;
                value
            };
            if let Some(parameter_index) = parameter_index {
                let parameter = &parameters[parameter_index];
                if !assigned.insert(parameter_index) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            KnownDiagnosticCode::DuplicateDeclaration.into(),
                            format!("argument `{}` is supplied more than once", parameter.name),
                            argument.span.clone(),
                        )
                        .with_help("supply each parameter exactly once"),
                    );
                }
                let expected = self.model.resolve_type(&parameter.ty);
                self.expect_type(&expected, &actual, &argument.value.span);
                if self.is_affine(&actual, &mut BTreeSet::new()) {
                    mark_expression_moved(&argument.value, environment);
                }
            } else if let Some(name) = &argument.name {
                self.diagnostics.push(
                    Diagnostic::error(
                        KnownDiagnosticCode::UnknownName.into(),
                        format!("unknown argument name `{name}`"),
                        argument.span.clone(),
                    )
                    .with_help("use a declared parameter name"),
                );
            } else {
                self.type_mismatch("too many positional arguments", &argument.span);
            }
            types.push(actual);
        }
        let missing = parameters
            .iter()
            .enumerate()
            .filter(|(index, _)| !assigned.contains(index))
            .map(|(_, parameter)| parameter.name.as_str())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            self.type_mismatch(format!("missing arguments: {}", missing.join(", ")), span);
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
                let code = self
                    .affine_diagnostic_code(&binding.ty, &mut BTreeSet::new())
                    .unwrap_or(KnownDiagnosticCode::TypeMismatch);
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
        self.require_capability(context, capability, span);
    }

    fn require_capability(
        &mut self,
        context: &CheckContext,
        capability: Option<&str>,
        span: &Span,
    ) {
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

    fn is_affine(&self, ty: &Type, seen: &mut BTreeSet<String>) -> bool {
        self.affine_diagnostic_code(ty, seen).is_some()
    }

    fn affine_diagnostic_code(
        &self,
        ty: &Type,
        seen: &mut BTreeSet<String>,
    ) -> Option<KnownDiagnosticCode> {
        match self.model.normalized(ty) {
            Type::Proposal(_) => Some(KnownDiagnosticCode::ProposalUseAfterMove),
            Type::Permit(_) => Some(KnownDiagnosticCode::PermitUseAfterMove),
            Type::Option(inner)
            | Type::List(inner)
            | Type::Incoming(inner)
            | Type::Untrusted(inner)
            | Type::Candidate(inner)
            | Type::Checked(inner)
            | Type::Observation(inner)
            | Type::Secret(inner) => self.affine_diagnostic_code(&inner, seen),
            Type::Result(ok, error) => self
                .affine_diagnostic_code(&ok, seen)
                .or_else(|| self.affine_diagnostic_code(&error, seen)),
            Type::Named(name) => {
                if !seen.insert(name.clone()) {
                    return None;
                }
                let result = self
                    .model
                    .types
                    .get(&name)
                    .and_then(|declaration| match &declaration.definition {
                        TypeDefinition::Alias(reference) => {
                            self.affine_diagnostic_code(&self.model.resolve_type(reference), seen)
                        }
                        TypeDefinition::Record(fields) => fields.iter().find_map(|field| {
                            self.affine_diagnostic_code(&self.model.resolve_type(&field.ty), seen)
                        }),
                    })
                    .or_else(|| {
                        self.model.enums.get(&name).and_then(|declaration| {
                            declaration.variants.iter().find_map(|variant| {
                                variant.payload.as_ref().and_then(|payload| {
                                    self.affine_diagnostic_code(
                                        &self.model.resolve_type(payload),
                                        seen,
                                    )
                                })
                            })
                        })
                    });
                seen.remove(&name);
                result
            }
            _ => None,
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

    fn unknown_reference(&mut self, kind: &str, path: &Path) {
        self.diagnostics.push(
            Diagnostic::error(
                KnownDiagnosticCode::UnknownName.into(),
                format!("unknown {kind} `{}`", path.as_string()),
                path.span.clone(),
            )
            .with_help(format!("declare the {kind} before using it")),
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
