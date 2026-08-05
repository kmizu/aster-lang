use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    AuditEvent, DriverError, EffectDriver, EffectRequest, EffectResolution, Machine, MachineError,
    MachineSnapshot, RunOutcome, StartRequest, Step, Trace, TraceError, canonical_sha256,
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
    let input_hash = canonical_sha256(&start).map_err(|error| RunError::Data(error.to_string()))?;
    let run_id = canonical_sha256(&json!({
        "program_hash": program.program_hash,
        "input_hash": input_hash,
    }))
    .map_err(|error| RunError::Data(error.to_string()))?;
    let mut trace = Trace::new(run_id);
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
    let mut machine = Machine::start(program, start)?;
    let mut snapshots = Vec::new();
    loop {
        let step = machine.step();
        append_audit_events(&mut trace, machine.take_audit_events())?;
        match step {
            Step::Continue => {}
            Step::Yield(request) => {
                trace.append("effect_requested", to_value(&request)?)?;
                let preview = driver.preview(&request)?;
                machine.reserve_pending_usage(&preview.max_usage)?;
                trace.append("budget_reserved", to_value(&preview.max_usage)?)?;
                let (position, hash) = trace.checkpoint()?;
                machine.set_trace_checkpoint(position, hash);
                let snapshot = machine.snapshot()?;
                trace.append(
                    "snapshot_written",
                    json!({"snapshot_hash": canonical_sha256(&snapshot).map_err(|error| RunError::Data(error.to_string()))?}),
                )?;
                snapshots.push(snapshot);
                let resolution = driver.resolve(&request, &preview)?;
                trace.append("effect_resolved", to_value(&resolution)?)?;
                machine.supply(&resolution)?;
                append_audit_events(&mut trace, machine.take_audit_events())?;
                trace.append("budget_settled", to_value(&resolution.actual_usage)?)?;
            }
            Step::Completed(outcome) => {
                trace.append("state_committed", to_value(&outcome.state)?)?;
                trace.append("run_completed", to_value(&outcome)?)?;
                return Ok(RecordResult {
                    outcome,
                    trace,
                    snapshots,
                });
            }
            Step::Failed(error) => {
                trace.append("run_failed", json!({"error": error.to_string()}))?;
                return Err(RunError::Machine(error));
            }
        }
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
    if recorded_requests.len() != recorded_reservations.len()
        || recorded_requests.len() != recorded_resolutions.len()
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
                let maximums = from_value(
                    recorded_reservations
                        .get(effect_index)
                        .ok_or(ReplayError::EffectSequenceMismatch)?,
                )?;
                machine.reserve_pending_usage(&maximums)?;
                let resolution: EffectResolution = from_value(
                    recorded_resolutions
                        .get(effect_index)
                        .ok_or(ReplayError::EffectSequenceMismatch)?,
                )?;
                machine.supply(&resolution)?;
                generated_audit.extend(machine.take_audit_events());
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
    #[error("replay data failure: {0}")]
    Data(String),
}
