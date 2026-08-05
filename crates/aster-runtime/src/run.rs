use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    DriverError, EffectDriver, EffectKind, EffectRequest, EffectResolution, Machine, MachineError,
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
    let mut machine = Machine::start(program, start)?;
    let mut snapshots = Vec::new();
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => {
                trace.append("effect_requested", to_value(&request)?)?;
                let preview = driver.preview(&request)?;
                machine.reserve_pending_usage(&preview.max_usage)?;
                trace.append("budget_reserved", to_value(&preview.max_usage)?)?;
                let snapshot = machine.snapshot()?;
                trace.append(
                    "snapshot_written",
                    json!({"snapshot_hash": canonical_sha256(&snapshot).map_err(|error| RunError::Data(error.to_string()))?}),
                )?;
                snapshots.push(snapshot);
                let resolution = driver.resolve(&request, &preview)?;
                trace.append("effect_resolved", to_value(&resolution)?)?;
                machine.supply(&resolution)?;
                trace.append("budget_settled", to_value(&resolution.actual_usage)?)?;
                append_effect_evidence(&mut trace, &request, &resolution)?;
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
    loop {
        match machine.step() {
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

fn append_effect_evidence(
    trace: &mut Trace,
    request: &EffectRequest,
    resolution: &EffectResolution,
) -> Result<(), RunError> {
    match request.kind {
        EffectKind::Approval => trace.append(
            "permit_issued",
            json!({"request_hash": request.request_hash, "approved": resolution.payload.get("approved")}),
        )?,
        EffectKind::Write => trace.append(
            "proposal_committed",
            json!({"request_hash": request.request_hash}),
        )?,
        EffectKind::Read if request.identity.contains("lookup") => trace.append(
            "reconciliation_observation",
            json!({"request_hash": request.request_hash}),
        )?,
        EffectKind::Model | EffectKind::Read => {}
    }
    Ok(())
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
    #[error("replay data failure: {0}")]
    Data(String),
}
