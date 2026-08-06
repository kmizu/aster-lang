use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    AuditEvent, DriverError, EffectDriver, EffectRequest, EffectResolution, FixturePreview,
    Machine, MachineError, MachineSnapshot, RunOutcome, StartRequest, Step, Trace, TraceError,
    canonical_sha256,
};
use aster_ir::Program;

/// Successful record run and its durable evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordResult {
    /// Atomic final state and return value.
    pub outcome: RunOutcome,
    /// Hash-chained complete logical trace.
    pub trace: Trace,
    /// Effect-boundary continuations in request order.
    pub snapshots: Vec<MachineSnapshot>,
}

/// Failed record run together with every durable artifact produced beforehand.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct RecordFailure {
    /// Controlled runtime/driver/trace failure.
    pub error: RunError,
    /// Valid hash-chained trace ending in `run_failed` when representable.
    pub trace: Trace,
    /// Snapshots completed before the failure.
    pub snapshots: Vec<MachineSnapshot>,
}

/// One effect admitted after its variable usage has been reserved durably.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedEffect {
    /// Exact request produced by the deterministic machine.
    pub request: EffectRequest,
    /// Variable usage maximums reserved before external execution.
    pub maximums: BTreeMap<String, u64>,
    /// Sealed continuation captured before external execution.
    pub snapshot: MachineSnapshot,
    /// Canonical hash of the complete sealed snapshot artifact.
    pub snapshot_hash: String,
    /// Next trace position bound into the snapshot.
    pub trace_position: u64,
    /// Trace chain head bound into the snapshot.
    pub trace_hash: String,
}

/// Observable suspension or terminal state of a record session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordProgress {
    /// A request requires declared usage maximums before it can execute.
    AwaitingAdmission(EffectRequest),
    /// The admitted request may now be resolved exactly once.
    AwaitingResolution(Box<AdmittedEffect>),
    /// The machine committed its final state and return value.
    Completed(RunOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RecordPhase {
    Running,
    AwaitingAdmission(EffectRequest),
    AwaitingResolution(Box<AdmittedEffect>),
    Completed(RunOutcome),
}

/// Pure, transport-independent record-mode state machine.
pub struct RecordSession {
    machine: Machine,
    trace: Trace,
    snapshots: Vec<MachineSnapshot>,
    phase: RecordPhase,
}

impl RecordSession {
    /// Starts a session and writes deterministic run identity evidence.
    ///
    /// # Errors
    ///
    /// Rejects invalid start data, entry points, grants, state, or trace data.
    pub fn start(program: Program, start: StartRequest) -> Result<Self, RunError> {
        Self::start_evidenced(program, start).map_err(|failure| failure.error)
    }

    fn start_evidenced(program: Program, start: StartRequest) -> Result<Self, RecordFailure> {
        let input_hash = canonical_sha256(&start)
            .map_err(|error| initial_record_failure(RunError::Data(error.to_string())))?;
        let run_id = canonical_sha256(&json!({
            "program_hash": program.program_hash,
            "input_hash": input_hash,
        }))
        .map_err(|error| initial_record_failure(RunError::Data(error.to_string())))?;
        let mut trace = Trace::new(run_id);
        let initialized = (|| -> Result<Machine, RunError> {
            trace.append(
                "run_header",
                json!({
                    "program_hash": program.program_hash,
                    "input_hash": input_hash,
                    "agent": start.agent,
                    "event": start.event,
                }),
            )?;
            trace.append(
                "event_received",
                json!({"event_id": start.event_id, "event_time": start.event_time}),
            )?;
            trace.append(
                "fingerprints",
                json!({
                    "program": program.program_hash,
                    "event": canonical_sha256(&json!({
                        "agent": start.agent,
                        "event": start.event,
                        "event_id": start.event_id,
                        "event_time": start.event_time,
                        "agent_arguments": start.agent_arguments,
                        "payload": start.payload,
                    })).map_err(|error| RunError::Data(error.to_string()))?,
                    "state": canonical_sha256(&start.state)
                        .map_err(|error| RunError::Data(error.to_string()))?,
                    "capabilities": canonical_sha256(&start.capabilities)
                        .map_err(|error| RunError::Data(error.to_string()))?,
                }),
            )?;
            Machine::start(program, start).map_err(RunError::Machine)
        })();
        match initialized {
            Ok(machine) => Ok(Self {
                machine,
                trace,
                snapshots: Vec::new(),
                phase: RecordPhase::Running,
            }),
            Err(error) => Err(record_failure(error, trace, Vec::new())),
        }
    }

    /// Restores the admitted effect bound into a sealed snapshot and trace.
    ///
    /// # Errors
    ///
    /// Rejects invalid snapshots, trace checkpoints, missing pending effects,
    /// or malformed reserved-usage evidence.
    pub fn restore(
        program: Program,
        snapshot: MachineSnapshot,
        mut trace: Trace,
    ) -> Result<Self, RunError> {
        trace.rewind_to(snapshot.trace_position, &snapshot.trace_hash)?;
        let mut machine = Machine::restore(program, snapshot.clone())?;
        let Step::Yield(request) = machine.step() else {
            return Err(RunError::SessionPhase);
        };
        let budget_evidence = machine.pending_budget_evidence()?;
        let maximums = serde_json::from_value(
            budget_evidence
                .get("variable_maximums")
                .cloned()
                .ok_or(RunError::SessionPhase)?,
        )
        .map_err(|error| RunError::Data(error.to_string()))?;
        let snapshot_hash =
            canonical_sha256(&snapshot).map_err(|error| RunError::Data(error.to_string()))?;
        trace.append("snapshot_written", json!({"snapshot_hash": snapshot_hash}))?;
        let admitted = AdmittedEffect {
            request,
            maximums,
            snapshot: snapshot.clone(),
            snapshot_hash,
            trace_position: snapshot.trace_position,
            trace_hash: snapshot.trace_hash.clone(),
        };
        Ok(Self {
            machine,
            trace,
            snapshots: vec![snapshot],
            phase: RecordPhase::AwaitingResolution(Box::new(admitted)),
        })
    }

    /// Advances pure instructions until admission, resolution, or completion.
    ///
    /// # Errors
    ///
    /// Returns controlled machine, trace, or serialization failures.
    pub fn progress(&mut self) -> Result<RecordProgress, RunError> {
        match self.phase.clone() {
            RecordPhase::AwaitingAdmission(request) => {
                return Ok(RecordProgress::AwaitingAdmission(request));
            }
            RecordPhase::AwaitingResolution(admitted) => {
                return Ok(RecordProgress::AwaitingResolution(admitted));
            }
            RecordPhase::Completed(outcome) => return Ok(RecordProgress::Completed(outcome)),
            RecordPhase::Running => {}
        }
        loop {
            let step = self.machine.step();
            append_audit_events(&mut self.trace, self.machine.take_audit_events())?;
            match step {
                Step::Continue => {}
                Step::Yield(request) => {
                    self.trace.append("effect_requested", to_value(&request)?)?;
                    self.phase = RecordPhase::AwaitingAdmission(request.clone());
                    return Ok(RecordProgress::AwaitingAdmission(request));
                }
                Step::Completed(outcome) => {
                    self.trace
                        .append("state_committed", to_value(&outcome.state)?)?;
                    self.trace.append("run_completed", to_value(&outcome)?)?;
                    self.phase = RecordPhase::Completed(outcome.clone());
                    return Ok(RecordProgress::Completed(outcome));
                }
                Step::Failed(error) => return Err(RunError::Machine(error)),
            }
        }
    }

    /// Reserves usage and seals the pending continuation before execution.
    ///
    /// # Errors
    ///
    /// Rejects out-of-phase or mismatched admission, unavailable budget,
    /// snapshot failure, and trace failure.
    pub fn admit(
        &mut self,
        request_hash: &str,
        maximums: BTreeMap<String, u64>,
    ) -> Result<AdmittedEffect, RunError> {
        let request = match &self.phase {
            RecordPhase::AwaitingAdmission(request) if request.request_hash == request_hash => {
                request.clone()
            }
            _ => return Err(RunError::SessionPhase),
        };
        self.machine.reserve_pending_usage(&maximums)?;
        self.trace
            .append("budget_reserved", self.machine.pending_budget_evidence()?)?;
        let (trace_position, trace_hash) = self.trace.checkpoint()?;
        self.machine
            .set_trace_checkpoint(trace_position, trace_hash.clone());
        let snapshot = self.machine.snapshot()?;
        let snapshot_hash =
            canonical_sha256(&snapshot).map_err(|error| RunError::Data(error.to_string()))?;
        self.trace
            .append("snapshot_written", json!({"snapshot_hash": snapshot_hash}))?;
        self.snapshots.push(snapshot.clone());
        let admitted = AdmittedEffect {
            request,
            maximums,
            snapshot,
            snapshot_hash,
            trace_position,
            trace_hash,
        };
        self.phase = RecordPhase::AwaitingResolution(Box::new(admitted.clone()));
        Ok(admitted)
    }

    /// Settles one admitted effect resolution and resumes pure execution.
    ///
    /// # Errors
    ///
    /// Rejects resolution before admission, request mismatch, malformed result
    /// data, actual usage above the reservation, and trace failure.
    pub fn resolve(&mut self, resolution: &EffectResolution) -> Result<(), RunError> {
        let admitted = match &self.phase {
            RecordPhase::AwaitingResolution(admitted)
                if admitted.request.request_hash == resolution.request_hash =>
            {
                admitted.clone()
            }
            _ => return Err(RunError::SessionPhase),
        };
        self.trace
            .append("effect_resolved", to_value(resolution)?)?;
        self.machine.supply(resolution)?;
        append_audit_events(&mut self.trace, self.machine.take_audit_events())?;
        self.trace.append(
            "budget_settled",
            budget_settlement_evidence(
                &admitted.maximums,
                &resolution.actual_usage,
                &self.machine.budget_evidence()?,
            )
            .map_err(RunError::Data)?,
        )?;
        self.phase = RecordPhase::Running;
        Ok(())
    }

    /// Returns the complete in-memory hash-chained trace so far.
    #[must_use]
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    /// Returns sealed snapshots in effect-admission order.
    #[must_use]
    pub fn snapshots(&self) -> &[MachineSnapshot] {
        &self.snapshots
    }

    /// Converts a completed session into its durable success evidence.
    #[must_use]
    pub fn finish(self, outcome: RunOutcome) -> RecordResult {
        RecordResult {
            outcome,
            trace: self.trace,
            snapshots: self.snapshots,
        }
    }

    /// Appends a controlled terminal failure and retains partial evidence.
    #[must_use]
    pub fn fail(self, error: RunError) -> RecordFailure {
        record_failure(error, self.trace, self.snapshots)
    }
}

/// Drives a machine through admitted fixture effects and records every boundary.
///
/// # Errors
///
/// Rejects VM, fixture, budget, snapshot, or trace failures without publishing
/// a successful outcome.
pub fn record_run<D: EffectDriver>(
    program: Program,
    start: StartRequest,
    driver: &mut D,
) -> Result<RecordResult, RunError> {
    record_run_evidenced(program, start, driver).map_err(|failure| failure.error)
}

/// Records a run while retaining partial evidence on every controlled failure.
///
/// # Errors
///
/// Returns the failure plus a hash-chained failure trace and prior snapshots.
pub fn record_run_evidenced<D: EffectDriver>(
    program: Program,
    start: StartRequest,
    driver: &mut D,
) -> Result<RecordResult, RecordFailure> {
    let mut session = RecordSession::start_evidenced(program, start)?;
    let mut preview: Option<FixturePreview> = None;
    loop {
        let progress = match session.progress() {
            Ok(progress) => progress,
            Err(error) => return Err(session.fail(error)),
        };
        match progress {
            RecordProgress::AwaitingAdmission(request) => {
                let next_preview = match driver.preview(&request) {
                    Ok(preview) => preview,
                    Err(error) => return Err(session.fail(RunError::Driver(error))),
                };
                if let Err(error) =
                    session.admit(&request.request_hash, next_preview.max_usage.clone())
                {
                    return Err(session.fail(error));
                }
                preview = Some(next_preview);
            }
            RecordProgress::AwaitingResolution(admitted) => {
                let Some(next_preview) = preview.take() else {
                    return Err(session.fail(RunError::SessionPhase));
                };
                let resolution = match driver.resolve(&admitted.request, &next_preview) {
                    Ok(resolution) => resolution,
                    Err(error) => return Err(session.fail(RunError::Driver(error))),
                };
                if let Err(error) = session.resolve(&resolution) {
                    return Err(session.fail(error));
                }
            }
            RecordProgress::Completed(outcome) => return Ok(session.finish(outcome)),
        }
    }
}

fn initial_record_failure(error: RunError) -> RecordFailure {
    RecordFailure {
        error,
        trace: Trace::new("record-initialization-failed"),
        snapshots: Vec::new(),
    }
}

fn record_failure(
    mut error: RunError,
    mut trace: Trace,
    snapshots: Vec<MachineSnapshot>,
) -> RecordFailure {
    if let Err(trace_error) = trace.append("run_failed", json!({"error": error.to_string()})) {
        error = RunError::Trace(trace_error);
    }
    RecordFailure {
        error,
        trace,
        snapshots,
    }
}

/// Semantically re-executes a trace without accepting or constructing a driver.
///
/// # Errors
///
/// Rejects hash-chain, fingerprint, request, resolution, budget, and final
/// outcome divergence.
pub fn replay_run(
    program: Program,
    start: StartRequest,
    trace: &Trace,
) -> Result<RunOutcome, ReplayError> {
    trace.verify()?;
    verify_header(&program, &start, trace)?;
    let recorded_requests: Vec<_> = payloads(trace, "effect_requested").collect();
    let recorded_reservations: Vec<_> = payloads(trace, "budget_reserved").collect();
    let recorded_resolutions: Vec<_> = payloads(trace, "effect_resolved").collect();
    let recorded_settlements: Vec<_> = payloads(trace, "budget_settled").collect();
    if recorded_requests.len() != recorded_reservations.len()
        || recorded_requests.len() != recorded_resolutions.len()
        || recorded_requests.len() != recorded_settlements.len()
    {
        return Err(ReplayError::EffectSequenceMismatch);
    }
    let mut machine = Machine::start(program, start)?;
    let mut effect_index = 0_usize;
    let mut generated_audit = Vec::new();
    loop {
        let step = machine.step();
        generated_audit.extend(machine.take_audit_events());
        match step {
            Step::Continue => {}
            Step::Yield(request) => {
                let expected: EffectRequest = from_value(
                    recorded_requests
                        .get(effect_index)
                        .ok_or(ReplayError::EffectSequenceMismatch)?,
                )?;
                if request != expected {
                    return Err(ReplayError::RequestDivergence);
                }
                let recorded_reservation = recorded_reservations
                    .get(effect_index)
                    .ok_or(ReplayError::EffectSequenceMismatch)?;
                let maximums = from_value(
                    recorded_reservation
                        .get("variable_maximums")
                        .ok_or(ReplayError::EffectSequenceMismatch)?,
                )?;
                machine.reserve_pending_usage(&maximums)?;
                if machine.pending_budget_evidence()? != **recorded_reservation {
                    return Err(ReplayError::BudgetDivergence);
                }
                let resolution: EffectResolution = from_value(
                    recorded_resolutions
                        .get(effect_index)
                        .ok_or(ReplayError::EffectSequenceMismatch)?,
                )?;
                machine.supply(&resolution)?;
                generated_audit.extend(machine.take_audit_events());
                let settlement = budget_settlement_evidence(
                    &maximums,
                    &resolution.actual_usage,
                    &machine.budget_evidence()?,
                )
                .map_err(ReplayError::Data)?;
                if Some(&settlement) != recorded_settlements.get(effect_index).copied() {
                    return Err(ReplayError::BudgetDivergence);
                }
                effect_index = effect_index
                    .checked_add(1)
                    .ok_or(ReplayError::EffectSequenceMismatch)?;
            }
            Step::Completed(outcome) => {
                if effect_index != recorded_requests.len() {
                    return Err(ReplayError::EffectSequenceMismatch);
                }
                let recorded: RunOutcome = from_value(
                    payloads(trace, "run_completed")
                        .next()
                        .ok_or(ReplayError::MissingCompletion)?,
                )?;
                if outcome != recorded {
                    return Err(ReplayError::OutcomeDivergence);
                }
                let recorded_audit = trace
                    .entries
                    .iter()
                    .filter(|entry| is_audit_kind(&entry.kind))
                    .map(|entry| AuditEvent {
                        kind: entry.kind.clone(),
                        payload: entry.payload.clone(),
                    })
                    .collect::<Vec<_>>();
                if generated_audit != recorded_audit {
                    return Err(ReplayError::GovernanceDivergence);
                }
                return Ok(outcome);
            }
            Step::Failed(error) => return Err(ReplayError::Machine(error)),
        }
    }
}

fn verify_header(
    program: &Program,
    start: &StartRequest,
    trace: &Trace,
) -> Result<(), ReplayError> {
    let header = payloads(trace, "run_header")
        .next()
        .ok_or(ReplayError::MissingHeader)?;
    let input_hash =
        canonical_sha256(start).map_err(|error| ReplayError::Data(error.to_string()))?;
    if header.get("program_hash").and_then(Value::as_str) != Some(&program.program_hash)
        || header.get("input_hash").and_then(Value::as_str) != Some(&input_hash)
    {
        return Err(ReplayError::FingerprintMismatch);
    }
    Ok(())
}

fn payloads<'a>(trace: &'a Trace, kind: &'a str) -> impl Iterator<Item = &'a Value> {
    trace
        .entries
        .iter()
        .filter(move |entry| entry.kind == kind)
        .map(|entry| &entry.payload)
}

fn append_audit_events(trace: &mut Trace, events: Vec<AuditEvent>) -> Result<(), RunError> {
    for event in events {
        trace.append(event.kind, event.payload)?;
    }
    Ok(())
}

/// Builds canonical actual/released/after evidence after one effect settles.
///
/// # Errors
///
/// Rejects actual usage above the maximum supplied before driver invocation.
pub fn budget_settlement_evidence(
    maximums: &std::collections::BTreeMap<String, u64>,
    actual: &std::collections::BTreeMap<String, u64>,
    after: &Value,
) -> Result<Value, String> {
    let released = maximums
        .iter()
        .map(|(name, maximum)| {
            let actual = actual.get(name).copied().unwrap_or(0);
            maximum
                .checked_sub(actual)
                .map(|released| (name.clone(), released))
                .ok_or_else(|| format!("actual usage exceeds maximum for {name}"))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
    Ok(json!({
        "count_actual": 1,
        "count_released": 0,
        "variable_actual": actual,
        "variable_released": released,
        "after_settlement": after,
    }))
}

fn is_audit_kind(kind: &str) -> bool {
    matches!(
        kind,
        "policy_decision" | "permit_issued" | "proposal_committed" | "reconciliation_decision"
    )
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, RunError> {
    serde_json::to_value(value).map_err(|error| RunError::Data(error.to_string()))
}

fn from_value<T: for<'de> Deserialize<'de>>(value: &Value) -> Result<T, ReplayError> {
    serde_json::from_value(value.clone()).map_err(|error| ReplayError::Data(error.to_string()))
}

/// Record-mode failure.
#[derive(Debug, Error)]
pub enum RunError {
    #[error(transparent)]
    Machine(#[from] MachineError),
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error("record session phase mismatch")]
    SessionPhase,
    #[error("record data failure: {0}")]
    Data(String),
}

/// Driver-free semantic replay divergence.
#[derive(Debug, Error)]
pub enum ReplayError {
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error(transparent)]
    Machine(#[from] MachineError),
    #[error("missing run header")]
    MissingHeader,
    #[error("missing run completion")]
    MissingCompletion,
    #[error("program, input, state, event, or grant fingerprint mismatch")]
    FingerprintMismatch,
    #[error("recorded effect sequence mismatch")]
    EffectSequenceMismatch,
    #[error("replayed effect request diverged")]
    RequestDivergence,
    #[error("replayed outcome diverged")]
    OutcomeDivergence,
    #[error("replayed governance evidence diverged")]
    GovernanceDivergence,
    #[error("replayed budget evidence diverged")]
    BudgetDivergence,
    #[error("replay data failure: {0}")]
    Data(String),
}
