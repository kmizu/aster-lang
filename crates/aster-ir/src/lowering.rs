use std::collections::{BTreeMap, BTreeSet};

use aster_semantics::CheckedProgram;
use aster_syntax::{
    Argument, BinaryOperator, Block, CapabilityExpression, DeclarationKind, Expression,
    ExpressionKind, FieldInitializer, FunctionDeclaration, Module, Parameter, Pattern,
    PolicyDecision, StatementKind, TypeDefinition, TypeReference, UnaryOperator,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Agent, CapabilitySpec, Catalog, FieldSpec, IR_SCHEMA_VERSION, Instruction, InstructionKind,
    MatchTarget, NamedExpression, NamedValue, PatternSpec, PolicyDecisionSpec, PolicyRuleSpec,
    PolicySpec, Program, ProgramError, PromptSpec, PureBlockSpec, PureExpression, PureMatchArmSpec,
    PureStatementSpec, Routine, StateFieldSpec, ToolMode, ToolSpec, TypeSpec,
    ValidatorRequirementSpec, ValidatorSpec, ValueId,
};

/// Controlled failure to convert checked source into explicit IR.
#[derive(Debug, Error)]
pub enum LoweringError {
    /// Normalized AST serialization unexpectedly failed.
    #[error("normalized AST serialization failed: {0}")]
    SyntaxSerialization(serde_json::Error),
    /// Persisted IR sealing failed.
    #[error(transparent)]
    Program(#[from] ProgramError),
    /// A routine exceeded representable instruction or value identities.
    #[error("routine `{0}` exceeds the ASTER 0.1 IR identity range")]
    RoutineTooLarge(String),
    /// A checked pure metadata location contained an effect expression.
    #[error("checked pure metadata unexpectedly contains an effect")]
    EffectInPureMetadata,
    /// Checked input contained a negative budget.
    #[error("checked input contains a negative budget limit")]
    NegativeBudget,
}

/// Lowers a checked module into versioned, deterministic, explicit IR.
///
/// # Errors
///
/// Returns a controlled error on serialization, identity overflow, or an
/// internal checked-program invariant violation.
pub fn lower(checked: &CheckedProgram) -> Result<Program, LoweringError> {
    let module = checked.module();
    let callables = callable_names(module);
    let mut routines = BTreeMap::new();
    let mut agents = BTreeMap::new();

    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Function(function) => {
                let key = format!("fn:{}", function.name);
                routines.insert(key.clone(), lower_routine(key, function, &callables)?);
            }
            DeclarationKind::Flow(flow) => {
                let key = format!("flow:{}", flow.name);
                routines.insert(key.clone(), lower_routine(key, flow, &callables)?);
            }
            DeclarationKind::Agent(agent) => {
                let mut handlers = BTreeMap::new();
                for handler in &agent.handlers {
                    let key = format!("agent:{}:{}", agent.name, handler.event);
                    let function = FunctionDeclaration {
                        name: key.clone(),
                        parameters: handler.parameters.clone(),
                        return_type: handler.return_type.clone(),
                        uses: Vec::new(),
                        body: handler.body.clone(),
                    };
                    routines.insert(
                        key.clone(),
                        lower_routine(key.clone(), &function, &callables)?,
                    );
                    handlers.insert(handler.event.clone(), key);
                }
                agents.insert(
                    agent.name.clone(),
                    Agent {
                        parameters: fields_from_parameters(&agent.parameters),
                        state: agent
                            .state
                            .iter()
                            .map(|field| {
                                Ok(StateFieldSpec {
                                    name: field.name.clone(),
                                    ty: type_spec(&field.ty),
                                    default: pure_expression(&field.default)?,
                                })
                            })
                            .collect::<Result<_, LoweringError>>()?,
                        budget: agent
                            .budget
                            .iter()
                            .map(|entry| {
                                Ok((
                                    entry.dimension.clone(),
                                    u64::try_from(entry.limit)
                                        .map_err(|_| LoweringError::NegativeBudget)?,
                                ))
                            })
                            .collect::<Result<_, LoweringError>>()?,
                        handlers,
                        capabilities: agent
                            .requires
                            .iter()
                            .map(capability_spec)
                            .collect::<Result<_, _>>()?,
                    },
                );
            }
            _ => {}
        }
    }

    let normalized = module
        .normalized_json()
        .map_err(LoweringError::SyntaxSerialization)?;
    let mut program = Program {
        schema_version: IR_SCHEMA_VERSION,
        compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
        module_name: module.name.as_string(),
        source_hash: hex::encode(Sha256::digest(normalized.as_bytes())),
        program_hash: String::new(),
        routines,
        agents,
        catalog: catalog(module)?,
    };
    program.seal()?;
    Ok(program)
}

fn catalog(module: &Module) -> Result<Catalog, LoweringError> {
    let mut catalog = Catalog::default();
    for declaration in &module.declarations {
        match &declaration.kind {
            DeclarationKind::Type(value) => match &value.definition {
                TypeDefinition::Alias(target) => {
                    catalog
                        .aliases
                        .insert(value.name.clone(), type_spec(target));
                }
                TypeDefinition::Record(fields) => {
                    catalog.records.insert(
                        value.name.clone(),
                        fields
                            .iter()
                            .map(|field| FieldSpec {
                                name: field.name.clone(),
                                ty: type_spec(&field.ty),
                            })
                            .collect(),
                    );
                }
            },
            DeclarationKind::Prompt(value) => {
                catalog.prompts.insert(
                    value.name.clone(),
                    PromptSpec {
                        parameters: fields_from_parameters(&value.parameters),
                        result_type: type_spec(&value.result_type),
                        instruction: value.instruction.clone(),
                        data: value.data.clone(),
                    },
                );
            }
            DeclarationKind::Capability(value) => {
                catalog.capabilities.insert(
                    value.name.clone(),
                    fields_from_parameters(&value.parameters),
                );
            }
            DeclarationKind::Enum(value) => {
                catalog.enums.insert(
                    value.name.clone(),
                    value
                        .variants
                        .iter()
                        .map(|variant| {
                            (
                                variant.name.clone(),
                                variant.payload.as_ref().map(type_spec),
                            )
                        })
                        .collect(),
                );
            }
            DeclarationKind::Tool(value) => {
                if let Some(spec) = tool_spec(value)? {
                    catalog.tools.insert(value.path.as_string(), spec);
                }
            }
            DeclarationKind::Validator(value) => {
                catalog.validators.insert(
                    value.name.clone(),
                    ValidatorSpec {
                        parameters: fields_from_parameters(&value.parameters),
                        requirements: value
                            .requirements
                            .iter()
                            .map(|requirement| -> Result<_, LoweringError> {
                                Ok(ValidatorRequirementSpec {
                                    expression: pure_expression(requirement)?,
                                    span: requirement.span.clone(),
                                })
                            })
                            .collect::<Result<_, _>>()?,
                    },
                );
            }
            DeclarationKind::Policy(value) => {
                catalog
                    .policies
                    .insert(value.name.clone(), policy_spec(value)?);
            }
            _ => {}
        }
    }
    Ok(catalog)
}

fn tool_spec(value: &aster_syntax::ToolDeclaration) -> Result<Option<ToolSpec>, LoweringError> {
    let mode = match value.metadata.mode {
        Some(aster_syntax::ToolMode::Read) => ToolMode::Read,
        Some(aster_syntax::ToolMode::Write) => ToolMode::Write,
        None => return Ok(None),
    };
    Ok(Some(ToolSpec {
        parameters: fields_from_parameters(&value.parameters),
        result_type: type_spec(&value.return_type),
        mode,
        capability: value
            .metadata
            .capability
            .as_ref()
            .map(capability_spec)
            .transpose()?,
        idempotency: value.metadata.idempotency.clone(),
        risk: value
            .metadata
            .risk
            .map(|risk| format!("{risk:?}").to_lowercase()),
        sensitivity: value
            .metadata
            .sensitivity
            .map(|sensitivity| format!("{sensitivity:?}").to_lowercase()),
    }))
}

fn policy_spec(value: &aster_syntax::PolicyDeclaration) -> Result<PolicySpec, LoweringError> {
    Ok(PolicySpec {
        parameters: fields_from_parameters(&value.parameters),
        rules: value
            .rules
            .iter()
            .map(|rule| {
                let decision = match &rule.decision {
                    PolicyDecision::Allow => PolicyDecisionSpec::Allow,
                    PolicyDecision::Approve(value) => {
                        PolicyDecisionSpec::Approve(pure_expression(value)?)
                    }
                    PolicyDecision::Deny(value) => {
                        PolicyDecisionSpec::Deny(pure_expression(value)?)
                    }
                };
                Ok(PolicyRuleSpec {
                    decision,
                    condition: rule.condition.as_ref().map(pure_expression).transpose()?,
                })
            })
            .collect::<Result<_, LoweringError>>()?,
    })
}

fn callable_names(module: &Module) -> BTreeSet<String> {
    module
        .declarations
        .iter()
        .filter_map(|declaration| match &declaration.kind {
            DeclarationKind::Function(value) | DeclarationKind::Flow(value) => {
                Some(value.name.clone())
            }
            _ => None,
        })
        .collect()
}

fn lower_routine(
    name: String,
    function: &FunctionDeclaration,
    callables: &BTreeSet<String>,
) -> Result<Routine, LoweringError> {
    let mut builder = RoutineBuilder::new(name.clone(), callables, &function.parameters);
    builder.lower_block(&function.body)?;
    Ok(Routine {
        name,
        parameters: fields_from_parameters(&function.parameters),
        return_type: type_spec(&function.return_type),
        instructions: builder.instructions,
    })
}

struct RoutineBuilder<'a> {
    name: String,
    callables: &'a BTreeSet<String>,
    instructions: Vec<Instruction>,
    next_value: u32,
    next_binding: u32,
    bindings: BTreeMap<String, String>,
}

impl<'a> RoutineBuilder<'a> {
    fn new(name: String, callables: &'a BTreeSet<String>, parameters: &[Parameter]) -> Self {
        Self {
            name,
            callables,
            instructions: Vec::new(),
            next_value: 0,
            next_binding: 0,
            bindings: parameters
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.name.clone()))
                .collect(),
        }
    }

    fn bind_name(&mut self, source_name: &str) -> Result<String, LoweringError> {
        let index = self.next_binding;
        self.next_binding = self
            .next_binding
            .checked_add(1)
            .ok_or_else(|| LoweringError::RoutineTooLarge(self.name.clone()))?;
        let runtime_name = format!("#local:{index}:{source_name}");
        self.bindings
            .insert(source_name.to_owned(), runtime_name.clone());
        Ok(runtime_name)
    }

    fn resolved_path(&self, path: &aster_syntax::Path) -> String {
        let mut segments = path.segments.clone();
        if let Some(first) = segments.first_mut()
            && let Some(runtime_name) = self.bindings.get(first)
        {
            runtime_name.clone_into(first);
        }
        segments.join(".")
    }

    fn value(&mut self) -> Result<ValueId, LoweringError> {
        let value = ValueId(self.next_value);
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| LoweringError::RoutineTooLarge(self.name.clone()))?;
        Ok(value)
    }

    fn push(&mut self, kind: InstructionKind) -> Result<usize, LoweringError> {
        let index = self.instructions.len();
        let id =
            u32::try_from(index).map_err(|_| LoweringError::RoutineTooLarge(self.name.clone()))?;
        self.instructions.push(Instruction::new(id, kind));
        Ok(index)
    }

    fn position(&self) -> Result<u32, LoweringError> {
        u32::try_from(self.instructions.len())
            .map_err(|_| LoweringError::RoutineTooLarge(self.name.clone()))
    }

    fn lower_block(&mut self, block: &Block) -> Result<(), LoweringError> {
        for statement in &block.statements {
            self.lower_statement(statement)?;
        }
        Ok(())
    }

    fn lower_statement(
        &mut self,
        statement: &aster_syntax::Statement,
    ) -> Result<(), LoweringError> {
        match &statement.kind {
            StatementKind::Let { name, value, .. } => {
                let value = self.lower_expression(value)?;
                let name = self.bind_name(name)?;
                self.push(InstructionKind::Bind { name, value })?;
            }
            StatementKind::Require { condition } => {
                let condition = self.lower_expression(condition)?;
                self.push(InstructionKind::Require { condition })?;
            }
            StatementKind::UpdateState { fields } => {
                let fields = self.lower_fields(fields)?;
                self.push(InstructionKind::UpdateState { fields })?;
            }
            StatementKind::Return { value } => {
                let value = self.lower_expression(value)?;
                self.push(InstructionKind::Return { value })?;
            }
            StatementKind::Expression { expression } => {
                self.lower_expression(expression)?;
            }
        }
        Ok(())
    }

    fn lower_expression(&mut self, expression: &Expression) -> Result<ValueId, LoweringError> {
        match &expression.kind {
            ExpressionKind::Infer {
                prompt,
                arguments,
                model_alias,
            } => self.lower_infer(prompt.as_string(), arguments, model_alias.clone()),
            ExpressionKind::Validate {
                candidate,
                validator,
            } => self.lower_validate(candidate, validator.as_string()),
            ExpressionKind::Observe { action, arguments } => {
                self.lower_observe(action.as_string(), arguments)
            }
            ExpressionKind::Intent { purpose, fields } => {
                self.lower_intent(purpose.as_string(), fields)
            }
            ExpressionKind::Propose {
                action,
                arguments,
                intent,
            } => self.lower_proposal(action.as_string(), arguments, intent),
            ExpressionKind::Authorize { proposal, policy } => {
                let proposal = self.lower_expression(proposal)?;
                let target = self.value()?;
                self.push(InstructionKind::Authorize {
                    target,
                    proposal,
                    policy: policy.as_string(),
                    approval_may_suspend: true,
                })?;
                Ok(target)
            }
            ExpressionKind::Commit { proposal, permit } => self.lower_commit(proposal, permit),
            ExpressionKind::Reconcile {
                receipt,
                observation,
                validator,
            } => self.lower_reconcile(receipt, observation, validator.as_string()),
            ExpressionKind::Try { value } => {
                let result = self.lower_expression(value)?;
                let target = self.value()?;
                self.push(InstructionKind::UnwrapResult { target, result })?;
                Ok(target)
            }
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            } => self.lower_if(condition, then_block, else_block),
            ExpressionKind::Match { value, arms } => self.lower_match(value, arms),
            ExpressionKind::Call { callee, arguments } => {
                if let ExpressionKind::Path { path } = &callee.kind
                    && self.callables.contains(&path.as_string())
                {
                    let target = self.value()?;
                    let arguments = self.lower_arguments(arguments)?;
                    self.push(InstructionKind::Call {
                        target,
                        routine: path.as_string(),
                        arguments,
                    })?;
                    return Ok(target);
                }
                self.evaluate_pure(expression)
            }
            _ => self.evaluate_pure(expression),
        }
    }

    fn lower_infer(
        &mut self,
        prompt: String,
        arguments: &[Argument],
        model_alias: String,
    ) -> Result<ValueId, LoweringError> {
        let target = self.value()?;
        let arguments = self.lower_arguments(arguments)?;
        self.push(InstructionKind::Infer {
            target,
            prompt,
            arguments,
            model_alias,
        })?;
        Ok(target)
    }

    fn lower_validate(
        &mut self,
        candidate: &Expression,
        validator: String,
    ) -> Result<ValueId, LoweringError> {
        let candidate = self.lower_expression(candidate)?;
        let target = self.value()?;
        self.push(InstructionKind::Validate {
            target,
            candidate,
            validator,
        })?;
        Ok(target)
    }

    fn lower_observe(
        &mut self,
        action: String,
        arguments: &[Argument],
    ) -> Result<ValueId, LoweringError> {
        let arguments = self.lower_arguments(arguments)?;
        let target = self.value()?;
        self.push(InstructionKind::Observe {
            target,
            action,
            arguments,
        })?;
        Ok(target)
    }

    fn lower_intent(
        &mut self,
        purpose: String,
        fields: &[FieldInitializer],
    ) -> Result<ValueId, LoweringError> {
        let fields = self.lower_fields(fields)?;
        let target = self.value()?;
        self.push(InstructionKind::ConstructIntent {
            target,
            purpose,
            fields,
        })?;
        Ok(target)
    }

    fn lower_proposal(
        &mut self,
        action: String,
        arguments: &[Argument],
        intent: &Expression,
    ) -> Result<ValueId, LoweringError> {
        let arguments = self.lower_arguments(arguments)?;
        let intent = self.lower_expression(intent)?;
        let target = self.value()?;
        self.push(InstructionKind::ConstructProposal {
            target,
            action,
            arguments,
            intent,
        })?;
        Ok(target)
    }

    fn lower_commit(
        &mut self,
        proposal: &Expression,
        permit: &Expression,
    ) -> Result<ValueId, LoweringError> {
        let proposal = self.lower_expression(proposal)?;
        let permit = self.lower_expression(permit)?;
        let target = self.value()?;
        self.push(InstructionKind::Commit {
            target,
            proposal,
            permit,
        })?;
        Ok(target)
    }

    fn lower_reconcile(
        &mut self,
        receipt: &Expression,
        observation: &Expression,
        validator: String,
    ) -> Result<ValueId, LoweringError> {
        let receipt = self.lower_expression(receipt)?;
        let observation = self.lower_expression(observation)?;
        let target = self.value()?;
        self.push(InstructionKind::Reconcile {
            target,
            receipt,
            observation,
            validator,
        })?;
        Ok(target)
    }

    fn evaluate_pure(&mut self, expression: &Expression) -> Result<ValueId, LoweringError> {
        let target = self.value()?;
        let expression = match &expression.kind {
            ExpressionKind::Unit => PureExpression::Unit,
            ExpressionKind::Bool { value } => PureExpression::Bool { value: *value },
            ExpressionKind::Int { value } => PureExpression::Int { value: *value },
            ExpressionKind::Text { value } => PureExpression::Text {
                value: value.clone(),
            },
            ExpressionKind::Path { path } => PureExpression::Path {
                path: self.resolved_path(path),
            },
            ExpressionKind::List { elements } => PureExpression::List {
                elements: self.lower_pure_elements(elements)?,
            },
            ExpressionKind::Record { path, fields } => PureExpression::Record {
                ty: path.as_string(),
                fields: self.lower_pure_fields(fields)?,
            },
            ExpressionKind::Field { target, field } => PureExpression::Field {
                target: Box::new(self.lower_slot(target)?),
                field: field.clone(),
            },
            ExpressionKind::Unary { operator, operand } => PureExpression::Unary {
                operator: unary_name(*operator).to_owned(),
                operand: Box::new(self.lower_slot(operand)?),
            },
            ExpressionKind::Binary {
                left,
                operator,
                right,
            } => PureExpression::Binary {
                left: Box::new(self.lower_slot(left)?),
                operator: binary_name(*operator).to_owned(),
                right: Box::new(self.lower_slot(right)?),
            },
            ExpressionKind::Call { callee, arguments } => {
                let ExpressionKind::Path { path } = &callee.kind else {
                    return Err(LoweringError::EffectInPureMetadata);
                };
                PureExpression::Call {
                    function: path.as_string(),
                    arguments: self.lower_pure_arguments(arguments)?,
                }
            }
            _ => return Err(LoweringError::EffectInPureMetadata),
        };
        self.push(InstructionKind::Evaluate { target, expression })?;
        Ok(target)
    }

    fn lower_slot(&mut self, expression: &Expression) -> Result<PureExpression, LoweringError> {
        Ok(PureExpression::Slot {
            value: self.lower_expression(expression)?,
        })
    }

    fn lower_pure_elements(
        &mut self,
        expressions: &[Expression],
    ) -> Result<Vec<PureExpression>, LoweringError> {
        expressions
            .iter()
            .map(|expression| self.lower_slot(expression))
            .collect()
    }

    fn lower_pure_arguments(
        &mut self,
        arguments: &[Argument],
    ) -> Result<Vec<NamedExpression>, LoweringError> {
        arguments
            .iter()
            .map(|argument| {
                Ok(NamedExpression {
                    name: argument.name.clone(),
                    value: self.lower_slot(&argument.value)?,
                })
            })
            .collect()
    }

    fn lower_pure_fields(
        &mut self,
        fields: &[FieldInitializer],
    ) -> Result<Vec<NamedExpression>, LoweringError> {
        fields
            .iter()
            .map(|field| {
                Ok(NamedExpression {
                    name: Some(field.name.clone()),
                    value: self.lower_slot(&field.value)?,
                })
            })
            .collect()
    }

    fn lower_arguments(
        &mut self,
        arguments: &[Argument],
    ) -> Result<Vec<NamedValue>, LoweringError> {
        arguments
            .iter()
            .map(|argument| {
                Ok(NamedValue {
                    name: argument.name.clone(),
                    value: self.lower_expression(&argument.value)?,
                })
            })
            .collect()
    }

    fn lower_fields(
        &mut self,
        fields: &[FieldInitializer],
    ) -> Result<Vec<NamedValue>, LoweringError> {
        fields
            .iter()
            .map(|field| {
                Ok(NamedValue {
                    name: Some(field.name.clone()),
                    value: self.lower_expression(&field.value)?,
                })
            })
            .collect()
    }

    fn lower_if(
        &mut self,
        condition: &Expression,
        then_block: &Block,
        else_block: &Block,
    ) -> Result<ValueId, LoweringError> {
        let result = self.value()?;
        let condition = self.lower_expression(condition)?;
        let branch_index = self.push(InstructionKind::Branch {
            condition,
            then_target: 0,
            else_target: 0,
        })?;
        let outer_bindings = self.bindings.clone();
        let then_target = self.position()?;
        let then_value = self.lower_block_value(then_block)?;
        self.push(InstructionKind::Copy {
            target: result,
            source: then_value,
        })?;
        let jump_index = self.push(InstructionKind::Jump { target: 0 })?;
        self.bindings.clone_from(&outer_bindings);
        let else_target = self.position()?;
        let else_value = self.lower_block_value(else_block)?;
        self.push(InstructionKind::Copy {
            target: result,
            source: else_value,
        })?;
        let end = self.position()?;
        self.bindings = outer_bindings;
        self.instructions[branch_index].kind = InstructionKind::Branch {
            condition,
            then_target,
            else_target,
        };
        self.instructions[jump_index].kind = InstructionKind::Jump { target: end };
        Ok(result)
    }

    fn lower_match(
        &mut self,
        value: &Expression,
        arms: &[aster_syntax::MatchArm],
    ) -> Result<ValueId, LoweringError> {
        let result = self.value()?;
        let value = self.lower_expression(value)?;
        let match_index = self.push(InstructionKind::Match {
            value,
            arms: Vec::new(),
        })?;
        let mut targets = Vec::new();
        let mut jumps = Vec::new();
        let outer_bindings = self.bindings.clone();
        for arm in arms {
            self.bindings.clone_from(&outer_bindings);
            let mut pattern = pattern_spec(&arm.pattern);
            if let PatternSpec::Variant {
                binding: Some(binding),
                ..
            } = &mut pattern
            {
                *binding = self.bind_name(binding)?;
            }
            targets.push(MatchTarget {
                pattern,
                target: self.position()?,
            });
            let arm_value = self.lower_expression(&arm.value)?;
            self.push(InstructionKind::Copy {
                target: result,
                source: arm_value,
            })?;
            jumps.push(self.push(InstructionKind::Jump { target: 0 })?);
        }
        self.bindings = outer_bindings;
        let end = self.position()?;
        self.instructions[match_index].kind = InstructionKind::Match {
            value,
            arms: targets,
        };
        for jump in jumps {
            self.instructions[jump].kind = InstructionKind::Jump { target: end };
        }
        Ok(result)
    }

    fn lower_block_value(&mut self, block: &Block) -> Result<ValueId, LoweringError> {
        let Some((last, prefix)) = block.statements.split_last() else {
            return self.evaluate_pure_unit();
        };
        for statement in prefix {
            self.lower_statement(statement)?;
        }
        if let StatementKind::Expression { expression } = &last.kind {
            self.lower_expression(expression)
        } else {
            self.lower_statement(last)?;
            self.evaluate_pure_unit()
        }
    }

    fn evaluate_pure_unit(&mut self) -> Result<ValueId, LoweringError> {
        let target = self.value()?;
        self.push(InstructionKind::Evaluate {
            target,
            expression: PureExpression::Unit,
        })?;
        Ok(target)
    }
}

fn fields_from_parameters(parameters: &[Parameter]) -> Vec<FieldSpec> {
    parameters
        .iter()
        .map(|parameter| FieldSpec {
            name: parameter.name.clone(),
            ty: type_spec(&parameter.ty),
        })
        .collect()
}

fn type_spec(reference: &TypeReference) -> TypeSpec {
    TypeSpec {
        name: reference.path.as_string(),
        arguments: reference.arguments.iter().map(type_spec).collect(),
    }
}

fn capability_spec(value: &CapabilityExpression) -> Result<CapabilitySpec, LoweringError> {
    Ok(CapabilitySpec {
        name: value.path.as_string(),
        arguments: value
            .arguments
            .iter()
            .map(|argument| {
                Ok(NamedExpression {
                    name: argument.name.clone(),
                    value: pure_expression(&argument.value)?,
                })
            })
            .collect::<Result<_, LoweringError>>()?,
    })
}

fn pure_expression(expression: &Expression) -> Result<PureExpression, LoweringError> {
    Ok(match &expression.kind {
        ExpressionKind::Unit => PureExpression::Unit,
        ExpressionKind::Bool { value } => PureExpression::Bool { value: *value },
        ExpressionKind::Int { value } => PureExpression::Int { value: *value },
        ExpressionKind::Text { value } => PureExpression::Text {
            value: value.clone(),
        },
        ExpressionKind::Path { path } => PureExpression::Path {
            path: path.as_string(),
        },
        ExpressionKind::List { elements } => PureExpression::List {
            elements: elements
                .iter()
                .map(pure_expression)
                .collect::<Result<_, _>>()?,
        },
        ExpressionKind::Record { path, fields } => PureExpression::Record {
            ty: path.as_string(),
            fields: pure_fields(fields)?,
        },
        ExpressionKind::Field { target, field } => PureExpression::Field {
            target: Box::new(pure_expression(target)?),
            field: field.clone(),
        },
        ExpressionKind::Unary { operator, operand } => PureExpression::Unary {
            operator: unary_name(*operator).to_owned(),
            operand: Box::new(pure_expression(operand)?),
        },
        ExpressionKind::Binary {
            left,
            operator,
            right,
        } => PureExpression::Binary {
            left: Box::new(pure_expression(left)?),
            operator: binary_name(*operator).to_owned(),
            right: Box::new(pure_expression(right)?),
        },
        ExpressionKind::Call { callee, arguments } => {
            let ExpressionKind::Path { path } = &callee.kind else {
                return Err(LoweringError::EffectInPureMetadata);
            };
            PureExpression::Call {
                function: path.as_string(),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        Ok(NamedExpression {
                            name: argument.name.clone(),
                            value: pure_expression(&argument.value)?,
                        })
                    })
                    .collect::<Result<_, LoweringError>>()?,
            }
        }
        ExpressionKind::If {
            condition,
            then_block,
            else_block,
        } => PureExpression::If {
            condition: Box::new(pure_expression(condition)?),
            then_block: pure_block(then_block)?,
            else_block: pure_block(else_block)?,
        },
        ExpressionKind::Match { value, arms } => PureExpression::Match {
            value: Box::new(pure_expression(value)?),
            arms: arms
                .iter()
                .map(|arm| {
                    Ok(PureMatchArmSpec {
                        pattern: pattern_spec(&arm.pattern),
                        value: pure_expression(&arm.value)?,
                    })
                })
                .collect::<Result<_, LoweringError>>()?,
        },
        ExpressionKind::Try { .. }
        | ExpressionKind::Infer { .. }
        | ExpressionKind::Validate { .. }
        | ExpressionKind::Observe { .. }
        | ExpressionKind::Intent { .. }
        | ExpressionKind::Propose { .. }
        | ExpressionKind::Authorize { .. }
        | ExpressionKind::Commit { .. }
        | ExpressionKind::Reconcile { .. } => return Err(LoweringError::EffectInPureMetadata),
    })
}

fn pure_block(block: &Block) -> Result<PureBlockSpec, LoweringError> {
    let statements = block
        .statements
        .iter()
        .map(|statement| match &statement.kind {
            StatementKind::Let { name, value, .. } => Ok(PureStatementSpec::Let {
                name: name.clone(),
                value: pure_expression(value)?,
            }),
            StatementKind::Require { condition } => Ok(PureStatementSpec::Require {
                condition: pure_expression(condition)?,
            }),
            StatementKind::Expression { expression } => Ok(PureStatementSpec::Expression {
                value: pure_expression(expression)?,
            }),
            StatementKind::UpdateState { .. } | StatementKind::Return { .. } => {
                Err(LoweringError::EffectInPureMetadata)
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(PureBlockSpec { statements })
}

fn pure_fields(fields: &[FieldInitializer]) -> Result<Vec<NamedExpression>, LoweringError> {
    fields
        .iter()
        .map(|field| {
            Ok(NamedExpression {
                name: Some(field.name.clone()),
                value: pure_expression(&field.value)?,
            })
        })
        .collect()
}

const fn unary_name(operator: UnaryOperator) -> &'static str {
    match operator {
        UnaryOperator::Not => "not",
        UnaryOperator::Negate => "negate",
    }
}

const fn binary_name(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "add",
        BinaryOperator::Subtract => "subtract",
        BinaryOperator::Multiply => "multiply",
        BinaryOperator::Divide => "divide",
        BinaryOperator::Equal => "equal",
        BinaryOperator::NotEqual => "not_equal",
        BinaryOperator::Less => "less",
        BinaryOperator::LessEqual => "less_equal",
        BinaryOperator::Greater => "greater",
        BinaryOperator::GreaterEqual => "greater_equal",
        BinaryOperator::And => "and",
        BinaryOperator::Or => "or",
    }
}

fn pattern_spec(pattern: &Pattern) -> PatternSpec {
    match pattern {
        Pattern::Wildcard => PatternSpec::Wildcard,
        Pattern::Variant { path, binding } => PatternSpec::Variant {
            path: path.as_string(),
            binding: binding.clone(),
        },
    }
}
