use std::collections::BTreeMap;

use aster_ir::lower;
use aster_runtime::{
    CapabilityGrant, CapabilityGrants, EffectKind, ExecutionGrant, Hello, HostEffectResolution,
    HostOutboundFrame, HostOutboundMessage, HostProtocolError, RecordProgress, RecordSession,
    StartRequest, decode_host_reply,
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
