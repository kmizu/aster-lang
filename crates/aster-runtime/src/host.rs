use std::{collections::BTreeMap, fmt};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor},
};
use serde_json::{Value, value::RawValue};
use thiserror::Error;

use crate::{
    AdmittedEffect, EffectRequest, EffectResolution, MachineError, MachineSnapshot, RecordFailure,
    RecordProgress, RecordSession, RunError, RunOutcome, StartRequest, Trace, canonical_sha256,
};
use aster_ir::Program;

/// Stable host protocol name used during negotiation and grant binding.
pub const HOST_PROTOCOL_NAME: &str = "aster-host";
/// ASTER v0.2 host protocol schema and sequencing version.
pub const HOST_PROTOCOL_VERSION: u32 = 1;
/// Maximum UTF-8 bytes accepted for one JSON Lines protocol frame.
pub const HOST_PROTOCOL_MAX_LINE_BYTES: usize = 1_048_576;

const MODEL_TOKENS: &str = "model_tokens";
const MONEY_MICROUNITS: &str = "money_microunits";
const INVALID_USAGE_MARKER: &str = "invalid host usage declaration";

/// ASTER-to-host protocol envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostOutboundFrame {
    /// Envelope schema version.
    pub schema_version: u32,
    /// Deterministic run/session identity.
    pub session_id: String,
    /// Monotonic ASTER-issued message identifier.
    pub message_id: u64,
    /// Exactly one tagged protocol payload.
    #[serde(flatten)]
    pub message: HostOutboundMessage,
}

impl HostOutboundFrame {
    /// Wraps one outbound message in the current exact envelope.
    #[must_use]
    pub fn new(session_id: String, message_id: u64, message: HostOutboundMessage) -> Self {
        Self {
            schema_version: HOST_PROTOCOL_VERSION,
            session_id,
            message_id,
            message,
        }
    }
}

impl<'de> Deserialize<'de> for HostOutboundFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let envelope = OutboundEnvelope::deserialize(deserializer)?;
        let message = HostOutboundMessage::decode(&envelope.kind, &envelope.payload)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            schema_version: envelope.schema_version,
            session_id: envelope.session_id,
            message_id: envelope.message_id,
            message,
        })
    }
}

/// Host-to-ASTER reply envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostInboundFrame {
    /// Envelope schema version.
    pub schema_version: u32,
    /// Deterministic run/session identity being answered.
    pub session_id: String,
    /// Exact outstanding ASTER message identifier.
    pub in_reply_to: u64,
    /// Exactly one tagged reply payload.
    #[serde(flatten)]
    pub message: HostInboundMessage,
}

impl HostInboundFrame {
    /// Wraps one host reply in the current exact envelope.
    #[must_use]
    pub fn new(session_id: String, in_reply_to: u64, message: HostInboundMessage) -> Self {
        Self {
            schema_version: HOST_PROTOCOL_VERSION,
            session_id,
            in_reply_to,
            message,
        }
    }
}

impl<'de> Deserialize<'de> for HostInboundFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let envelope = InboundEnvelope::deserialize(deserializer)?;
        let message = HostInboundMessage::decode(&envelope.kind, &envelope.payload)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            schema_version: envelope.schema_version,
            session_id: envelope.session_id,
            in_reply_to: envelope.in_reply_to,
            message,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutboundEnvelope {
    schema_version: u32,
    session_id: String,
    message_id: u64,
    kind: String,
    payload: Box<RawValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InboundEnvelope {
    schema_version: u32,
    session_id: String,
    in_reply_to: u64,
    kind: String,
    payload: Box<RawValue>,
}

/// Tagged ASTER-to-host messages.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum HostOutboundMessage {
    /// Protocol negotiation and run binding.
    Hello(Hello),
    /// Effect request that has not yet been admitted for execution.
    EffectPreview(EffectPreview),
    /// Durable authorization to execute one exact admitted request.
    ExecuteGrant(ExecutionGrant),
    /// Successful terminal evidence.
    Completed(HostCompleted),
    /// Controlled redacted terminal failure.
    Failed(HostFailed),
}

impl HostOutboundMessage {
    fn decode(kind: &str, payload: &RawValue) -> Result<Self, String> {
        match kind {
            "hello" => decode_payload(payload).map(Self::Hello),
            "effect_preview" => decode_payload(payload).map(Self::EffectPreview),
            "execute_grant" => decode_payload(payload).map(Self::ExecuteGrant),
            "completed" => decode_payload(payload).map(Self::Completed),
            "failed" => decode_payload(payload).map(Self::Failed),
            _ => Err("unsupported host message kind".to_owned()),
        }
    }
}

/// Tagged host-to-ASTER replies.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum HostInboundMessage {
    /// Exact protocol acknowledgement.
    HelloAck(HelloAck),
    /// Variable maximums proposed for one previewed effect.
    EffectAdmission(EffectAdmission),
    /// Result and usage for one exact execution grant.
    EffectResolution(HostEffectResolution),
}

impl HostInboundMessage {
    fn decode(kind: &str, payload: &RawValue) -> Result<Self, String> {
        match kind {
            "hello_ack" => decode_payload(payload).map(Self::HelloAck),
            "effect_admission" => decode_payload(payload).map(Self::EffectAdmission),
            "effect_resolution" => decode_payload(payload).map(Self::EffectResolution),
            _ => Err("unsupported host reply kind".to_owned()),
        }
    }
}

fn decode_payload<T>(payload: &RawValue) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(payload.get()).map_err(|error| {
        if error.to_string().starts_with(INVALID_USAGE_MARKER) {
            INVALID_USAGE_MARKER.to_owned()
        } else {
            "malformed host payload".to_owned()
        }
    })
}

/// Initial protocol and run identity announcement.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    /// Stable protocol name.
    pub protocol: String,
    /// Supported protocol version.
    pub protocol_version: u32,
    /// ASTER runtime package version.
    pub runtime_version: String,
    /// Exact compiled program fingerprint.
    pub program_hash: String,
    /// Deterministic run identity, equal to the envelope session ID.
    pub run_id: String,
}

impl Hello {
    /// Builds an announcement for the current host protocol.
    #[must_use]
    pub fn new(runtime_version: String, program_hash: String, run_id: String) -> Self {
        Self {
            protocol: HOST_PROTOCOL_NAME.to_owned(),
            protocol_version: HOST_PROTOCOL_VERSION,
            runtime_version,
            program_hash,
            run_id,
        }
    }
}

/// Exact protocol acknowledgement required before any effect preview.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HelloAck {
    /// Stable protocol name.
    pub protocol: String,
    /// Supported protocol version.
    pub protocol_version: u32,
}

impl HelloAck {
    /// Builds an acknowledgement for the current host protocol.
    #[must_use]
    pub fn current() -> Self {
        Self {
            protocol: HOST_PROTOCOL_NAME.to_owned(),
            protocol_version: HOST_PROTOCOL_VERSION,
        }
    }

    fn is_current(&self) -> bool {
        self.protocol == HOST_PROTOCOL_NAME && self.protocol_version == HOST_PROTOCOL_VERSION
    }
}

/// Complete machine-produced effect request prior to admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPreview {
    /// Exact request; preview does not authorize execution.
    pub request: EffectRequest,
}

/// Host-declared variable maximums for one previewed request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectAdmission {
    /// Exact hash of the previewed request.
    pub request_hash: String,
    /// Variable usage to reserve before granting execution.
    #[serde(deserialize_with = "deserialize_usage")]
    pub max_usage: BTreeMap<String, u64>,
}

impl EffectAdmission {
    /// Validates that only host-declared variable dimensions are present.
    ///
    /// # Errors
    ///
    /// Rejects fixed or unknown dimensions.
    pub fn validate(&self) -> Result<(), HostProtocolError> {
        validate_usage_dimensions(&self.max_usage)
    }
}

/// Durable transport sequencing grant for one admitted effect.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionGrant {
    /// Complete immutable effect request.
    pub request: EffectRequest,
    /// Exact reserved variable maximums.
    #[serde(deserialize_with = "deserialize_usage")]
    pub max_usage: BTreeMap<String, u64>,
    /// Canonical hash of the sealed pending continuation.
    pub snapshot_hash: String,
    /// Canonical binding of run, request, checkpoint, snapshot, and usage.
    pub execution_grant_hash: String,
}

impl ExecutionGrant {
    /// Builds a grant from one already admitted effect.
    ///
    /// # Errors
    ///
    /// Rejects unsupported usage dimensions or canonical hash failure.
    pub fn for_admitted(
        run_id: &str,
        admitted: &AdmittedEffect,
    ) -> Result<Self, HostProtocolError> {
        validate_usage_dimensions(&admitted.maximums)?;
        let execution_grant_hash = execution_grant_hash(run_id, admitted)?;
        Ok(Self {
            request: admitted.request.clone(),
            max_usage: admitted.maximums.clone(),
            snapshot_hash: admitted.snapshot_hash.clone(),
            execution_grant_hash,
        })
    }

    /// Verifies every public grant field against the durable admission.
    ///
    /// # Errors
    ///
    /// Rejects request, usage, snapshot, or grant-hash substitution.
    pub fn validate(
        &self,
        run_id: &str,
        admitted: &AdmittedEffect,
    ) -> Result<(), HostProtocolError> {
        if self.request != admitted.request
            || self.max_usage != admitted.maximums
            || self.snapshot_hash != admitted.snapshot_hash
            || self.execution_grant_hash != execution_grant_hash(run_id, admitted)?
        {
            return Err(HostProtocolError::BindingMismatch);
        }
        Ok(())
    }
}

fn execution_grant_hash(
    run_id: &str,
    admitted: &AdmittedEffect,
) -> Result<String, HostProtocolError> {
    canonical_sha256(&serde_json::json!({
        "protocol_version": HOST_PROTOCOL_VERSION,
        "run_id": run_id,
        "request_hash": admitted.request.request_hash,
        "trace_position": admitted.trace_position,
        "trace_hash": admitted.trace_hash,
        "snapshot_hash": admitted.snapshot_hash,
        "max_usage": admitted.maximums,
    }))
    .map_err(|_| HostProtocolError::BindingMismatch)
}

/// Host result bound to one exact execution grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostEffectResolution {
    /// Exact admitted request hash.
    pub request_hash: String,
    /// Exact execution grant hash.
    pub execution_grant_hash: String,
    /// Typed result payload decoded later by the machine boundary.
    pub payload: Value,
    /// Actual usage for every declared maximum dimension.
    #[serde(deserialize_with = "deserialize_usage")]
    pub actual_usage: BTreeMap<String, u64>,
}

impl HostEffectResolution {
    /// Verifies grant bindings and exact bounded actual usage.
    ///
    /// # Errors
    ///
    /// Rejects request or grant substitution, unsupported/missing dimensions,
    /// and actual usage above a reserved maximum.
    pub fn validate_against(
        &self,
        run_id: &str,
        admitted: &AdmittedEffect,
        grant: &ExecutionGrant,
    ) -> Result<(), HostProtocolError> {
        grant.validate(run_id, admitted)?;
        if self.request_hash != admitted.request.request_hash
            || self.execution_grant_hash != grant.execution_grant_hash
        {
            return Err(HostProtocolError::BindingMismatch);
        }
        validate_usage_dimensions(&self.actual_usage)?;
        if self.actual_usage.keys().ne(admitted.maximums.keys())
            || self.actual_usage.iter().any(|(name, actual)| {
                admitted
                    .maximums
                    .get(name)
                    .is_none_or(|maximum| actual > maximum)
            })
        {
            return Err(HostProtocolError::InvalidUsage);
        }
        Ok(())
    }

    /// Drops transport-only grant evidence and returns the VM resolution.
    #[must_use]
    pub fn into_runtime(self) -> EffectResolution {
        EffectResolution {
            request_hash: self.request_hash,
            payload: self.payload,
            actual_usage: self.actual_usage,
        }
    }
}

fn validate_usage_dimensions(usage: &BTreeMap<String, u64>) -> Result<(), HostProtocolError> {
    if usage
        .keys()
        .any(|name| name != MODEL_TOKENS && name != MONEY_MICROUNITS)
    {
        return Err(HostProtocolError::InvalidUsage);
    }
    Ok(())
}

fn deserialize_usage<'de, D>(deserializer: D) -> Result<BTreeMap<String, u64>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UsageVisitor;

    impl<'de> Visitor<'de> for UsageVisitor {
        type Value = BTreeMap<String, u64>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a unique map of supported host usage dimensions")
        }

        fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut usage = BTreeMap::new();
            while let Some((name, value)) = access.next_entry::<String, u64>()? {
                if (name != MODEL_TOKENS && name != MONEY_MICROUNITS)
                    || usage.insert(name, value).is_some()
                {
                    return Err(serde::de::Error::custom(INVALID_USAGE_MARKER));
                }
            }
            Ok(usage)
        }
    }

    deserializer.deserialize_map(UsageVisitor)
}

/// Successful terminal state and trace fingerprints.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostCompleted {
    /// Canonical hash of the committed final state artifact.
    pub final_state_hash: String,
    /// Current trace chain head.
    pub trace_hash: String,
}

/// Stable payload-free controlled failure summary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostFailed {
    /// Stable public diagnostic code.
    pub code: String,
    /// Redacted non-payload summary.
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HostOutstanding {
    Hello(HostOutboundFrame),
    Admission {
        frame: HostOutboundFrame,
        request: EffectRequest,
    },
    Resolution {
        frame: HostOutboundFrame,
        admitted: Box<AdmittedEffect>,
        grant: ExecutionGrant,
    },
    Terminal(HostOutboundFrame),
}

impl HostOutstanding {
    fn frame(&self) -> &HostOutboundFrame {
        match self {
            Self::Hello(frame)
            | Self::Admission { frame, .. }
            | Self::Resolution { frame, .. }
            | Self::Terminal(frame) => frame,
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal(_))
    }
}

/// Pure host-protocol sequencer layered over one deterministic record session.
pub struct HostSession {
    record: Option<RecordSession>,
    failure: Option<RecordFailure>,
    fallback_trace: Trace,
    run_id: String,
    program_hash: String,
    next_message_id: u64,
    outstanding: HostOutstanding,
    outcome: Option<RunOutcome>,
}

impl HostSession {
    /// Starts a record session with a required protocol handshake.
    ///
    /// # Errors
    ///
    /// Rejects invalid runtime start data before a protocol session exists.
    pub fn start(program: Program, start: StartRequest) -> Result<Self, HostProtocolError> {
        let program_hash = program.program_hash.clone();
        let record = RecordSession::start(program, start).map_err(|error| map_run_error(&error))?;
        Ok(Self::from_record(record, program_hash))
    }

    /// Restores one admitted effect and requires a fresh handshake before the
    /// identical execution grant is re-emitted.
    ///
    /// # Errors
    ///
    /// Rejects invalid snapshot seals, program/trace/checkpoint mismatches, or
    /// a snapshot without one admitted pending request.
    pub fn restore(
        program: Program,
        snapshot: MachineSnapshot,
        trace: Trace,
    ) -> Result<Self, HostProtocolError> {
        let program_hash = program.program_hash.clone();
        let record = RecordSession::restore(program, snapshot, trace)
            .map_err(|error| map_restore_error(&error))?;
        Ok(Self::from_record(record, program_hash))
    }

    fn from_record(record: RecordSession, program_hash: String) -> Self {
        let run_id = record.trace().run_id.clone();
        let hello = HostOutboundFrame::new(
            run_id.clone(),
            0,
            HostOutboundMessage::Hello(Hello::new(
                env!("CARGO_PKG_VERSION").to_owned(),
                program_hash.clone(),
                run_id.clone(),
            )),
        );
        Self {
            record: Some(record),
            failure: None,
            fallback_trace: Trace::new(run_id.clone()),
            run_id,
            program_hash,
            next_message_id: 1,
            outstanding: HostOutstanding::Hello(hello),
            outcome: None,
        }
    }

    /// Returns the frame awaiting a host reply or the terminal frame.
    #[must_use]
    pub fn outbound(&self) -> Option<HostOutboundFrame> {
        Some(self.outstanding.frame().clone())
    }

    /// Applies one exact reply and advances to the next outbound frame.
    ///
    /// # Errors
    ///
    /// Rejects malformed envelopes, stale/duplicate/out-of-order replies,
    /// cross-session or request/grant substitution, invalid usage, and
    /// controlled runtime failures. Failure closes the trace and exposes a
    /// redacted terminal frame through [`Self::outbound`].
    pub fn reply(
        &mut self,
        value: HostInboundFrame,
    ) -> Result<HostOutboundFrame, HostProtocolError> {
        match self.reply_inner(value) {
            Ok(frame) => Ok(frame),
            Err(error) => {
                self.transition_failed(&error);
                Err(error)
            }
        }
    }

    fn reply_inner(
        &mut self,
        value: HostInboundFrame,
    ) -> Result<HostOutboundFrame, HostProtocolError> {
        if value.schema_version != HOST_PROTOCOL_VERSION {
            return Err(HostProtocolError::MalformedFrame);
        }
        let expected = self.outstanding.clone();
        let expected_frame = expected.frame();
        if value.session_id != self.run_id {
            return Err(HostProtocolError::BindingMismatch);
        }
        if value.in_reply_to != expected_frame.message_id {
            return Err(HostProtocolError::OutOfSequence);
        }
        match (expected, value.message) {
            (HostOutstanding::Hello(_), HostInboundMessage::HelloAck(ack)) => {
                if !ack.is_current() {
                    return Err(HostProtocolError::MalformedFrame);
                }
                self.advance_record()
            }
            (
                HostOutstanding::Admission { request, .. },
                HostInboundMessage::EffectAdmission(admission),
            ) => {
                admission.validate()?;
                if admission.request_hash != request.request_hash {
                    return Err(HostProtocolError::BindingMismatch);
                }
                let admitted = self
                    .active_record_mut()?
                    .admit(&request.request_hash, admission.max_usage)
                    .map_err(|error| map_run_error(&error))?;
                self.emit_grant(admitted)
            }
            (
                HostOutstanding::Resolution {
                    admitted, grant, ..
                },
                HostInboundMessage::EffectResolution(resolution),
            ) => {
                resolution.validate_against(&self.run_id, &admitted, &grant)?;
                let runtime_resolution = resolution.into_runtime();
                self.active_record_mut()?
                    .resolve(&runtime_resolution)
                    .map_err(|error| map_run_error(&error))?;
                self.advance_record()
            }
            _ => Err(HostProtocolError::OutOfSequence),
        }
    }

    fn active_record_mut(&mut self) -> Result<&mut RecordSession, HostProtocolError> {
        self.record.as_mut().ok_or(HostProtocolError::OutOfSequence)
    }

    fn advance_record(&mut self) -> Result<HostOutboundFrame, HostProtocolError> {
        let progress = self
            .active_record_mut()?
            .progress()
            .map_err(|error| map_run_error(&error))?;
        match progress {
            RecordProgress::AwaitingAdmission(request) => {
                let frame = self.next_frame(HostOutboundMessage::EffectPreview(EffectPreview {
                    request: request.clone(),
                }))?;
                self.outstanding = HostOutstanding::Admission {
                    frame: frame.clone(),
                    request,
                };
                Ok(frame)
            }
            RecordProgress::AwaitingResolution(admitted) => self.emit_grant(*admitted),
            RecordProgress::Completed(outcome) => {
                let final_state_hash = canonical_sha256(&outcome.state)
                    .map_err(|_| HostProtocolError::RuntimeFailure)?;
                let trace_hash = self
                    .trace()
                    .entries
                    .last()
                    .map_or_else(String::new, |entry| entry.entry_hash.clone());
                let frame = self.next_frame(HostOutboundMessage::Completed(HostCompleted {
                    final_state_hash,
                    trace_hash,
                }))?;
                self.outcome = Some(outcome);
                self.outstanding = HostOutstanding::Terminal(frame.clone());
                Ok(frame)
            }
        }
    }

    fn emit_grant(
        &mut self,
        admitted: AdmittedEffect,
    ) -> Result<HostOutboundFrame, HostProtocolError> {
        let grant = ExecutionGrant::for_admitted(&self.run_id, &admitted)?;
        let frame = self.next_frame(HostOutboundMessage::ExecuteGrant(grant.clone()))?;
        self.outstanding = HostOutstanding::Resolution {
            frame: frame.clone(),
            admitted: Box::new(admitted),
            grant,
        };
        Ok(frame)
    }

    fn next_frame(
        &mut self,
        message: HostOutboundMessage,
    ) -> Result<HostOutboundFrame, HostProtocolError> {
        let message_id = self.next_message_id;
        self.next_message_id = self
            .next_message_id
            .checked_add(1)
            .ok_or(HostProtocolError::RuntimeFailure)?;
        Ok(HostOutboundFrame::new(
            self.run_id.clone(),
            message_id,
            message,
        ))
    }

    fn transition_failed(&mut self, error: &HostProtocolError) {
        if self.outstanding.is_terminal() {
            return;
        }
        if let Some(record) = self.record.take() {
            self.failure = Some(record.fail(RunError::Data(error.to_string())));
        }
        let frame = HostOutboundFrame::new(
            self.run_id.clone(),
            self.next_message_id,
            HostOutboundMessage::Failed(HostFailed {
                code: error.diagnostic_code().to_owned(),
                summary: error.to_string(),
            }),
        );
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.outstanding = HostOutstanding::Terminal(frame);
    }

    /// Closes an active session after a transport-level protocol failure.
    pub fn fail_protocol(&mut self, error: &HostProtocolError) {
        self.transition_failed(error);
    }

    /// Signals transport EOF. EOF is valid only after a terminal frame.
    ///
    /// # Errors
    ///
    /// Returns `ASTER-HOST-11005` while a reply is required and closes the
    /// session with redacted failure evidence.
    pub fn end_of_input(&mut self) -> Result<(), HostProtocolError> {
        if self.outstanding.is_terminal() {
            return Ok(());
        }
        let error = HostProtocolError::UnexpectedEof;
        self.transition_failed(&error);
        Err(error)
    }

    /// Returns the active or controlled-failure trace.
    #[must_use]
    pub fn trace(&self) -> &Trace {
        if let Some(failure) = &self.failure {
            &failure.trace
        } else if let Some(record) = &self.record {
            record.trace()
        } else {
            &self.fallback_trace
        }
    }

    /// Returns snapshots sealed before every emitted execution grant.
    #[must_use]
    pub fn snapshots(&self) -> &[MachineSnapshot] {
        if let Some(failure) = &self.failure {
            &failure.snapshots
        } else if let Some(record) = &self.record {
            record.snapshots()
        } else {
            &[]
        }
    }

    /// Returns the committed outcome only after a completed terminal frame.
    #[must_use]
    pub fn outcome(&self) -> Option<&RunOutcome> {
        self.outcome.as_ref()
    }

    /// Returns the exact program fingerprint announced by the handshake.
    #[must_use]
    pub fn program_hash(&self) -> &str {
        &self.program_hash
    }
}

fn map_restore_error(error: &RunError) -> HostProtocolError {
    match error {
        RunError::Trace(_)
        | RunError::Machine(
            MachineError::ProgramMismatch
            | MachineError::SnapshotSchemaMismatch
            | MachineError::RuntimeVersionMismatch
            | MachineError::SnapshotHashMismatch,
        ) => HostProtocolError::BindingMismatch,
        _ => map_run_error(error),
    }
}

fn map_run_error(error: &RunError) -> HostProtocolError {
    match error {
        RunError::SessionPhase | RunError::Machine(MachineError::ResolutionMismatch) => {
            HostProtocolError::BindingMismatch
        }
        RunError::Machine(
            MachineError::UnexpectedUsageDimension
            | MachineError::UsageAlreadyReserved
            | MachineError::Budget(_),
        ) => HostProtocolError::InvalidUsage,
        RunError::Machine(MachineError::TypeMismatch(_)) => HostProtocolError::MalformedFrame,
        RunError::Machine(_) | RunError::Driver(_) | RunError::Trace(_) | RunError::Data(_) => {
            HostProtocolError::RuntimeFailure
        }
    }
}

/// Decodes and validates one complete host reply without retaining its input.
///
/// # Errors
///
/// Rejects malformed JSON, unknown fields/kinds, unsupported versions, and
/// illegal usage dimensions.
pub fn decode_host_reply(input: &str) -> Result<HostInboundFrame, HostProtocolError> {
    let frame: HostInboundFrame = serde_json::from_str(input).map_err(|error| {
        if error.to_string().starts_with(INVALID_USAGE_MARKER) {
            HostProtocolError::InvalidUsage
        } else {
            HostProtocolError::MalformedFrame
        }
    })?;
    if frame.schema_version != HOST_PROTOCOL_VERSION {
        return Err(HostProtocolError::MalformedFrame);
    }
    match &frame.message {
        HostInboundMessage::HelloAck(ack) if !ack.is_current() => {
            Err(HostProtocolError::MalformedFrame)
        }
        HostInboundMessage::EffectAdmission(admission) => admission.validate(),
        HostInboundMessage::EffectResolution(resolution) => {
            validate_usage_dimensions(&resolution.actual_usage)
        }
        HostInboundMessage::HelloAck(_) => Ok(()),
    }?;
    Ok(frame)
}

/// Safe host protocol failure classes.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HostProtocolError {
    /// ASTER-HOST-11001.
    #[error("malformed or unsupported host protocol frame")]
    MalformedFrame,
    /// ASTER-HOST-11002.
    #[error("host protocol reply is out of sequence")]
    OutOfSequence,
    /// ASTER-HOST-11003.
    #[error("host protocol binding mismatch")]
    BindingMismatch,
    /// ASTER-HOST-11004.
    #[error("invalid host usage declaration")]
    InvalidUsage,
    /// ASTER-HOST-11005.
    #[error("host protocol ended before a required reply")]
    UnexpectedEof,
    /// ASTER-HOST-11006.
    #[error("host protocol frame could not be written")]
    WriteFailure,
    /// Existing non-protocol typed runtime failure.
    #[error("typed runtime failure")]
    RuntimeFailure,
}

impl HostProtocolError {
    /// Returns the stable public diagnostic code for this failure class.
    #[must_use]
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::MalformedFrame => "ASTER-HOST-11001",
            Self::OutOfSequence => "ASTER-HOST-11002",
            Self::BindingMismatch => "ASTER-HOST-11003",
            Self::InvalidUsage => "ASTER-HOST-11004",
            Self::UnexpectedEof => "ASTER-HOST-11005",
            Self::WriteFailure => "ASTER-HOST-11006",
            Self::RuntimeFailure => "ASTER-RUNTIME-9001",
        }
    }
}
