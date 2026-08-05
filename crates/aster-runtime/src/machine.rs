use std::collections::{BTreeMap, BTreeSet};

use aster_ir::{
    Agent, CapabilitySpec, InstructionKind, NamedExpression, NamedValue, PatternSpec,
    PolicyDecisionSpec, Program, PureExpression, TypeSpec, ValueId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;

use crate::capability::CompiledGrants;
use crate::{
    AuthorityLedger, Budget, BudgetDimension, CapabilityGrants, Intent, Proposal, ReceiptValue,
    Reservation, RuntimeValue, canonical_sha256, snapshot_values,
};

/// Validated inputs needed to start one agent event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StartRequest {
    pub agent: String,
    pub event: String,
    pub event_id: String,
    pub event_time: String,
    pub agent_arguments: BTreeMap<String, JsonValue>,
    pub payload: JsonValue,
    pub state: BTreeMap<String, JsonValue>,
    pub capabilities: CapabilityGrants,
}

/// External effect categories exposed by the VM boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Model,
    Read,
    Approval,
    Write,
}

/// Complete deterministic external request identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectRequest {
    pub kind: EffectKind,
    pub identity: String,
    pub payload: JsonValue,
    pub request_hash: String,
}

/// Typed driver or replay response bound to one request hash.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EffectResolution {
    pub request_hash: String,
    pub payload: JsonValue,
    pub actual_usage: BTreeMap<String, u64>,
}

/// One routine frame; serializable without host closures.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrameSnapshot {
    routine: String,
    instruction_pointer: u32,
    locals: BTreeMap<String, RuntimeValue>,
    slots: BTreeMap<ValueId, RuntimeValue>,
    return_target: Option<ValueId>,
}

/// Pending effect and the slot that receives its typed resolution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PendingEffect {
    request: EffectRequest,
    target: ValueId,
    reservation: Option<Reservation>,
    usage_reservations: BTreeMap<String, Reservation>,
    completion: PendingCompletion,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PendingCompletion {
    Model {
        expected_type: TypeSpec,
    },
    Read {
        expected_type: TypeSpec,
    },
    Approval {
        proposal: Box<Proposal>,
        policy: String,
        expires_at: String,
    },
    Write {
        action: String,
        proposal_hash: String,
        expected_type: TypeSpec,
    },
}

/// Complete versioned continuation at an instruction/effect boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MachineSnapshot {
    pub schema_version: u32,
    pub runtime_version: String,
    pub program_hash: String,
    pub agent: String,
    pub event: String,
    pub event_id: String,
    pub event_time: String,
    pub input_hash: String,
    pub frames: Vec<FrameSnapshot>,
    pub current_state: BTreeMap<String, RuntimeValue>,
    pub pending_state: BTreeMap<String, RuntimeValue>,
    pub budget: Budget,
    pub grant_fingerprint: String,
    grant_request_hashes: BTreeSet<String>,
    pub authority: AuthorityLedger,
    pub trace_position: u64,
    pub trace_hash: String,
    pending_effect: Option<PendingEffect>,
    snapshot_hash: String,
}

impl MachineSnapshot {
    /// Serializes a secret-free, content-hashed continuation.
    ///
    /// # Errors
    ///
    /// Rejects secret values and serialization failures before output.
    pub fn to_json(&self) -> Result<String, MachineError> {
        self.validate_no_secrets()?;
        serde_json::to_string(self).map_err(|error| MachineError::Serialization(error.to_string()))
    }

    /// Decodes and verifies a persisted continuation.
    ///
    /// # Errors
    ///
    /// Rejects malformed JSON, unsupported schema, secrets, or hash mismatch.
    pub fn from_json(json: &str) -> Result<Self, MachineError> {
        let snapshot: Self = serde_json::from_str(json)
            .map_err(|error| MachineError::Serialization(error.to_string()))?;
        if snapshot.schema_version != 1 {
            return Err(MachineError::SnapshotSchemaMismatch);
        }
        snapshot.validate_no_secrets()?;
        if snapshot.compute_hash()? != snapshot.snapshot_hash {
            return Err(MachineError::SnapshotHashMismatch);
        }
        Ok(snapshot)
    }

    fn seal(&mut self) -> Result<(), MachineError> {
        self.snapshot_hash.clear();
        self.snapshot_hash = self.compute_hash()?;
        Ok(())
    }

    fn compute_hash(&self) -> Result<String, MachineError> {
        let mut unhashed = self.clone();
        unhashed.snapshot_hash.clear();
        canonical_sha256(&unhashed).map_err(|error| MachineError::Serialization(error.to_string()))
    }

    fn validate_no_secrets(&self) -> Result<(), MachineError> {
        snapshot_values(&self.current_state)
            .map_err(|_| MachineError::SecretPersistenceRejected)?;
        snapshot_values(&self.pending_state)
            .map_err(|_| MachineError::SecretPersistenceRejected)?;
        for frame in &self.frames {
            snapshot_values(&frame.locals).map_err(|_| MachineError::SecretPersistenceRejected)?;
            let slots: BTreeMap<_, _> = frame
                .slots
                .iter()
                .map(|(id, value)| (id.0.to_string(), value.clone()))
                .collect();
            snapshot_values(&slots).map_err(|_| MachineError::SecretPersistenceRejected)?;
        }
        Ok(())
    }
}

/// Successful event result after atomic state publication.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunOutcome {
    pub state: BTreeMap<String, JsonValue>,
    pub value: JsonValue,
}

/// One pure machine transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    Continue,
    Yield(EffectRequest),
    Completed(RunOutcome),
    Failed(MachineError),
}

enum PolicyRuntimeDecision {
    Allow,
    Approve(Box<RuntimeValue>),
    Deny(String),
}

enum PatternBinding {
    Unbound,
    Bound(String, Box<RuntimeValue>),
}

/// Serializable explicit-instruction machine.
pub struct Machine {
    program: Program,
    snapshot: MachineSnapshot,
    completed: Option<RunOutcome>,
    failed: Option<MachineError>,
}

impl Machine {
    /// Validates an entry point and initializes its explicit frame.
    ///
    /// # Errors
    ///
    /// Rejects unknown agents/events and malformed typed boundary values.
    pub fn start(program: Program, request: StartRequest) -> Result<Self, MachineError> {
        validate_instant(&request.event_time)?;
        let compiled_grants = request
            .capabilities
            .clone()
            .compile()
            .map_err(|error| MachineError::Capability(error.to_string()))?;
        let agent = program
            .agents
            .get(&request.agent)
            .ok_or(MachineError::UnknownAgent)?;
        let routine = program
            .handler(&request.agent, &request.event)
            .ok_or(MachineError::UnknownEvent)?;
        let mut locals = BTreeMap::new();
        for parameter in &agent.parameters {
            let value = request
                .agent_arguments
                .get(&parameter.name)
                .ok_or_else(|| MachineError::MissingInput(parameter.name.clone()))?;
            locals.insert(
                parameter.name.clone(),
                decode_json(value, &parameter.ty, &program)?,
            );
        }
        let handler_parameter = routine
            .parameters
            .first()
            .ok_or(MachineError::MissingHandlerParameter)?;
        locals.insert(
            handler_parameter.name.clone(),
            decode_json(&request.payload, &handler_parameter.ty, &program)?,
        );
        validate_declared_capabilities(&agent.capabilities, &locals, &program, &compiled_grants)?;
        let current_state = initial_state(agent, &request.state, &locals, &program)?;
        let mut self_record = current_state.clone();
        for (name, value) in &locals {
            if agent
                .parameters
                .iter()
                .any(|parameter| parameter.name == *name)
            {
                self_record.insert(name.clone(), value.clone());
            }
        }
        locals.insert("self".to_owned(), RuntimeValue::Record(self_record));
        locals.insert(
            "event".to_owned(),
            RuntimeValue::Record(BTreeMap::from([
                (
                    "id".to_owned(),
                    RuntimeValue::Text(request.event_id.clone()),
                ),
                (
                    "time".to_owned(),
                    RuntimeValue::Text(request.event_time.clone()),
                ),
            ])),
        );
        let input_hash = canonical_sha256(&request)
            .map_err(|error| MachineError::Serialization(error.to_string()))?;
        let limits = budget_limits(&agent.budget);
        let snapshot = MachineSnapshot {
            schema_version: 1,
            runtime_version: env!("CARGO_PKG_VERSION").to_owned(),
            program_hash: program.program_hash.clone(),
            agent: request.agent,
            event: request.event,
            event_id: request.event_id,
            event_time: request.event_time,
            input_hash,
            frames: vec![FrameSnapshot {
                routine: routine.name.clone(),
                instruction_pointer: 0,
                locals,
                slots: BTreeMap::new(),
                return_target: None,
            }],
            current_state,
            pending_state: BTreeMap::new(),
            budget: Budget::new(limits),
            grant_fingerprint: compiled_grants.fingerprint,
            grant_request_hashes: compiled_grants.request_hashes,
            authority: AuthorityLedger::default(),
            trace_position: 0,
            trace_hash: String::new(),
            pending_effect: None,
            snapshot_hash: String::new(),
        };
        Ok(Self {
            program,
            snapshot,
            completed: None,
            failed: None,
        })
    }

    /// Restores a verified snapshot against the exact program hash.
    ///
    /// # Errors
    ///
    /// Rejects a snapshot created for any other program.
    pub fn restore(program: Program, snapshot: MachineSnapshot) -> Result<Self, MachineError> {
        if program.program_hash != snapshot.program_hash {
            return Err(MachineError::ProgramMismatch);
        }
        Ok(Self {
            program,
            snapshot,
            completed: None,
            failed: None,
        })
    }

    /// Executes exactly one pure instruction or reports one suspension/terminal state.
    #[must_use]
    pub fn step(&mut self) -> Step {
        if let Some(error) = &self.failed {
            return Step::Failed(error.clone());
        }
        if let Some(outcome) = &self.completed {
            return Step::Completed(outcome.clone());
        }
        if let Some(pending) = &self.snapshot.pending_effect {
            return Step::Yield(pending.request.clone());
        }
        match self.execute_current() {
            Ok(Some(step)) => step,
            Ok(None) => Step::Continue,
            Err(error) => {
                self.failed = Some(error.clone());
                Step::Failed(error)
            }
        }
    }

    /// Supplies one exactly matching external resolution.
    ///
    /// # Errors
    ///
    /// Rejects missing or mismatched pending requests and malformed payloads.
    pub fn supply(&mut self, resolution: &EffectResolution) -> Result<(), MachineError> {
        let pending = self
            .snapshot
            .pending_effect
            .clone()
            .ok_or(MachineError::NoPendingEffect)?;
        if resolution.request_hash != pending.request.request_hash {
            return Err(MachineError::ResolutionMismatch);
        }
        if resolution
            .actual_usage
            .keys()
            .any(|name| !pending.usage_reservations.contains_key(name))
        {
            return Err(MachineError::UnexpectedUsageDimension);
        }
        if let Some(reservation) = pending.reservation {
            self.snapshot
                .budget
                .settle(reservation, 1)
                .map_err(|error| MachineError::Budget(error.to_string()))?;
        }
        for (name, reservation) in pending.usage_reservations {
            let actual = resolution.actual_usage.get(&name).copied().unwrap_or(0);
            self.snapshot
                .budget
                .settle(reservation, actual)
                .map_err(|error| MachineError::Budget(error.to_string()))?;
        }
        let value = match pending.completion {
            PendingCompletion::Model { expected_type } => {
                RuntimeValue::Result(Ok(Box::new(RuntimeValue::Candidate(Box::new(
                    decode_json(&resolution.payload, &expected_type, &self.program)?,
                )))))
            }
            PendingCompletion::Read { expected_type } => {
                RuntimeValue::Result(Ok(Box::new(RuntimeValue::Observation(Box::new(
                    decode_json(&resolution.payload, &expected_type, &self.program)?,
                )))))
            }
            PendingCompletion::Approval {
                proposal,
                policy,
                expires_at,
            } => {
                let approved = resolution
                    .payload
                    .get("approved")
                    .and_then(JsonValue::as_bool)
                    .ok_or_else(|| MachineError::TypeMismatch("approval response".to_owned()))?;
                if approved {
                    let permit = self.snapshot.authority.issue(
                        &proposal,
                        &policy,
                        &self.snapshot.grant_fingerprint,
                        &expires_at,
                    );
                    RuntimeValue::Result(Ok(Box::new(RuntimeValue::Permit(permit))))
                } else {
                    RuntimeValue::Result(Err("approval denied".to_owned()))
                }
            }
            PendingCompletion::Write {
                action,
                proposal_hash,
                expected_type,
            } => RuntimeValue::Result(Ok(Box::new(RuntimeValue::Receipt(ReceiptValue {
                action,
                proposal_hash,
                value: Box::new(decode_json(
                    &resolution.payload,
                    &expected_type,
                    &self.program,
                )?),
            })))),
        };
        self.frame_mut()?.slots.insert(pending.target, value);
        self.advance()?;
        self.snapshot.pending_effect = None;
        Ok(())
    }

    /// Reserves fixture-declared variable maximum usage before driver invocation.
    ///
    /// # Errors
    ///
    /// Rejects missing pending effects, unknown or duplicate dimensions, and
    /// exhausted budgets without invoking a driver.
    pub fn reserve_pending_usage(
        &mut self,
        maximums: &BTreeMap<String, u64>,
    ) -> Result<(), MachineError> {
        let pending = self
            .snapshot
            .pending_effect
            .as_ref()
            .ok_or(MachineError::NoPendingEffect)?;
        for (name, maximum) in maximums {
            if pending.usage_reservations.contains_key(name) {
                return Err(MachineError::UsageAlreadyReserved);
            }
            let dimension =
                variable_usage_dimension(name).ok_or(MachineError::UnexpectedUsageDimension)?;
            if self.snapshot.budget.remaining(dimension) < *maximum {
                return Err(MachineError::Budget(format!(
                    "budget exhausted for {dimension:?}"
                )));
            }
        }
        let mut reservations = BTreeMap::new();
        for (name, maximum) in maximums {
            let dimension =
                variable_usage_dimension(name).ok_or(MachineError::UnexpectedUsageDimension)?;
            let reservation = self
                .snapshot
                .budget
                .reserve(dimension, *maximum)
                .map_err(|error| MachineError::Budget(error.to_string()))?;
            reservations.insert(name.clone(), reservation);
        }
        self.snapshot
            .pending_effect
            .as_mut()
            .ok_or(MachineError::NoPendingEffect)?
            .usage_reservations
            .extend(reservations);
        Ok(())
    }

    /// Captures a content-hashed, secret-free continuation.
    ///
    /// # Errors
    ///
    /// Rejects secrets and canonical hashing failures.
    pub fn snapshot(&self) -> Result<MachineSnapshot, MachineError> {
        let mut snapshot = self.snapshot.clone();
        snapshot.validate_no_secrets()?;
        snapshot.seal()?;
        Ok(snapshot)
    }

    fn execute_current(&mut self) -> Result<Option<Step>, MachineError> {
        let instruction = self.current_instruction()?.kind.clone();
        if self.execute_local_instruction(&instruction)? {
            return Ok(None);
        }
        match instruction {
            InstructionKind::Infer {
                target,
                prompt,
                arguments,
                model_alias,
            } => {
                return self
                    .yield_inference(target, prompt, &arguments, &model_alias)
                    .map(Some);
            }
            InstructionKind::Validate {
                target,
                candidate,
                validator,
            } => self.execute_validate(target, candidate, &validator)?,
            InstructionKind::Observe {
                target,
                action,
                arguments,
            } => return self.yield_observation(target, action, &arguments).map(Some),
            InstructionKind::ConstructIntent {
                target,
                purpose,
                fields,
            } => self.execute_intent(target, purpose, &fields)?,
            InstructionKind::ConstructProposal {
                target,
                action,
                arguments,
                intent,
            } => self.execute_proposal(target, &action, &arguments, intent)?,
            InstructionKind::Authorize {
                target,
                proposal,
                policy,
                approval_may_suspend,
            } => {
                if let Some(step) =
                    self.execute_authorize(target, proposal, policy, approval_may_suspend)?
                {
                    return Ok(Some(step));
                }
            }
            InstructionKind::Commit {
                target,
                proposal,
                permit,
            } => return self.yield_commit(target, proposal, permit).map(Some),
            InstructionKind::Reconcile {
                target,
                receipt,
                observation,
                validator,
            } => self.execute_reconcile(target, receipt, observation, &validator)?,
            InstructionKind::Require { condition } => {
                if self.slot(condition)? != &RuntimeValue::Bool(true) {
                    return Err(MachineError::RequirementFailed);
                }
                self.advance()?;
            }
            InstructionKind::UpdateState { fields } => {
                let values = self.named_runtime_values(&fields)?;
                self.snapshot.pending_state.extend(values);
                self.advance()?;
            }
            InstructionKind::Return { value } => {
                return self.execute_return(value);
            }
            InstructionKind::Evaluate { .. }
            | InstructionKind::Bind { .. }
            | InstructionKind::Call { .. }
            | InstructionKind::UnwrapResult { .. }
            | InstructionKind::Branch { .. }
            | InstructionKind::Jump { .. }
            | InstructionKind::Match { .. } => {
                return Err(MachineError::UnsupportedInstruction);
            }
        }
        Ok(None)
    }

    fn yield_inference(
        &mut self,
        target: ValueId,
        prompt: String,
        arguments: &[NamedValue],
        model_alias: &str,
    ) -> Result<Step, MachineError> {
        let prompt_spec = self
            .program
            .catalog
            .prompts
            .get(&prompt)
            .cloned()
            .ok_or(MachineError::UnknownPrompt)?;
        self.require_capability(&json!({
            "capability": "ModelUse",
            "arguments": [model_alias],
        }))?;
        let payload = json!({
            "prompt": prompt,
            "instruction": prompt_spec.instruction,
            "model_alias": model_alias,
            "data": self.named_values_json(arguments)?,
            "expected_type": prompt_spec.result_type,
            "source_provenance": self.program.source_hash,
        });
        let reservation = self
            .snapshot
            .budget
            .reserve(BudgetDimension::ModelCalls, 1)
            .map_err(|error| MachineError::Budget(error.to_string()))?;
        let request = effect_request(EffectKind::Model, prompt, payload)?;
        self.snapshot.pending_effect = Some(PendingEffect {
            request: request.clone(),
            target,
            reservation: Some(reservation),
            usage_reservations: BTreeMap::new(),
            completion: PendingCompletion::Model {
                expected_type: prompt_spec.result_type,
            },
        });
        Ok(Step::Yield(request))
    }

    fn execute_return(&mut self, value: ValueId) -> Result<Option<Step>, MachineError> {
        let returned = self.slot(value)?.clone();
        if self.snapshot.frames.len() > 1 {
            let frame = self
                .snapshot
                .frames
                .pop()
                .ok_or(MachineError::MissingFrame)?;
            let target = frame
                .return_target
                .ok_or(MachineError::MissingReturnTarget)?;
            self.frame_mut()?.slots.insert(target, returned);
            return Ok(None);
        }
        let value = runtime_to_json(&returned)?;
        let mut state = self.snapshot.current_state.clone();
        state.append(&mut self.snapshot.pending_state);
        let state = state
            .iter()
            .map(|(name, value)| Ok((name.clone(), runtime_to_json(value)?)))
            .collect::<Result<_, MachineError>>()?;
        let outcome = RunOutcome { state, value };
        self.completed = Some(outcome.clone());
        Ok(Some(Step::Completed(outcome)))
    }

    fn execute_local_instruction(
        &mut self,
        instruction: &InstructionKind,
    ) -> Result<bool, MachineError> {
        match instruction {
            InstructionKind::Evaluate { target, expression } => {
                let value = self.evaluate(expression)?;
                self.frame_mut()?.slots.insert(*target, value);
                self.advance()?;
            }
            InstructionKind::Bind { name, value } => {
                let value = self.slot(*value)?.clone();
                self.frame_mut()?.locals.insert(name.clone(), value);
                self.advance()?;
            }
            InstructionKind::UnwrapResult { target, result } => {
                self.execute_unwrap(*target, *result)?;
            }
            InstructionKind::Call {
                target,
                routine,
                arguments,
            } => self.execute_call(*target, routine, arguments)?,
            InstructionKind::Branch {
                condition,
                then_target,
                else_target,
            } => {
                let target = match self.slot(*condition)? {
                    RuntimeValue::Bool(true) => *then_target,
                    RuntimeValue::Bool(false) => *else_target,
                    _ => return Err(MachineError::TypeMismatch("branch condition".to_owned())),
                };
                self.frame_mut()?.instruction_pointer = target;
            }
            InstructionKind::Jump { target } => {
                self.frame_mut()?.instruction_pointer = *target;
            }
            InstructionKind::Match { value, arms } => {
                let value = self.slot(*value)?.clone();
                let (target, binding) = arms
                    .iter()
                    .find_map(|arm| {
                        match_pattern(&value, &arm.pattern).map(|binding| (arm.target, binding))
                    })
                    .ok_or(MachineError::NonExhaustiveMatch)?;
                if let PatternBinding::Bound(name, value) = binding {
                    self.frame_mut()?.locals.insert(name, *value);
                }
                self.frame_mut()?.instruction_pointer = target;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn execute_unwrap(&mut self, target: ValueId, result: ValueId) -> Result<(), MachineError> {
        match self.slot(result)?.clone() {
            RuntimeValue::Result(Ok(value)) => {
                self.frame_mut()?.slots.insert(target, *value);
                self.advance()
            }
            RuntimeValue::Result(Err(message)) => Err(MachineError::PropagatedError(message)),
            _ => Err(MachineError::TypeMismatch("expected Result".to_owned())),
        }
    }

    fn execute_call(
        &mut self,
        target: ValueId,
        name: &str,
        arguments: &[NamedValue],
    ) -> Result<(), MachineError> {
        let routine = self
            .program
            .routine(name)
            .cloned()
            .ok_or(MachineError::UnknownRoutine)?;
        let values: Vec<_> = arguments
            .iter()
            .map(|argument| self.slot(argument.value).cloned())
            .collect::<Result<_, _>>()?;
        let locals = routine
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| (parameter.name.clone(), value))
            .collect();
        self.advance()?;
        self.snapshot.frames.push(FrameSnapshot {
            routine: routine.name,
            instruction_pointer: 0,
            locals,
            slots: BTreeMap::new(),
            return_target: Some(target),
        });
        Ok(())
    }

    fn execute_validate(
        &mut self,
        target: ValueId,
        candidate: ValueId,
        validator: &str,
    ) -> Result<(), MachineError> {
        let RuntimeValue::Candidate(value) = self.slot(candidate)?.clone() else {
            return Err(MachineError::TypeMismatch("Candidate".to_owned()));
        };
        let valid = self.validator_accepts(validator, vec![(*value).clone()])?;
        let result = if valid {
            RuntimeValue::Result(Ok(Box::new(RuntimeValue::Checked(value))))
        } else {
            RuntimeValue::Result(Err("validation failed".to_owned()))
        };
        self.frame_mut()?.slots.insert(target, result);
        self.advance()
    }

    fn yield_observation(
        &mut self,
        target: ValueId,
        action: String,
        arguments: &[NamedValue],
    ) -> Result<Step, MachineError> {
        let tool = self
            .program
            .catalog
            .tools
            .get(&action)
            .cloned()
            .ok_or(MachineError::UnknownTool)?;
        let arguments_json = self.named_values_json(arguments)?;
        let arguments_map = arguments_json
            .as_object()
            .cloned()
            .ok_or_else(|| MachineError::TypeMismatch("observation arguments".to_owned()))?
            .into_iter()
            .collect();
        let capability_request =
            self.capability_request_json(tool.capability.as_ref(), &arguments_map)?;
        self.require_capability(&capability_request)?;
        let payload = json!({
            "action": action,
            "arguments": arguments_json,
        });
        let reservation = self
            .snapshot
            .budget
            .reserve(BudgetDimension::ExternalReads, 1)
            .map_err(|error| MachineError::Budget(error.to_string()))?;
        let request = effect_request(EffectKind::Read, action, payload)?;
        self.snapshot.pending_effect = Some(PendingEffect {
            request: request.clone(),
            target,
            reservation: Some(reservation),
            usage_reservations: BTreeMap::new(),
            completion: PendingCompletion::Read {
                expected_type: tool.result_type,
            },
        });
        Ok(Step::Yield(request))
    }

    fn execute_intent(
        &mut self,
        target: ValueId,
        purpose: String,
        fields: &[NamedValue],
    ) -> Result<(), MachineError> {
        let mut fields = self.named_values_json(fields)?;
        let object = fields
            .as_object_mut()
            .ok_or_else(|| MachineError::TypeMismatch("intent fields".to_owned()))?;
        let expires_at = object
            .remove("expires_at")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or(MachineError::InvalidInstant)?;
        self.frame_mut()?.slots.insert(
            target,
            RuntimeValue::Intent(Intent {
                purpose,
                fields: object.clone().into_iter().collect(),
                expires_at,
            }),
        );
        self.advance()
    }

    fn execute_proposal(
        &mut self,
        target: ValueId,
        action: &str,
        arguments: &[NamedValue],
        intent: ValueId,
    ) -> Result<(), MachineError> {
        let RuntimeValue::Intent(intent) = self.slot(intent)?.clone() else {
            return Err(MachineError::TypeMismatch("Intent".to_owned()));
        };
        let tool = self
            .program
            .catalog
            .tools
            .get(action)
            .cloned()
            .ok_or(MachineError::UnknownTool)?;
        let arguments_json = self.named_values_json(arguments)?;
        let arguments_map = arguments_json
            .as_object()
            .cloned()
            .ok_or_else(|| MachineError::TypeMismatch("proposal arguments".to_owned()))?
            .into_iter()
            .collect();
        let idempotency_name = tool.idempotency.ok_or(MachineError::MissingIdempotency)?;
        let idempotency_key = arguments_json
            .get(&idempotency_name)
            .and_then(JsonValue::as_str)
            .ok_or(MachineError::MissingIdempotency)?;
        let capability_request =
            self.capability_request_json(tool.capability.as_ref(), &arguments_map)?;
        self.require_capability(&capability_request)?;
        let proposal = Proposal::new(
            action,
            arguments_map,
            intent,
            tool.risk.as_deref().unwrap_or("reversible"),
            capability_request,
            idempotency_key,
            &self.program.program_hash,
        )
        .map_err(|error| MachineError::Serialization(error.to_string()))?;
        self.frame_mut()?
            .slots
            .insert(target, RuntimeValue::Proposal(proposal));
        self.advance()
    }

    fn execute_authorize(
        &mut self,
        target: ValueId,
        proposal: ValueId,
        policy: String,
        approval_may_suspend: bool,
    ) -> Result<Option<Step>, MachineError> {
        let RuntimeValue::Proposal(proposal) = self.slot(proposal)?.clone() else {
            return Err(MachineError::TypeMismatch("Proposal".to_owned()));
        };
        let decision = self.policy_decision(&policy, &proposal)?;
        match decision {
            PolicyRuntimeDecision::Allow => {
                let permit = self.snapshot.authority.issue(
                    &proposal,
                    &policy,
                    &self.snapshot.grant_fingerprint,
                    &proposal.intent.expires_at,
                );
                self.frame_mut()?.slots.insert(
                    target,
                    RuntimeValue::Result(Ok(Box::new(RuntimeValue::Permit(permit)))),
                );
                self.advance()?;
                Ok(None)
            }
            PolicyRuntimeDecision::Approve(principal) if approval_may_suspend => {
                let principal_json = runtime_to_json(&principal)?;
                self.require_capability(&json!({
                    "capability": "HumanApproval",
                    "arguments": [principal_json],
                }))?;
                let payload = json!({
                    "policy": policy,
                    "proposal_hash": proposal.hash(),
                    "principal": principal_json,
                });
                let reservation = self
                    .snapshot
                    .budget
                    .reserve(BudgetDimension::Approvals, 1)
                    .map_err(|error| MachineError::Budget(error.to_string()))?;
                let request = effect_request(EffectKind::Approval, policy.clone(), payload)?;
                self.snapshot.pending_effect = Some(PendingEffect {
                    request: request.clone(),
                    target,
                    reservation: Some(reservation),
                    usage_reservations: BTreeMap::new(),
                    completion: PendingCompletion::Approval {
                        expires_at: proposal.intent.expires_at.clone(),
                        proposal: Box::new(proposal),
                        policy,
                    },
                });
                Ok(Some(Step::Yield(request)))
            }
            PolicyRuntimeDecision::Approve(_) => Err(MachineError::ApprovalNotRepresentable),
            PolicyRuntimeDecision::Deny(reason) => {
                self.frame_mut()?
                    .slots
                    .insert(target, RuntimeValue::Result(Err(reason)));
                self.advance()?;
                Ok(None)
            }
        }
    }

    fn yield_commit(
        &mut self,
        target: ValueId,
        proposal: ValueId,
        permit: ValueId,
    ) -> Result<Step, MachineError> {
        let RuntimeValue::Proposal(proposal) = self.slot(proposal)?.clone() else {
            return Err(MachineError::TypeMismatch("Proposal".to_owned()));
        };
        let RuntimeValue::Permit(permit) = self.slot(permit)?.clone() else {
            return Err(MachineError::TypeMismatch("Permit".to_owned()));
        };
        self.require_capability(&proposal.capability_request)?;
        self.snapshot
            .authority
            .consume(
                &proposal,
                &permit,
                &self.snapshot.grant_fingerprint,
                &self.snapshot.event_time,
            )
            .map_err(|error| MachineError::Authority(error.to_string()))?;
        let reservation = self
            .snapshot
            .budget
            .reserve(BudgetDimension::ExternalWrites, 1)
            .map_err(|error| MachineError::Budget(error.to_string()))?;
        let action = proposal.action.clone();
        let expected_type = self
            .program
            .catalog
            .tools
            .get(&action)
            .map(|tool| tool.result_type.clone())
            .ok_or(MachineError::UnknownTool)?;
        let proposal_hash = proposal.hash().to_owned();
        let payload = json!({
            "action": action,
            "arguments": proposal.arguments,
            "proposal_hash": proposal_hash,
        });
        let request = effect_request(EffectKind::Write, action.clone(), payload)?;
        self.snapshot.pending_effect = Some(PendingEffect {
            request: request.clone(),
            target,
            reservation: Some(reservation),
            usage_reservations: BTreeMap::new(),
            completion: PendingCompletion::Write {
                action,
                proposal_hash,
                expected_type,
            },
        });
        Ok(Step::Yield(request))
    }

    fn execute_reconcile(
        &mut self,
        target: ValueId,
        receipt: ValueId,
        observation: ValueId,
        validator: &str,
    ) -> Result<(), MachineError> {
        let RuntimeValue::Receipt(receipt) = self.slot(receipt)?.clone() else {
            return Err(MachineError::TypeMismatch("Receipt".to_owned()));
        };
        let RuntimeValue::Observation(observation) = self.slot(observation)?.clone() else {
            return Err(MachineError::TypeMismatch("Observation".to_owned()));
        };
        let valid = self.validator_accepts(
            validator,
            vec![(*receipt.value).clone(), (*observation).clone()],
        )?;
        let value = if valid {
            RuntimeValue::Result(Ok(Box::new(RuntimeValue::Reconciled(receipt))))
        } else {
            RuntimeValue::Result(Err("reconciliation failed".to_owned()))
        };
        self.frame_mut()?.slots.insert(target, value);
        self.advance()
    }

    fn evaluate(&self, expression: &PureExpression) -> Result<RuntimeValue, MachineError> {
        let frame = self.frame()?;
        eval_expression(expression, &frame.locals, &frame.slots, &self.program)
    }

    fn named_values_json(&self, values: &[NamedValue]) -> Result<JsonValue, MachineError> {
        let mut result = serde_json::Map::new();
        for (index, argument) in values.iter().enumerate() {
            let name = argument.name.clone().unwrap_or_else(|| index.to_string());
            result.insert(name, runtime_to_json(self.slot(argument.value)?)?);
        }
        Ok(JsonValue::Object(result))
    }

    fn named_runtime_values(
        &self,
        values: &[NamedValue],
    ) -> Result<BTreeMap<String, RuntimeValue>, MachineError> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                Ok((
                    value.name.clone().unwrap_or_else(|| index.to_string()),
                    self.slot(value.value)?.clone(),
                ))
            })
            .collect()
    }

    fn validator_accepts(
        &self,
        name: &str,
        values: Vec<RuntimeValue>,
    ) -> Result<bool, MachineError> {
        let validator = self
            .program
            .catalog
            .validators
            .get(name)
            .ok_or(MachineError::UnknownValidator)?;
        let locals: BTreeMap<_, _> = validator
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| (parameter.name.clone(), value))
            .collect();
        let slots = BTreeMap::new();
        for requirement in &validator.requirements {
            if eval_expression(requirement, &locals, &slots, &self.program)?
                != RuntimeValue::Bool(true)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn policy_decision(
        &self,
        name: &str,
        proposal: &Proposal,
    ) -> Result<PolicyRuntimeDecision, MachineError> {
        let policy = self
            .program
            .catalog
            .policies
            .get(name)
            .ok_or(MachineError::UnknownPolicy)?;
        let state = self
            .frame()?
            .locals
            .get("self")
            .cloned()
            .ok_or(MachineError::UnknownValue)?;
        let locals: BTreeMap<_, _> = policy
            .parameters
            .iter()
            .zip([RuntimeValue::Proposal(proposal.clone()), state])
            .map(|(parameter, value)| (parameter.name.clone(), value))
            .collect();
        let slots = BTreeMap::new();
        for rule in &policy.rules {
            let matches = match &rule.condition {
                Some(condition) => {
                    eval_expression(condition, &locals, &slots, &self.program)?
                        == RuntimeValue::Bool(true)
                }
                None => true,
            };
            if !matches {
                continue;
            }
            return match &rule.decision {
                PolicyDecisionSpec::Allow => Ok(PolicyRuntimeDecision::Allow),
                PolicyDecisionSpec::Approve(principal) => Ok(PolicyRuntimeDecision::Approve(
                    Box::new(eval_expression(principal, &locals, &slots, &self.program)?),
                )),
                PolicyDecisionSpec::Deny(reason) => {
                    let reason = eval_expression(reason, &locals, &slots, &self.program)?;
                    let RuntimeValue::Text(reason) = reason else {
                        return Err(MachineError::TypeMismatch("policy reason".to_owned()));
                    };
                    Ok(PolicyRuntimeDecision::Deny(reason))
                }
            };
        }
        Err(MachineError::NonTotalPolicy)
    }

    fn capability_request_json(
        &self,
        capability: Option<&CapabilitySpec>,
        tool_arguments: &BTreeMap<String, JsonValue>,
    ) -> Result<JsonValue, MachineError> {
        let Some(capability) = capability else {
            return Ok(JsonValue::Null);
        };
        let frame = self.frame()?;
        let mut locals = frame.locals.clone();
        for (name, value) in tool_arguments {
            locals.insert(name.clone(), json_to_runtime(value)?);
        }
        let arguments = capability
            .arguments
            .iter()
            .map(|argument| {
                eval_expression(&argument.value, &locals, &frame.slots, &self.program)
                    .and_then(|value| runtime_to_json(&value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({"capability": capability.name, "arguments": arguments}))
    }

    fn require_capability(&self, request: &JsonValue) -> Result<(), MachineError> {
        if request.is_null() {
            return Ok(());
        }
        let request_hash = canonical_sha256(request)
            .map_err(|error| MachineError::Serialization(error.to_string()))?;
        if self.snapshot.grant_request_hashes.contains(&request_hash) {
            Ok(())
        } else {
            Err(MachineError::MissingCapability)
        }
    }

    fn current_instruction(&self) -> Result<&aster_ir::Instruction, MachineError> {
        let frame = self.frame()?;
        let routine = self
            .program
            .routines
            .get(&frame.routine)
            .ok_or(MachineError::UnknownRoutine)?;
        routine
            .instructions
            .get(usize::try_from(frame.instruction_pointer).map_err(|_| MachineError::InvalidIp)?)
            .ok_or(MachineError::InvalidIp)
    }

    fn frame(&self) -> Result<&FrameSnapshot, MachineError> {
        self.snapshot
            .frames
            .last()
            .ok_or(MachineError::MissingFrame)
    }

    fn frame_mut(&mut self) -> Result<&mut FrameSnapshot, MachineError> {
        self.snapshot
            .frames
            .last_mut()
            .ok_or(MachineError::MissingFrame)
    }

    fn slot(&self, id: ValueId) -> Result<&RuntimeValue, MachineError> {
        self.frame()?
            .slots
            .get(&id)
            .ok_or(MachineError::UnknownValue)
    }

    fn advance(&mut self) -> Result<(), MachineError> {
        let frame = self.frame_mut()?;
        frame.instruction_pointer = frame
            .instruction_pointer
            .checked_add(1)
            .ok_or(MachineError::InvalidIp)?;
        Ok(())
    }
}

fn validate_declared_capabilities(
    capabilities: &[CapabilitySpec],
    locals: &BTreeMap<String, RuntimeValue>,
    program: &Program,
    grants: &CompiledGrants,
) -> Result<(), MachineError> {
    for capability in capabilities {
        let arguments = capability
            .arguments
            .iter()
            .map(|argument| {
                eval_expression(&argument.value, locals, &BTreeMap::new(), program)
                    .and_then(|value| runtime_to_json(&value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let request = json!({
            "capability": capability.name,
            "arguments": arguments,
        });
        if !grants
            .permits(&request)
            .map_err(|error| MachineError::Capability(error.to_string()))?
        {
            return Err(MachineError::MissingCapability);
        }
    }
    Ok(())
}

fn initial_state(
    agent: &Agent,
    supplied: &BTreeMap<String, JsonValue>,
    locals: &BTreeMap<String, RuntimeValue>,
    program: &Program,
) -> Result<BTreeMap<String, RuntimeValue>, MachineError> {
    if supplied
        .keys()
        .any(|name| !agent.state.iter().any(|field| field.name.as_str() == name))
    {
        return Err(MachineError::UnknownStateField);
    }
    agent
        .state
        .iter()
        .map(|field| {
            let value = match supplied.get(&field.name) {
                Some(value) => decode_json(value, &field.ty, program)?,
                None => eval_expression(&field.default, locals, &BTreeMap::new(), program)?,
            };
            Ok((field.name.clone(), value))
        })
        .collect()
}

fn budget_limits(values: &BTreeMap<String, u64>) -> BTreeMap<BudgetDimension, u64> {
    values
        .iter()
        .filter_map(|(name, value)| budget_dimension(name).map(|dimension| (dimension, *value)))
        .collect()
}

fn effect_request(
    kind: EffectKind,
    identity: String,
    payload: JsonValue,
) -> Result<EffectRequest, MachineError> {
    let request_hash = canonical_sha256(&json!({
        "kind": kind,
        "identity": identity,
        "payload": payload,
    }))
    .map_err(|error| MachineError::Serialization(error.to_string()))?;
    Ok(EffectRequest {
        kind,
        identity,
        payload,
        request_hash,
    })
}

fn eval_expression(
    expression: &PureExpression,
    locals: &BTreeMap<String, RuntimeValue>,
    slots: &BTreeMap<ValueId, RuntimeValue>,
    program: &Program,
) -> Result<RuntimeValue, MachineError> {
    match expression {
        PureExpression::Unit => Ok(RuntimeValue::Unit),
        PureExpression::Bool { value } => Ok(RuntimeValue::Bool(*value)),
        PureExpression::Int { value } => Ok(RuntimeValue::Int(*value)),
        PureExpression::Text { value } => Ok(RuntimeValue::Text(value.clone())),
        PureExpression::Path { path } => resolve_path(path, locals),
        PureExpression::Slot { value } => {
            slots.get(value).cloned().ok_or(MachineError::UnknownValue)
        }
        PureExpression::List { elements } => elements
            .iter()
            .map(|value| eval_expression(value, locals, slots, program))
            .collect::<Result<_, _>>()
            .map(RuntimeValue::List),
        PureExpression::Record { fields, .. } => {
            eval_named_expressions(fields, locals, slots, program).map(RuntimeValue::Record)
        }
        PureExpression::Field { target, field } => {
            project(eval_expression(target, locals, slots, program)?, field)
        }
        PureExpression::Unary { operator, operand } => {
            eval_unary(operator, eval_expression(operand, locals, slots, program)?)
        }
        PureExpression::Binary {
            left,
            operator,
            right,
        } => eval_binary(
            operator,
            eval_expression(left, locals, slots, program)?,
            eval_expression(right, locals, slots, program)?,
        ),
        PureExpression::Call {
            function,
            arguments,
        } => {
            let values = arguments
                .iter()
                .map(|argument| eval_expression(&argument.value, locals, slots, program))
                .collect::<Result<Vec<_>, _>>()?;
            eval_call(function, values, program)
        }
    }
}

fn eval_named_expressions(
    fields: &[NamedExpression],
    locals: &BTreeMap<String, RuntimeValue>,
    slots: &BTreeMap<ValueId, RuntimeValue>,
    program: &Program,
) -> Result<BTreeMap<String, RuntimeValue>, MachineError> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            Ok((
                field.name.clone().unwrap_or_else(|| index.to_string()),
                eval_expression(&field.value, locals, slots, program)?,
            ))
        })
        .collect()
}

fn resolve_path(
    path: &str,
    locals: &BTreeMap<String, RuntimeValue>,
) -> Result<RuntimeValue, MachineError> {
    let mut segments = path.split('.');
    let first = segments.next().ok_or(MachineError::UnknownValue)?;
    let mut value = match first {
        "Unit" => RuntimeValue::Unit,
        "None" => RuntimeValue::Option(None),
        name => locals
            .get(name)
            .cloned()
            .ok_or(MachineError::UnknownValue)?,
    };
    for field in segments {
        value = project(value, field)?;
    }
    Ok(value)
}

fn eval_unary(operator: &str, operand: RuntimeValue) -> Result<RuntimeValue, MachineError> {
    match (operator, operand) {
        ("not", RuntimeValue::Bool(value)) => Ok(RuntimeValue::Bool(!value)),
        ("negate", RuntimeValue::Int(value)) => value
            .checked_neg()
            .map(RuntimeValue::Int)
            .ok_or_else(|| MachineError::TypeMismatch("integer overflow".to_owned())),
        _ => Err(MachineError::TypeMismatch("unary operand".to_owned())),
    }
}

fn eval_binary(
    operator: &str,
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<RuntimeValue, MachineError> {
    match operator {
        "equal" => Ok(RuntimeValue::Bool(left == right)),
        "not_equal" => Ok(RuntimeValue::Bool(left != right)),
        "and" | "or" => match (left, right) {
            (RuntimeValue::Bool(left), RuntimeValue::Bool(right)) => {
                Ok(RuntimeValue::Bool(if operator == "and" {
                    left && right
                } else {
                    left || right
                }))
            }
            _ => Err(MachineError::TypeMismatch("boolean operands".to_owned())),
        },
        _ => eval_integer_binary(operator, left, right),
    }
}

fn eval_integer_binary(
    operator: &str,
    left: RuntimeValue,
    right: RuntimeValue,
) -> Result<RuntimeValue, MachineError> {
    let (RuntimeValue::Int(left), RuntimeValue::Int(right)) = (left, right) else {
        return Err(MachineError::TypeMismatch("integer operands".to_owned()));
    };
    match operator {
        "add" => left.checked_add(right).map(RuntimeValue::Int),
        "subtract" => left.checked_sub(right).map(RuntimeValue::Int),
        "multiply" => left.checked_mul(right).map(RuntimeValue::Int),
        "divide" => left.checked_div(right).map(RuntimeValue::Int),
        "less" => return Ok(RuntimeValue::Bool(left < right)),
        "less_equal" => return Ok(RuntimeValue::Bool(left <= right)),
        "greater" => return Ok(RuntimeValue::Bool(left > right)),
        "greater_equal" => return Ok(RuntimeValue::Bool(left >= right)),
        _ => return Err(MachineError::UnsupportedPureExpression),
    }
    .ok_or_else(|| MachineError::TypeMismatch("integer arithmetic".to_owned()))
}

fn eval_call(
    function: &str,
    values: Vec<RuntimeValue>,
    program: &Program,
) -> Result<RuntimeValue, MachineError> {
    match (function, values.as_slice()) {
        ("Ok", [value]) => Ok(RuntimeValue::Result(Ok(Box::new(value.clone())))),
        ("Err", [RuntimeValue::Text(message)]) => Ok(RuntimeValue::Result(Err(message.clone()))),
        ("Some", [value]) => Ok(RuntimeValue::Option(Some(Box::new(value.clone())))),
        ("Human", [value]) => Ok(value.clone()),
        ("len", [RuntimeValue::List(values)]) => i64::try_from(values.len())
            .map(RuntimeValue::Int)
            .map_err(|_| MachineError::TypeMismatch("list length".to_owned())),
        ("first", [RuntimeValue::List(values)]) => Ok(values.first().cloned().map_or_else(
            || RuntimeValue::Result(Err("empty list".to_owned())),
            |value| RuntimeValue::Result(Ok(Box::new(value))),
        )),
        ("contains", [RuntimeValue::List(values), value]) => {
            Ok(RuntimeValue::Bool(values.contains(value)))
        }
        ("subset", [RuntimeValue::List(left), RuntimeValue::List(right)]) => Ok(
            RuntimeValue::Bool(left.iter().all(|value| right.contains(value))),
        ),
        ("provenance", [value]) => {
            let json = runtime_to_json(value)?;
            canonical_sha256(&json)
                .map(RuntimeValue::Text)
                .map_err(|error| MachineError::Serialization(error.to_string()))
        }
        ("add_seconds", [RuntimeValue::Text(instant), RuntimeValue::Int(seconds)]) => {
            add_seconds(instant, *seconds).map(RuntimeValue::Text)
        }
        _ if program.routine(function).is_some() => eval_routine(function, values, program),
        _ => Err(MachineError::UnsupportedPureExpression),
    }
}

fn eval_routine(
    name: &str,
    arguments: Vec<RuntimeValue>,
    program: &Program,
) -> Result<RuntimeValue, MachineError> {
    let routine = program.routine(name).ok_or(MachineError::UnknownRoutine)?;
    let mut locals: BTreeMap<_, _> = routine
        .parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, value)| (parameter.name.clone(), value))
        .collect();
    let mut slots = BTreeMap::new();
    let mut ip = 0_usize;
    loop {
        let instruction = routine
            .instructions
            .get(ip)
            .ok_or(MachineError::InvalidIp)?;
        match &instruction.kind {
            InstructionKind::Evaluate { target, expression } => {
                slots.insert(
                    *target,
                    eval_expression(expression, &locals, &slots, program)?,
                );
                ip = ip.checked_add(1).ok_or(MachineError::InvalidIp)?;
            }
            InstructionKind::Bind { name, value } => {
                let value = slots
                    .get(value)
                    .cloned()
                    .ok_or(MachineError::UnknownValue)?;
                locals.insert(name.clone(), value);
                ip = ip.checked_add(1).ok_or(MachineError::InvalidIp)?;
            }
            InstructionKind::Call {
                target,
                routine,
                arguments,
            } => {
                let values = arguments
                    .iter()
                    .map(|argument| {
                        slots
                            .get(&argument.value)
                            .cloned()
                            .ok_or(MachineError::UnknownValue)
                    })
                    .collect::<Result<_, _>>()?;
                slots.insert(*target, eval_routine(routine, values, program)?);
                ip = ip.checked_add(1).ok_or(MachineError::InvalidIp)?;
            }
            InstructionKind::Branch {
                condition,
                then_target,
                else_target,
            } => {
                ip = usize::try_from(match slots.get(condition) {
                    Some(RuntimeValue::Bool(true)) => *then_target,
                    Some(RuntimeValue::Bool(false)) => *else_target,
                    _ => return Err(MachineError::TypeMismatch("branch condition".to_owned())),
                })
                .map_err(|_| MachineError::InvalidIp)?;
            }
            InstructionKind::Jump { target } => {
                ip = usize::try_from(*target).map_err(|_| MachineError::InvalidIp)?;
            }
            InstructionKind::Return { value } => {
                return slots.get(value).cloned().ok_or(MachineError::UnknownValue);
            }
            _ => return Err(MachineError::EffectInPureRoutine),
        }
    }
}

fn add_seconds(instant: &str, seconds: i64) -> Result<String, MachineError> {
    let (year, month, day, hour, minute, second) = instant_parts(instant)?;
    let total = days_from_civil(year, month, day)
        .checked_mul(86_400)
        .and_then(|value| value.checked_add(i64::from(hour) * 3_600))
        .and_then(|value| value.checked_add(i64::from(minute) * 60))
        .and_then(|value| value.checked_add(i64::from(second)))
        .and_then(|value| value.checked_add(seconds))
        .ok_or(MachineError::InvalidInstant)?;
    let days = total.div_euclid(86_400);
    let seconds_of_day = total.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    if !(1..=9_999).contains(&year) {
        return Err(MachineError::InvalidInstant);
    }
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    ))
}

fn validate_instant(instant: &str) -> Result<(), MachineError> {
    instant_parts(instant).map(|_| ())
}

fn instant_parts(instant: &str) -> Result<(i64, u32, u32, u32, u32, u32), MachineError> {
    let bytes = instant.as_bytes();
    if bytes.len() != 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return Err(MachineError::InvalidInstant);
    }
    let year = parse_decimal(&instant[0..4])?;
    let month =
        u32::try_from(parse_decimal(&instant[5..7])?).map_err(|_| MachineError::InvalidInstant)?;
    let day =
        u32::try_from(parse_decimal(&instant[8..10])?).map_err(|_| MachineError::InvalidInstant)?;
    let hour = u32::try_from(parse_decimal(&instant[11..13])?)
        .map_err(|_| MachineError::InvalidInstant)?;
    let minute = u32::try_from(parse_decimal(&instant[14..16])?)
        .map_err(|_| MachineError::InvalidInstant)?;
    let second = u32::try_from(parse_decimal(&instant[17..19])?)
        .map_err(|_| MachineError::InvalidInstant)?;
    let max_day = days_in_month(year, month).ok_or(MachineError::InvalidInstant)?;
    if year == 0 || day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return Err(MachineError::InvalidInstant);
    }
    Ok((year, month, day, hour, minute, second))
}

fn parse_decimal(value: &str) -> Result<i64, MachineError> {
    if value.bytes().all(|byte| byte.is_ascii_digit()) {
        value.parse().map_err(|_| MachineError::InvalidInstant)
    } else {
        Err(MachineError::InvalidInstant)
    }
}

fn days_in_month(year: i64, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let zero_based = days + 719_468;
    let era = zero_based.div_euclid(146_097);
    let day_of_era = zero_based - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn decode_json(
    value: &JsonValue,
    ty: &TypeSpec,
    program: &Program,
) -> Result<RuntimeValue, MachineError> {
    if let Some(target) = program.catalog.aliases.get(&ty.name) {
        return decode_json(value, target, program);
    }
    match (ty.name.as_str(), ty.arguments.as_slice()) {
        ("Incoming", [inner]) => Ok(RuntimeValue::Incoming(Box::new(decode_json(
            value, inner, program,
        )?))),
        ("Untrusted", [inner]) => Ok(RuntimeValue::Untrusted(Box::new(decode_json(
            value, inner, program,
        )?))),
        ("Option", [_]) if value.is_null() => Ok(RuntimeValue::Option(None)),
        ("Option", [inner]) => Ok(RuntimeValue::Option(Some(Box::new(decode_json(
            value, inner, program,
        )?)))),
        ("List", [inner]) => value
            .as_array()
            .ok_or_else(|| MachineError::TypeMismatch("List".to_owned()))?
            .iter()
            .map(|element| decode_json(element, inner, program))
            .collect::<Result<Vec<_>, _>>()
            .map(RuntimeValue::List),
        ("Unit", []) if value.is_null() => Ok(RuntimeValue::Unit),
        ("Bool", []) => value
            .as_bool()
            .map(RuntimeValue::Bool)
            .ok_or_else(|| MachineError::TypeMismatch("Bool".to_owned())),
        ("Int" | "Duration", []) => value
            .as_i64()
            .map(RuntimeValue::Int)
            .ok_or_else(|| MachineError::TypeMismatch(ty.name.clone())),
        ("Text" | "Error", []) => value
            .as_str()
            .map(|value| RuntimeValue::Text(value.to_owned()))
            .ok_or_else(|| MachineError::TypeMismatch(ty.name.clone())),
        ("Instant", []) => {
            let instant = value
                .as_str()
                .ok_or_else(|| MachineError::TypeMismatch("Instant".to_owned()))?;
            validate_instant(instant)?;
            Ok(RuntimeValue::Text(instant.to_owned()))
        }
        (name, []) if program.catalog.records.contains_key(name) => {
            let object = value
                .as_object()
                .ok_or_else(|| MachineError::TypeMismatch(name.to_owned()))?;
            let fields = program
                .catalog
                .records
                .get(name)
                .ok_or(MachineError::UnknownValue)?;
            if object
                .keys()
                .any(|key| !fields.iter().any(|field| field.name.as_str() == key))
            {
                return Err(MachineError::TypeMismatch(format!(
                    "unknown field in {name}"
                )));
            }
            fields
                .iter()
                .map(|field| {
                    let value = object
                        .get(&field.name)
                        .ok_or_else(|| MachineError::MissingInput(field.name.clone()))?;
                    Ok((field.name.clone(), decode_json(value, &field.ty, program)?))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()
                .map(RuntimeValue::Record)
        }
        _ => Err(MachineError::TypeMismatch(ty.name.clone())),
    }
}

fn json_to_runtime(value: &JsonValue) -> Result<RuntimeValue, MachineError> {
    match value {
        JsonValue::Null => Ok(RuntimeValue::Option(None)),
        JsonValue::Bool(value) => Ok(RuntimeValue::Bool(*value)),
        JsonValue::Number(value) => value
            .as_i64()
            .map(RuntimeValue::Int)
            .ok_or_else(|| MachineError::TypeMismatch("integer".to_owned())),
        JsonValue::String(value) => Ok(RuntimeValue::Text(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_to_runtime)
            .collect::<Result<_, _>>()
            .map(RuntimeValue::List),
        JsonValue::Object(values) => values
            .iter()
            .map(|(name, value)| Ok((name.clone(), json_to_runtime(value)?)))
            .collect::<Result<_, _>>()
            .map(RuntimeValue::Record),
    }
}

fn runtime_to_json(value: &RuntimeValue) -> Result<JsonValue, MachineError> {
    match value {
        RuntimeValue::Unit | RuntimeValue::Option(None) => Ok(JsonValue::Null),
        RuntimeValue::Bool(value) => Ok(json!(value)),
        RuntimeValue::Int(value) => Ok(json!(value)),
        RuntimeValue::Text(value) => Ok(json!(value)),
        RuntimeValue::List(values) => values
            .iter()
            .map(runtime_to_json)
            .collect::<Result<_, _>>()
            .map(JsonValue::Array),
        RuntimeValue::Record(values) => values
            .iter()
            .map(|(name, value)| Ok((name.clone(), runtime_to_json(value)?)))
            .collect::<Result<_, _>>()
            .map(JsonValue::Object),
        RuntimeValue::Option(Some(value))
        | RuntimeValue::Incoming(value)
        | RuntimeValue::Untrusted(value)
        | RuntimeValue::Candidate(value)
        | RuntimeValue::Checked(value)
        | RuntimeValue::Observation(value)
        | RuntimeValue::Result(Ok(value)) => runtime_to_json(value),
        RuntimeValue::Result(Err(message)) => Ok(json!({"error": message})),
        RuntimeValue::Intent(intent) => serde_json::to_value(intent)
            .map_err(|error| MachineError::Serialization(error.to_string())),
        RuntimeValue::Proposal(proposal) => serde_json::to_value(proposal)
            .map_err(|error| MachineError::Serialization(error.to_string())),
        RuntimeValue::Permit(_) => Err(MachineError::AuthorityValueOpaque),
        RuntimeValue::Receipt(receipt) | RuntimeValue::Reconciled(receipt) => {
            runtime_to_json(&receipt.value)
        }
        RuntimeValue::Secret(_) => Err(MachineError::SecretPersistenceRejected),
    }
}

fn project(value: RuntimeValue, field: &str) -> Result<RuntimeValue, MachineError> {
    match (value, field) {
        (
            RuntimeValue::Incoming(value)
            | RuntimeValue::Untrusted(value)
            | RuntimeValue::Checked(value)
            | RuntimeValue::Observation(value),
            "value",
        ) => Ok(*value),
        (RuntimeValue::Record(mut fields), name) => {
            fields.remove(name).ok_or(MachineError::UnknownValue)
        }
        (RuntimeValue::Proposal(proposal), "args") => proposal
            .arguments
            .iter()
            .map(|(name, value)| Ok((name.clone(), json_to_runtime(value)?)))
            .collect::<Result<_, _>>()
            .map(RuntimeValue::Record),
        (RuntimeValue::Proposal(proposal), "intent") => Ok(RuntimeValue::Intent(proposal.intent)),
        (RuntimeValue::Proposal(proposal), "risk") => Ok(RuntimeValue::Text(proposal.risk)),
        (RuntimeValue::Proposal(proposal), "action") => Ok(RuntimeValue::Text(proposal.action)),
        (RuntimeValue::Proposal(proposal), "idempotency_key") => {
            Ok(RuntimeValue::Text(proposal.idempotency_key))
        }
        (RuntimeValue::Receipt(receipt) | RuntimeValue::Reconciled(receipt), "value") => {
            Ok(*receipt.value)
        }
        _ => Err(MachineError::UnknownValue),
    }
}

fn match_pattern(value: &RuntimeValue, pattern: &PatternSpec) -> Option<PatternBinding> {
    match pattern {
        PatternSpec::Wildcard => Some(PatternBinding::Unbound),
        PatternSpec::Variant { path, binding } => {
            let variant = path.rsplit('.').next()?;
            let payload = match (variant, value) {
                ("None", RuntimeValue::Option(None)) => None,
                ("Some", RuntimeValue::Option(Some(value)))
                | ("Ok", RuntimeValue::Result(Ok(value))) => Some((**value).clone()),
                ("Err", RuntimeValue::Result(Err(message))) => {
                    Some(RuntimeValue::Text(message.clone()))
                }
                _ => return None,
            };
            Some(
                binding
                    .as_ref()
                    .zip(payload)
                    .map_or(PatternBinding::Unbound, |(name, value)| {
                        PatternBinding::Bound(name.clone(), Box::new(value))
                    }),
            )
        }
    }
}

fn budget_dimension(name: &str) -> Option<BudgetDimension> {
    match name {
        "model_calls" => Some(BudgetDimension::ModelCalls),
        "model_tokens" => Some(BudgetDimension::ModelTokens),
        "external_reads" => Some(BudgetDimension::ExternalReads),
        "external_writes" => Some(BudgetDimension::ExternalWrites),
        "approvals" => Some(BudgetDimension::Approvals),
        "money_microunits" => Some(BudgetDimension::MoneyMicrounits),
        _ => None,
    }
}

fn variable_usage_dimension(name: &str) -> Option<BudgetDimension> {
    match name {
        "model_tokens" => Some(BudgetDimension::ModelTokens),
        "money_microunits" => Some(BudgetDimension::MoneyMicrounits),
        _ => None,
    }
}

/// Controlled VM boundary and invariant failures.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MachineError {
    #[error("unknown agent")]
    UnknownAgent,
    #[error("unknown event")]
    UnknownEvent,
    #[error("unknown routine")]
    UnknownRoutine,
    #[error("missing handler parameter")]
    MissingHandlerParameter,
    #[error("missing input `{0}`")]
    MissingInput(String),
    #[error("initial state contains an unknown field")]
    UnknownStateField,
    #[error("type mismatch: {0}")]
    TypeMismatch(String),
    #[error("unknown runtime value")]
    UnknownValue,
    #[error("missing machine frame")]
    MissingFrame,
    #[error("invalid instruction pointer")]
    InvalidIp,
    #[error("missing routine return target")]
    MissingReturnTarget,
    #[error("unsupported instruction")]
    UnsupportedInstruction,
    #[error("non-exhaustive runtime match")]
    NonExhaustiveMatch,
    #[error("unsupported pure expression")]
    UnsupportedPureExpression,
    #[error("effect found while evaluating a pure routine")]
    EffectInPureRoutine,
    #[error("propagated error: {0}")]
    PropagatedError(String),
    #[error("no pending effect")]
    NoPendingEffect,
    #[error("effect resolution does not match pending request")]
    ResolutionMismatch,
    #[error("unexpected effect usage dimension")]
    UnexpectedUsageDimension,
    #[error("effect usage was already reserved")]
    UsageAlreadyReserved,
    #[error("unknown tool")]
    UnknownTool,
    #[error("unknown prompt")]
    UnknownPrompt,
    #[error("unknown validator")]
    UnknownValidator,
    #[error("unknown policy")]
    UnknownPolicy,
    #[error("policy was not total")]
    NonTotalPolicy,
    #[error("runtime requirement failed")]
    RequirementFailed,
    #[error("write tool is missing idempotency metadata")]
    MissingIdempotency,
    #[error("approval suspension is not representable")]
    ApprovalNotRepresentable,
    #[error("authority value is opaque")]
    AuthorityValueOpaque,
    #[error("invalid normalized instant")]
    InvalidInstant,
    #[error("snapshot program mismatch")]
    ProgramMismatch,
    #[error("snapshot schema mismatch")]
    SnapshotSchemaMismatch,
    #[error("snapshot hash mismatch")]
    SnapshotHashMismatch,
    #[error("secret persistence rejected")]
    SecretPersistenceRejected,
    #[error("budget failure: {0}")]
    Budget(String),
    #[error("authority failure: {0}")]
    Authority(String),
    #[error("required capability grant is missing or out of scope")]
    MissingCapability,
    #[error("capability failure: {0}")]
    Capability(String),
    #[error("serialization failure: {0}")]
    Serialization(String),
}
