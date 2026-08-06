use std::collections::BTreeMap;

use aster_ir::lower;
use aster_runtime::{
    CapabilityGrant, CapabilityGrants, EffectAdmission, EffectDriver, EffectKind, ExecutionGrant,
    FixtureDriver, FixtureEntry, FixturePreview, FixtureSet, Hello, HelloAck, HostEffectResolution,
    HostInboundFrame, HostInboundMessage, HostOutboundFrame, HostOutboundMessage,
    HostProtocolError, HostSession, RecordProgress, RecordSession, StartRequest, canonical_json,
    decode_host_reply, replay_run,
};
use aster_semantics::check_source;
use aster_syntax::SourceFile;
use serde_json::json;

fn meeting_program() -> aster_ir::Program {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/meeting-scheduler/main.aster");
    let text = std::fs::read_to_string(&path).expect("example is readable");
    lower(
        &check_source(&SourceFile::new(path.display().to_string(), text)).expect("example checks"),
    )
    .expect("example lowers")
}

fn start_request() -> StartRequest {
    StartRequest {
        agent: "Scheduler".to_owned(),
        event: "message".to_owned(),
        event_id: "evt-001".to_owned(),
        event_time: "2026-08-05T12:00:00Z".to_owned(),
        agent_arguments: BTreeMap::from([("user".to_owned(), json!("user-001"))]),
        payload: json!({
            "text": "Schedule a 30 minute meeting with new.person@example.test"
        }),
        state: BTreeMap::from([
            ("profile".to_owned(), json!({"known_attendees": []})),
            ("last_event".to_owned(), serde_json::Value::Null),
        ]),
        capabilities: CapabilityGrants {
            schema_version: 1,
            grants: [
                ("ModelUse", json!("planner")),
                ("CalendarRead", json!("user-001")),
                ("CalendarWrite", json!("user-001")),
                ("HumanApproval", json!("user-001")),
            ]
            .into_iter()
            .map(|(capability, argument)| CapabilityGrant {
                capability: capability.to_owned(),
                arguments: vec![argument],
            })
            .collect(),
        },
    }
}

fn admitted_model() -> (String, aster_runtime::AdmittedEffect) {
    let mut session =
        RecordSession::start(meeting_program(), start_request()).expect("session starts");
    let request = match session.progress().expect("session progresses") {
        RecordProgress::AwaitingAdmission(request) => request,
        other => panic!("expected admission, got {other:?}"),
    };
    let admitted = session
        .admit(
            &request.request_hash,
            BTreeMap::from([("model_tokens".to_owned(), 100)]),
        )
        .expect("model effect is admitted");
    (session.trace().run_id.clone(), admitted)
}

fn entry(
    kind: EffectKind,
    identity: &str,
    match_request: serde_json::Value,
    response: serde_json::Value,
) -> FixtureEntry {
    FixtureEntry {
        kind,
        identity: identity.to_owned(),
        match_request,
        response,
        max_usage: BTreeMap::new(),
        actual_usage: BTreeMap::new(),
    }
}

fn fixture_driver() -> FixtureDriver {
    let mut model = entry(
        EffectKind::Model,
        "ParseMeeting",
        json!({"model_alias": "planner"}),
        json!({
            "title": "Planning",
            "attendees": ["new.person@example.test"],
            "duration_minutes": 30
        }),
    );
    model.max_usage.insert("model_tokens".to_owned(), 100);
    model.actual_usage.insert("model_tokens".to_owned(), 20);
    FixtureDriver::new(FixtureSet {
        schema_version: 1,
        entries: vec![
            model,
            entry(
                EffectKind::Read,
                "Calendar.free",
                json!({"arguments": {"owner": "user-001", "duration_minutes": 30}}),
                json!([{"id": "slot-001"}]),
            ),
            entry(
                EffectKind::Approval,
                "CalendarPolicy",
                json!({"principal": "user-001"}),
                json!({"approved": true}),
            ),
            entry(
                EffectKind::Write,
                "Calendar.create",
                json!({"arguments": {"request_id": "evt-001"}}),
                json!({"id": "event-001"}),
            ),
            entry(
                EffectKind::Read,
                "Calendar.lookup",
                json!({"arguments": {"event_id": "event-001"}}),
                json!({"id": "event-001"}),
            ),
        ],
    })
    .expect("fixtures are valid")
}

fn reply_to(frame: &HostOutboundFrame, message: HostInboundMessage) -> HostInboundFrame {
    HostInboundFrame::new(frame.session_id.clone(), frame.message_id, message)
}

fn acknowledge(frame: &HostOutboundFrame) -> HostInboundFrame {
    reply_to(frame, HostInboundMessage::HelloAck(HelloAck::current()))
}

fn admitted(
    frame: &HostOutboundFrame,
    request_hash: &str,
    maximums: BTreeMap<String, u64>,
) -> HostInboundFrame {
    reply_to(
        frame,
        HostInboundMessage::EffectAdmission(EffectAdmission {
            request_hash: request_hash.to_owned(),
            max_usage: maximums,
        }),
    )
}

#[test]
fn hello_has_the_exact_envelope() {
    let frame = HostOutboundFrame::new(
        "run-1".to_owned(),
        0,
        HostOutboundMessage::Hello(Hello::new(
            "0.2.0".to_owned(),
            "program".to_owned(),
            "run-1".to_owned(),
        )),
    );
    assert_eq!(
        serde_json::to_string(&frame).expect("serialize"),
        r#"{"schema_version":1,"session_id":"run-1","message_id":0,"kind":"hello","payload":{"protocol":"aster-host","protocol_version":1,"runtime_version":"0.2.0","program_hash":"program","run_id":"run-1"}}"#
    );
}

#[test]
fn nested_unknown_fields_are_rejected() {
    let input = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":0,"kind":"hello_ack","payload":{"protocol":"aster-host","protocol_version":1,"extra":true}}"#;
    assert!(matches!(
        decode_host_reply(input),
        Err(HostProtocolError::MalformedFrame)
    ));
}

#[test]
fn wrong_schema_and_message_kind_are_rejected() {
    let wrong_schema = r#"{"schema_version":2,"session_id":"run-1","in_reply_to":0,"kind":"hello_ack","payload":{"protocol":"aster-host","protocol_version":1}}"#;
    let wrong_kind = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":0,"kind":"execute_now","payload":{}}"#;
    assert!(matches!(
        decode_host_reply(wrong_schema),
        Err(HostProtocolError::MalformedFrame)
    ));
    assert!(matches!(
        decode_host_reply(wrong_kind),
        Err(HostProtocolError::MalformedFrame)
    ));
}

#[test]
fn illegal_usage_dimensions_are_rejected() {
    let input = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":1,"kind":"effect_admission","payload":{"request_hash":"request","max_usage":{"external_writes":1}}}"#;
    assert!(matches!(
        decode_host_reply(input),
        Err(HostProtocolError::InvalidUsage)
    ));
}

#[test]
fn duplicate_usage_dimensions_are_rejected() {
    let input = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":1,"kind":"effect_admission","payload":{"request_hash":"request","max_usage":{"model_tokens":10,"model_tokens":20}}}"#;
    assert!(matches!(
        decode_host_reply(input),
        Err(HostProtocolError::InvalidUsage)
    ));
}

#[test]
fn usage_error_marker_in_an_unknown_field_cannot_change_the_error_class() {
    let input = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":0,"kind":"hello_ack","payload":{"protocol":"aster-host","protocol_version":1,"invalid host usage declaration":true}}"#;
    assert!(matches!(
        decode_host_reply(input),
        Err(HostProtocolError::MalformedFrame)
    ));
}

#[test]
fn execution_grant_binds_the_complete_admitted_effect() {
    let (run_id, admitted) = admitted_model();
    let grant = ExecutionGrant::for_admitted(&run_id, &admitted).expect("grant hashes");
    grant
        .validate(&run_id, &admitted)
        .expect("unaltered grant validates");

    let mut changed_request = grant.clone();
    changed_request.request.identity = "SubstitutedPrompt".to_owned();
    assert!(matches!(
        changed_request.validate(&run_id, &admitted),
        Err(HostProtocolError::BindingMismatch)
    ));

    let mut changed_hash = grant;
    changed_hash.execution_grant_hash = "substituted-grant-hash".to_owned();
    assert!(matches!(
        changed_hash.validate(&run_id, &admitted),
        Err(HostProtocolError::BindingMismatch)
    ));
}

#[test]
fn actual_usage_must_match_and_fit_the_grant() {
    let (run_id, admitted) = admitted_model();
    let grant = ExecutionGrant::for_admitted(&run_id, &admitted).expect("grant hashes");
    let missing = HostEffectResolution {
        request_hash: admitted.request.request_hash.clone(),
        execution_grant_hash: grant.execution_grant_hash.clone(),
        payload: json!({}),
        actual_usage: BTreeMap::new(),
    };
    assert!(matches!(
        missing.validate_against(&run_id, &admitted, &grant),
        Err(HostProtocolError::InvalidUsage)
    ));

    let overflow = HostEffectResolution {
        actual_usage: BTreeMap::from([("model_tokens".to_owned(), 101)]),
        ..missing
    };
    assert!(matches!(
        overflow.validate_against(&run_id, &admitted, &grant),
        Err(HostProtocolError::InvalidUsage)
    ));

    let exact = HostEffectResolution {
        actual_usage: BTreeMap::from([("model_tokens".to_owned(), 100)]),
        ..overflow
    };
    exact
        .validate_against(&run_id, &admitted, &grant)
        .expect("bounded exact dimensions validate");
    assert_eq!(exact.into_runtime().actual_usage["model_tokens"], 100);
    assert_eq!(admitted.request.kind, EffectKind::Model);
}

#[test]
fn host_session_handshake_precedes_two_phase_effect_execution() {
    let mut host =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = host.outbound().expect("hello is outstanding");
    assert!(matches!(hello.message, HostOutboundMessage::Hello(_)));
    assert!(host.snapshots().is_empty());

    let preview = host
        .reply(acknowledge(&hello))
        .expect("acknowledgement advances");
    let request = match &preview.message {
        HostOutboundMessage::EffectPreview(preview) => preview.request.clone(),
        other => panic!("expected preview, got {other:?}"),
    };
    assert!(host.snapshots().is_empty());

    let grant = host
        .reply(admitted(
            &preview,
            &request.request_hash,
            BTreeMap::from([("model_tokens".to_owned(), 100)]),
        ))
        .expect("admission advances");
    assert!(matches!(
        grant.message,
        HostOutboundMessage::ExecuteGrant(_)
    ));
    assert_eq!(host.snapshots().len(), 1);
}

#[test]
fn host_session_rejects_cross_session_and_stale_replies() {
    let mut cross_session =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = cross_session.outbound().expect("hello");
    let mut wrong_session = acknowledge(&hello);
    wrong_session.session_id = "different-run".to_owned();
    assert!(matches!(
        cross_session.reply(wrong_session),
        Err(HostProtocolError::BindingMismatch)
    ));
    assert!(matches!(
        cross_session.outbound().expect("failed frame").message,
        HostOutboundMessage::Failed(_)
    ));
    assert_eq!(
        cross_session
            .trace()
            .entries
            .last()
            .map(|entry| entry.kind.as_str()),
        Some("run_failed")
    );

    let mut stale =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = stale.outbound().expect("hello");
    stale
        .reply(acknowledge(&hello))
        .expect("first acknowledgement advances");
    assert!(matches!(
        stale.reply(acknowledge(&hello)),
        Err(HostProtocolError::OutOfSequence)
    ));
}

#[test]
fn host_session_rejects_resolution_before_grant_and_hash_substitution() {
    let mut early =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = early.outbound().expect("hello");
    let preview = early.reply(acknowledge(&hello)).expect("preview");
    assert!(matches!(
        early.reply(reply_to(
            &preview,
            HostInboundMessage::EffectResolution(HostEffectResolution {
                request_hash: "request".to_owned(),
                execution_grant_hash: "grant".to_owned(),
                payload: json!({}),
                actual_usage: BTreeMap::new(),
            }),
        )),
        Err(HostProtocolError::OutOfSequence)
    ));

    let mut substituted =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = substituted.outbound().expect("hello");
    let preview = substituted.reply(acknowledge(&hello)).expect("preview");
    assert!(matches!(
        substituted.reply(admitted(&preview, "substituted-request", BTreeMap::new())),
        Err(HostProtocolError::BindingMismatch)
    ));
}

#[test]
fn host_session_rejects_usage_overflow_and_unexpected_eof() {
    let mut overflow =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = overflow.outbound().expect("hello");
    let preview = overflow.reply(acknowledge(&hello)).expect("preview");
    let request_hash = match &preview.message {
        HostOutboundMessage::EffectPreview(preview) => preview.request.request_hash.clone(),
        other => panic!("expected preview, got {other:?}"),
    };
    assert!(matches!(
        overflow.reply(admitted(
            &preview,
            &request_hash,
            BTreeMap::from([("model_tokens".to_owned(), u64::MAX)]),
        )),
        Err(HostProtocolError::InvalidUsage)
    ));

    let mut eof =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    assert!(matches!(
        eof.end_of_input(),
        Err(HostProtocolError::UnexpectedEof)
    ));
    assert!(matches!(
        eof.outbound().expect("failed frame").message,
        HostOutboundMessage::Failed(_)
    ));
}

#[test]
fn host_session_restore_reemits_the_same_grant_without_readmission() {
    let program = meeting_program();
    let mut record =
        RecordSession::start(program.clone(), start_request()).expect("record session starts");
    let request = match record.progress().expect("record progresses") {
        RecordProgress::AwaitingAdmission(request) => request,
        other => panic!("expected admission, got {other:?}"),
    };
    let admitted = record
        .admit(
            &request.request_hash,
            BTreeMap::from([("model_tokens".to_owned(), 100)]),
        )
        .expect("effect is admitted");
    let expected = ExecutionGrant::for_admitted(&record.trace().run_id, &admitted)
        .expect("original grant hashes");

    let mut restored =
        HostSession::restore(program, admitted.snapshot.clone(), record.trace().clone())
            .expect("host session restores");
    let hello = restored.outbound().expect("fresh hello");
    let grant = restored
        .reply(acknowledge(&hello))
        .expect("grant follows handshake");

    assert_eq!(grant.message, HostOutboundMessage::ExecuteGrant(expected));
    assert_eq!(restored.snapshots(), &[admitted.snapshot]);
}

#[test]
fn host_session_restore_reemits_uncertain_write_without_readmission() {
    let program = meeting_program();
    let mut host =
        HostSession::start(program.clone(), start_request()).expect("host session starts");
    let mut driver = fixture_driver();
    let mut preview: Option<FixturePreview> = None;
    let hello = host.outbound().expect("hello");
    let mut outbound = host.reply(acknowledge(&hello)).expect("first preview");

    let (original_grant, snapshot, trace) = loop {
        outbound = match &outbound.message {
            HostOutboundMessage::EffectPreview(effect) => {
                let next_preview = driver.preview(&effect.request).expect("fixture previews");
                let reply = admitted(
                    &outbound,
                    &effect.request.request_hash,
                    next_preview.max_usage.clone(),
                );
                preview = Some(next_preview);
                host.reply(reply).expect("grant follows admission")
            }
            HostOutboundMessage::ExecuteGrant(grant) if grant.request.kind == EffectKind::Write => {
                let snapshot = host
                    .snapshots()
                    .last()
                    .expect("write continuation is sealed")
                    .clone();
                break (grant.clone(), snapshot, host.trace().clone());
            }
            HostOutboundMessage::ExecuteGrant(grant) => {
                let next_preview = preview.take().expect("preview precedes grant");
                let resolution = driver
                    .resolve(&grant.request, &next_preview)
                    .expect("fixture resolves");
                host.reply(reply_to(
                    &outbound,
                    HostInboundMessage::EffectResolution(HostEffectResolution {
                        request_hash: resolution.request_hash,
                        execution_grant_hash: grant.execution_grant_hash.clone(),
                        payload: resolution.payload,
                        actual_usage: resolution.actual_usage,
                    }),
                ))
                .expect("resolution advances")
            }
            other => panic!("unexpected host message {other:?}"),
        };
    };

    let mut restored =
        HostSession::restore(program, snapshot, trace).expect("write continuation restores");
    let hello = restored.outbound().expect("fresh hello");
    let resumed = restored
        .reply(acknowledge(&hello))
        .expect("write grant follows handshake");
    assert_eq!(
        resumed.message,
        HostOutboundMessage::ExecuteGrant(original_grant)
    );
}

#[test]
fn host_session_rejects_grant_substitution_and_actual_usage_overflow() {
    let mut substituted =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = substituted.outbound().expect("hello");
    let preview = substituted.reply(acknowledge(&hello)).expect("preview");
    let request_hash = match &preview.message {
        HostOutboundMessage::EffectPreview(preview) => preview.request.request_hash.clone(),
        other => panic!("expected preview, got {other:?}"),
    };
    let grant_frame = substituted
        .reply(admitted(
            &preview,
            &request_hash,
            BTreeMap::from([("model_tokens".to_owned(), 100)]),
        ))
        .expect("grant");
    let grant = match &grant_frame.message {
        HostOutboundMessage::ExecuteGrant(grant) => grant.clone(),
        other => panic!("expected grant, got {other:?}"),
    };
    assert!(matches!(
        substituted.reply(reply_to(
            &grant_frame,
            HostInboundMessage::EffectResolution(HostEffectResolution {
                request_hash: grant.request.request_hash,
                execution_grant_hash: "substituted-grant".to_owned(),
                payload: json!({}),
                actual_usage: BTreeMap::from([("model_tokens".to_owned(), 20)]),
            }),
        )),
        Err(HostProtocolError::BindingMismatch)
    ));

    let mut overflow =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = overflow.outbound().expect("hello");
    let preview = overflow.reply(acknowledge(&hello)).expect("preview");
    let request_hash = match &preview.message {
        HostOutboundMessage::EffectPreview(preview) => preview.request.request_hash.clone(),
        other => panic!("expected preview, got {other:?}"),
    };
    let grant_frame = overflow
        .reply(admitted(
            &preview,
            &request_hash,
            BTreeMap::from([("model_tokens".to_owned(), 100)]),
        ))
        .expect("grant");
    let grant = match &grant_frame.message {
        HostOutboundMessage::ExecuteGrant(grant) => grant.clone(),
        other => panic!("expected grant, got {other:?}"),
    };
    assert!(matches!(
        overflow.reply(reply_to(
            &grant_frame,
            HostInboundMessage::EffectResolution(HostEffectResolution {
                request_hash: grant.request.request_hash,
                execution_grant_hash: grant.execution_grant_hash,
                payload: json!({}),
                actual_usage: BTreeMap::from([("model_tokens".to_owned(), 101)]),
            }),
        )),
        Err(HostProtocolError::InvalidUsage)
    ));
}

#[test]
fn host_session_drives_all_effects_then_replays_without_host_interaction() {
    let program = meeting_program();
    let start = start_request();
    let mut host = HostSession::start(program.clone(), start.clone()).expect("host session starts");
    let mut driver = fixture_driver();
    let mut preview: Option<FixturePreview> = None;
    let mut effect_kinds = Vec::new();

    let hello = host.outbound().expect("hello");
    let mut outbound = host
        .reply(acknowledge(&hello))
        .expect("preview follows hello");
    loop {
        outbound = match &outbound.message {
            HostOutboundMessage::EffectPreview(effect) => {
                effect_kinds.push(effect.request.kind);
                let next_preview = driver.preview(&effect.request).expect("fixture previews");
                let reply = admitted(
                    &outbound,
                    &effect.request.request_hash,
                    next_preview.max_usage.clone(),
                );
                preview = Some(next_preview);
                host.reply(reply).expect("grant follows admission")
            }
            HostOutboundMessage::ExecuteGrant(grant) => {
                let next_preview = preview.take().expect("preview precedes grant");
                let resolution = driver
                    .resolve(&grant.request, &next_preview)
                    .expect("fixture resolves");
                host.reply(reply_to(
                    &outbound,
                    HostInboundMessage::EffectResolution(HostEffectResolution {
                        request_hash: resolution.request_hash,
                        execution_grant_hash: grant.execution_grant_hash.clone(),
                        payload: resolution.payload,
                        actual_usage: resolution.actual_usage,
                    }),
                ))
                .expect("resolution advances")
            }
            HostOutboundMessage::Completed(_) => break,
            other => panic!("unexpected host message {other:?}"),
        };
    }

    assert_eq!(
        effect_kinds,
        vec![
            EffectKind::Model,
            EffectKind::Read,
            EffectKind::Approval,
            EffectKind::Write,
            EffectKind::Read,
        ]
    );
    assert_eq!(driver.call_count(EffectKind::Model), 1);
    assert_eq!(driver.call_count(EffectKind::Read), 2);
    assert_eq!(driver.call_count(EffectKind::Approval), 1);
    assert_eq!(driver.call_count(EffectKind::Write), 1);
    let recorded = host.outcome().expect("host completed").clone();
    let terminal = host.outbound();
    let replayed = replay_run(program, start, host.trace()).expect("driver-free replay succeeds");
    assert_eq!(
        canonical_json(&recorded.state).expect("record state canonicalizes"),
        canonical_json(&replayed.state).expect("replay state canonicalizes")
    );
    assert_eq!(host.outbound(), terminal);
}

#[test]
fn host_redaction_excludes_untrusted_frame_values_from_every_artifact() {
    const PRIVATE: &str = "PRIVATE_FRAME_VALUE";
    const SECRET: &str = "SECRET_FRAME_VALUE";
    let mut host =
        HostSession::start(meeting_program(), start_request()).expect("host session starts");
    let hello = host.outbound().expect("hello");
    let mut hostile = acknowledge(&hello);
    hostile.session_id = format!("{PRIVATE}-{SECRET}");
    let error = host.reply(hostile).expect_err("cross-session reply fails");
    let failed = serde_json::to_string(&host.outbound().expect("failed frame"))
        .expect("failed frame serializes");
    let trace = host
        .trace()
        .to_json_lines()
        .expect("failure trace serializes");
    let snapshots = host
        .snapshots()
        .iter()
        .map(|snapshot| snapshot.to_json().expect("snapshot serializes"))
        .collect::<String>();
    for sink in [error.to_string(), failed, trace, snapshots] {
        assert!(!sink.contains(PRIVATE));
        assert!(!sink.contains(SECRET));
    }
}
