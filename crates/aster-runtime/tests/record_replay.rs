use std::collections::BTreeMap;

use aster_ir::lower;
use aster_runtime::{
    CapabilityGrant, CapabilityGrants, EffectDriver, EffectKind, EffectResolution, FixtureDriver,
    FixtureEntry, FixtureSet, RecordProgress, RecordSession, ReplayError, RunError, StartRequest,
    Trace, canonical_json, canonical_sha256, record_run, record_run_evidenced, replay_run,
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

fn direct_start() -> StartRequest {
    StartRequest {
        agent: "Writer".to_owned(),
        event: "message".to_owned(),
        event_id: "evt-001".to_owned(),
        event_time: "2026-08-05T12:00:00Z".to_owned(),
        agent_arguments: BTreeMap::from([("owner".to_owned(), json!("user-001"))]),
        payload: json!("save"),
        state: BTreeMap::new(),
        capabilities: CapabilityGrants {
            schema_version: 1,
            grants: vec![
                CapabilityGrant {
                    capability: "Read".to_owned(),
                    arguments: vec![json!("user-001")],
                },
                CapabilityGrant {
                    capability: "Write".to_owned(),
                    arguments: vec![json!("user-001")],
                },
            ],
        },
    }
}

fn direct_driver() -> FixtureDriver {
    FixtureDriver::new(FixtureSet {
        schema_version: 1,
        entries: vec![
            entry(
                EffectKind::Write,
                "Store.put",
                json!({"arguments": {"request_id": "evt-001"}}),
                json!({"id": "created-001"}),
            ),
            entry(
                EffectKind::Read,
                "Store.get",
                json!({"arguments": {"id": "created-001"}}),
                json!({"id": "created-001"}),
            ),
        ],
    })
    .expect("fixture is valid")
}

fn reseal(trace: &mut Trace) {
    let mut previous = String::new();
    for (index, entry) in trace.entries.iter_mut().enumerate() {
        entry.sequence = u64::try_from(index).expect("test trace is short");
        entry.previous_entry_hash.clone_from(&previous);
        entry.entry_hash.clear();
        entry.entry_hash = canonical_sha256(entry).expect("entry hashes");
        previous.clone_from(&entry.entry_hash);
    }
}

#[test]
fn record_session_requires_admission_before_resolution() {
    let mut session =
        RecordSession::start(meeting_program(), start_request()).expect("session starts");
    let request = match session.progress().expect("session progresses") {
        RecordProgress::AwaitingAdmission(request) => request,
        other => panic!("expected admission, got {other:?}"),
    };
    assert_eq!(request.kind, EffectKind::Model);
    assert!(matches!(
        session.resolve(&EffectResolution {
            request_hash: request.request_hash,
            payload: json!({}),
            actual_usage: BTreeMap::new(),
        }),
        Err(RunError::SessionPhase)
    ));
}

#[test]
fn record_session_restore_resumes_the_same_admitted_effect() {
    let program = meeting_program();
    let mut session =
        RecordSession::start(program.clone(), start_request()).expect("session starts");
    let request = match session.progress().expect("session progresses") {
        RecordProgress::AwaitingAdmission(request) => request,
        other => panic!("expected admission, got {other:?}"),
    };
    let driver = fixture_driver();
    let preview = driver.preview(&request).expect("fixture preview matches");
    let admitted = session
        .admit(&request.request_hash, preview.max_usage)
        .expect("effect is admitted");
    let original_trace = session.trace().clone();

    let mut restored = RecordSession::restore(program, admitted.snapshot.clone(), original_trace)
        .expect("admitted session restores");

    assert_eq!(
        restored.progress().expect("restored session progresses"),
        RecordProgress::AwaitingResolution(Box::new(admitted))
    );
    assert_eq!(restored.trace(), session.trace());
    assert_eq!(restored.snapshots(), session.snapshots());
}

#[test]
fn record_session_rejects_mismatched_resolution_before_tracing_it() {
    let mut session =
        RecordSession::start(meeting_program(), start_request()).expect("session starts");
    let request = match session.progress().expect("session progresses") {
        RecordProgress::AwaitingAdmission(request) => request,
        other => panic!("expected admission, got {other:?}"),
    };
    let driver = fixture_driver();
    let preview = driver.preview(&request).expect("fixture preview matches");
    session
        .admit(&request.request_hash, preview.max_usage)
        .expect("effect is admitted");

    assert!(matches!(
        session.resolve(&EffectResolution {
            request_hash: "substituted-request-hash".to_owned(),
            payload: json!({"content": "PRIVATE_RESOLUTION_VALUE"}),
            actual_usage: BTreeMap::new(),
        }),
        Err(RunError::SessionPhase)
    ));
    assert!(
        session
            .trace()
            .entries
            .iter()
            .all(|entry| entry.kind != "effect_resolved")
    );
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
    let effect_boundary_kinds = recorded
        .trace
        .entries
        .iter()
        .filter_map(|entry| {
            matches!(
                entry.kind.as_str(),
                "effect_requested"
                    | "budget_reserved"
                    | "snapshot_written"
                    | "effect_resolved"
                    | "budget_settled"
            )
            .then_some(entry.kind.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        effect_boundary_kinds,
        vec![
            "effect_requested",
            "budget_reserved",
            "snapshot_written",
            "effect_resolved",
            "budget_settled",
            "effect_requested",
            "budget_reserved",
            "snapshot_written",
            "effect_resolved",
            "budget_settled",
            "effect_requested",
            "budget_reserved",
            "snapshot_written",
            "effect_resolved",
            "budget_settled",
            "effect_requested",
            "budget_reserved",
            "snapshot_written",
            "effect_resolved",
            "budget_settled",
            "effect_requested",
            "budget_reserved",
            "snapshot_written",
            "effect_resolved",
            "budget_settled",
        ]
    );

    let replayed = replay_run(program, start, &recorded.trace).expect("semantic replay succeeds");
    assert_eq!(
        canonical_json(&json!(recorded.outcome.state)).unwrap(),
        canonical_json(&json!(replayed.state)).unwrap()
    );
    assert_eq!(
        replayed.state["last_event"],
        json!({"some": {"id": "event-001"}})
    );
}

#[test]
fn maliciously_rehashed_result_still_fails_semantic_replay() {
    let program = meeting_program();
    let start = start_request();
    let mut driver = fixture_driver();
    let mut trace = record_run(program.clone(), start.clone(), &mut driver)
        .expect("record succeeds")
        .trace;
    let model_result = trace
        .entries
        .iter_mut()
        .find(|entry| {
            entry.kind == "effect_resolved"
                && entry.payload["payload"]["title"] == json!("Planning")
        })
        .expect("model resolution exists");
    model_result.payload["payload"]["title"] = json!("Malicious title");
    reseal(&mut trace);
    trace.verify().expect("attacker recomputed a valid chain");
    assert!(matches!(
        replay_run(program, start, &trace),
        Err(ReplayError::RequestDivergence | ReplayError::OutcomeDivergence)
    ));
}

#[test]
fn replay_rejects_modified_program_and_reordered_effect_requests() {
    let program = meeting_program();
    let start = start_request();
    let mut driver = fixture_driver();
    let recorded =
        record_run(program.clone(), start.clone(), &mut driver).expect("record succeeds");
    let mut changed_program = program.clone();
    changed_program.program_hash = "changed-program".to_owned();
    assert!(matches!(
        replay_run(changed_program, start.clone(), &recorded.trace),
        Err(ReplayError::FingerprintMismatch)
    ));

    let mut reordered = recorded.trace;
    let request_indices: Vec<_> = reordered
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.kind == "effect_requested").then_some(index))
        .collect();
    let first = request_indices[0];
    let second = request_indices[1];
    let first_payload = reordered.entries[first].payload.clone();
    reordered.entries[first].payload = reordered.entries[second].payload.clone();
    reordered.entries[second].payload = first_payload;
    reseal(&mut reordered);
    assert!(matches!(
        replay_run(program, start, &reordered),
        Err(ReplayError::RequestDivergence)
    ));
}

#[test]
fn exhausted_model_budget_rejects_before_driver_invocation() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("examples/meeting-scheduler/main.aster");
    let text = std::fs::read_to_string(&path)
        .expect("example is readable")
        .replacen("model_calls <= 2", "model_calls <= 0", 1);
    let program =
        lower(&check_source(&SourceFile::new("zero-budget.aster", text)).expect("source checks"))
            .expect("source lowers");
    let mut driver = fixture_driver();
    assert!(matches!(
        record_run(program, start_request(), &mut driver),
        Err(RunError::Machine(_))
    ));
    assert_eq!(driver.call_count(EffectKind::Model), 0);
}

#[test]
fn exhausted_write_budget_rejects_before_driver_invocation() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/conformance/pass/direct_allow.aster");
    let text = std::fs::read_to_string(&path)
        .expect("fixture is readable")
        .replacen("external_writes <= 1", "external_writes <= 0", 1);
    let program = lower(
        &check_source(&SourceFile::new("zero-write-budget.aster", text)).expect("source checks"),
    )
    .expect("source lowers");
    let mut driver = direct_driver();
    assert!(matches!(
        record_run(program, direct_start(), &mut driver),
        Err(RunError::Machine(_))
    ));
    assert_eq!(driver.call_count(EffectKind::Write), 0);
}

#[test]
fn expired_intent_rejects_before_write_driver_invocation() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/conformance/pass/direct_allow.aster");
    let text = std::fs::read_to_string(&path)
        .expect("fixture is readable")
        .replacen(
            "expires_at = event.time",
            "expires_at = add_seconds(event.time, -1)",
            1,
        );
    let program =
        lower(&check_source(&SourceFile::new("expired.aster", text)).expect("source checks"))
            .expect("source lowers");
    let mut driver = direct_driver();
    assert!(matches!(
        record_run(program, direct_start(), &mut driver),
        Err(RunError::Machine(_))
    ));
    assert_eq!(driver.call_count(EffectKind::Write), 0);
}

#[test]
fn direct_allow_record_run_never_invokes_approval_driver() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/conformance/pass/direct_allow.aster");
    let text = std::fs::read_to_string(&path)
        .expect("fixture is readable")
        .replacen(
            "external_writes <= 1;",
            "external_writes <= 1; money_microunits <= 1;",
            1,
        );
    let program = lower(
        &check_source(&SourceFile::new(path.display().to_string(), text)).expect("source checks"),
    )
    .expect("source lowers");
    let mut driver = direct_driver();
    record_run(program, direct_start(), &mut driver).expect("direct run succeeds");
    assert_eq!(driver.call_count(EffectKind::Approval), 0);
    assert_eq!(driver.call_count(EffectKind::Write), 1);
}

#[test]
fn usage_overflow_failure_is_hash_chained_into_partial_trace() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/conformance/pass/direct_allow.aster");
    let text = std::fs::read_to_string(&path)
        .expect("fixture is readable")
        .replacen(
            "external_writes <= 1;",
            "external_writes <= 1; money_microunits <= 1;",
            1,
        );
    let program = lower(
        &check_source(&SourceFile::new(path.display().to_string(), text)).expect("source checks"),
    )
    .expect("source lowers");
    let mut overflowing = entry(
        EffectKind::Write,
        "Store.put",
        json!({"arguments": {"request_id": "evt-001"}}),
        json!({"id": "created-001"}),
    );
    overflowing
        .max_usage
        .insert("money_microunits".to_owned(), 1);
    overflowing
        .actual_usage
        .insert("money_microunits".to_owned(), 2);
    let mut driver = FixtureDriver::new(FixtureSet {
        schema_version: 1,
        entries: vec![overflowing],
    })
    .expect("fixture is valid before resolution");
    let failure = record_run_evidenced(program, direct_start(), &mut driver)
        .expect_err("actual usage above maximum fails");
    failure.trace.verify().expect("partial trace remains valid");
    assert_eq!(
        failure
            .trace
            .entries
            .last()
            .map(|entry| entry.kind.as_str()),
        Some("run_failed")
    );
    assert_eq!(driver.call_count(EffectKind::Write), 1);
}

#[test]
fn replay_rejects_changed_input_before_effects() {
    let program = meeting_program();
    let start = start_request();
    let mut driver = fixture_driver();
    let recorded = record_run(program.clone(), start.clone(), &mut driver).unwrap();
    let mut changed = start.clone();
    changed.event_id = "evt-other".to_owned();

    assert!(matches!(
        replay_run(program.clone(), changed, &recorded.trace),
        Err(ReplayError::FingerprintMismatch)
    ));

    let mut changed_state = start;
    changed_state.state.insert(
        "profile".to_owned(),
        json!({"known_attendees": ["different@example.test"]}),
    );
    assert!(matches!(
        replay_run(program, changed_state, &recorded.trace),
        Err(ReplayError::FingerprintMismatch)
    ));
}
