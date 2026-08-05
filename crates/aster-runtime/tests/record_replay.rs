use std::collections::BTreeMap;

use aster_ir::lower;
use aster_runtime::{
    CapabilityGrant, CapabilityGrants, EffectKind, FixtureDriver, FixtureEntry, FixtureSet,
    ReplayError, StartRequest, canonical_json, record_run, replay_run,
};
use aster_semantics::check_source;
use aster_syntax::SourceFile;
use serde_json::{Value, json};

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
            ("last_event".to_owned(), Value::Null),
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

fn entry(kind: EffectKind, identity: &str, match_request: Value, response: Value) -> FixtureEntry {
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

#[test]
fn meeting_record_and_driver_free_replay_have_identical_state() {
    // Catches replay implementations that merely trust recorded final output.
    let program = meeting_program();
    let start = start_request();
    let mut driver = fixture_driver();
    let recorded =
        record_run(program.clone(), start.clone(), &mut driver).expect("record succeeds");

    assert_eq!(driver.call_count(EffectKind::Model), 1);
    assert_eq!(driver.call_count(EffectKind::Read), 2);
    assert_eq!(driver.call_count(EffectKind::Approval), 1);
    assert_eq!(driver.call_count(EffectKind::Write), 1);
    recorded.trace.verify().expect("recorded trace verifies");
    assert_eq!(recorded.snapshots.len(), 5);

    let replayed = replay_run(program, start, &recorded.trace).expect("semantic replay succeeds");
    assert_eq!(
        canonical_json(&json!(recorded.outcome.state)).unwrap(),
        canonical_json(&json!(replayed.state)).unwrap()
    );
    assert_eq!(replayed.state["last_event"], json!({"id": "event-001"}));
}

#[test]
fn replay_rejects_changed_input_before_effects() {
    let program = meeting_program();
    let start = start_request();
    let mut driver = fixture_driver();
    let recorded = record_run(program.clone(), start.clone(), &mut driver).unwrap();
    let mut changed = start;
    changed.event_id = "evt-other".to_owned();

    assert!(matches!(
        replay_run(program, changed, &recorded.trace),
        Err(ReplayError::FingerprintMismatch)
    ));
}
