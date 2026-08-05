use std::collections::BTreeMap;

use aster_ir::lower;
use aster_runtime::{
    CapabilityGrant, CapabilityGrants, EffectKind, EffectResolution, Machine, MachineError,
    MachineSnapshot, StartRequest, Step,
};
use aster_semantics::check_source;
use aster_syntax::SourceFile;
use serde_json::{Value as JsonValue, json};

fn checked_program(path: &str) -> aster_ir::Program {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(path);
    let text = std::fs::read_to_string(&path).expect("fixture is readable");
    let source = SourceFile::new(path.display().to_string(), text);
    lower(&check_source(&source).expect("fixture checks")).expect("fixture lowers")
}

fn inference_program() -> aster_ir::Program {
    let source = SourceFile::new(
        "inference.aster",
        r#"module inference;
type Message = { text: Untrusted<Text>, };
type Answer = { value: Text, };
capability ModelUse(alias: Text);
prompt Parse(message: Untrusted<Text>) -> Answer {
  instruction """Extract the answer.""";
  data { message, };
}
agent A() requires [ModelUse("planner")] {
  state {}
  budget per_event { model_calls <= 1; model_tokens <= 10; }
  on message(msg: Incoming<Message>) -> Result<Unit, Error> {
    let candidate = (infer Parse(message = msg.value.text) using @planner)?;
    return Ok(Unit);
  }
}
"#,
    );
    lower(&check_source(&source).expect("source checks")).expect("source lowers")
}

fn grants(entries: &[(&str, JsonValue)]) -> CapabilityGrants {
    CapabilityGrants {
        schema_version: 1,
        grants: entries
            .iter()
            .map(|(capability, argument)| CapabilityGrant {
                capability: (*capability).to_owned(),
                arguments: vec![argument.clone()],
            })
            .collect(),
    }
}

#[test]
fn direct_allow_write_path_never_yields_approval() {
    // Catches policy evaluation that turns every authorization into approval.
    let program = checked_program("tests/conformance/pass/direct_allow.aster");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "Writer".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::from([("owner".to_owned(), json!("user-001"))]),
            payload: json!("save"),
            state: BTreeMap::new(),
            capabilities: grants(&[("Read", json!("user-001")), ("Write", json!("user-001"))]),
        },
    )
    .expect("machine starts");
    let mut writes = 0;
    let mut reads = 0;
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(effect) => {
                let payload = match effect.kind {
                    EffectKind::Write => {
                        writes += 1;
                        json!({"id": "created-001"})
                    }
                    EffectKind::Read => {
                        reads += 1;
                        json!({"id": "created-001"})
                    }
                    EffectKind::Approval => panic!("direct allow must not request approval"),
                    EffectKind::Model => panic!("fixture has no model effect"),
                };
                machine
                    .supply(&EffectResolution {
                        request_hash: effect.request_hash,
                        payload,
                        actual_usage: BTreeMap::new(),
                    })
                    .expect("effect resolves");
            }
            Step::Completed(outcome) => {
                assert_eq!(outcome.state, BTreeMap::new());
                break;
            }
            Step::Failed(error) => panic!("machine failed: {error}"),
        }
    }
    assert_eq!((writes, reads), (1, 1));
}

#[test]
fn meeting_scheduler_runs_full_approval_and_reconciliation_path() {
    // Catches example-specialized shortcuts and missing governed stages.
    let program = checked_program("examples/meeting-scheduler/main.aster");
    let mut machine = Machine::start(
        program,
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
                ("last_event".to_owned(), JsonValue::Null),
            ]),
            capabilities: grants(&[
                ("ModelUse", json!("planner")),
                ("CalendarRead", json!("user-001")),
                ("CalendarWrite", json!("user-001")),
                ("HumanApproval", json!("user-001")),
            ]),
        },
    )
    .expect("machine starts");
    let mut kinds = Vec::new();
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(effect) => {
                kinds.push(effect.kind);
                let payload = match (effect.kind, effect.identity.as_str()) {
                    (EffectKind::Model, "ParseMeeting") => json!({
                        "title": "Planning",
                        "attendees": ["new.person@example.test"],
                        "duration_minutes": 30
                    }),
                    (EffectKind::Read, "Calendar.free") => json!([{"id": "slot-001"}]),
                    (EffectKind::Approval, "CalendarPolicy") => json!({"approved": true}),
                    (EffectKind::Write, "Calendar.create")
                    | (EffectKind::Read, "Calendar.lookup") => json!({"id": "event-001"}),
                    other => panic!("unexpected effect: {other:?}"),
                };
                machine
                    .supply(&EffectResolution {
                        request_hash: effect.request_hash,
                        payload,
                        actual_usage: BTreeMap::new(),
                    })
                    .expect("effect resolves");
            }
            Step::Completed(outcome) => {
                assert_eq!(outcome.state["last_event"], json!({"id": "event-001"}));
                break;
            }
            Step::Failed(error) => panic!("machine failed: {error}"),
        }
    }
    assert_eq!(
        kinds,
        vec![
            EffectKind::Model,
            EffectKind::Read,
            EffectKind::Approval,
            EffectKind::Write,
            EffectKind::Read,
        ]
    );
}

#[test]
fn machine_yields_resumes_and_round_trips_pending_snapshot() {
    // Catches recursive AST execution and snapshots that lose an effect boundary.
    let program = inference_program();
    let request = StartRequest {
        agent: "A".to_owned(),
        event: "message".to_owned(),
        event_id: "evt-001".to_owned(),
        event_time: "2026-08-05T12:00:00Z".to_owned(),
        agent_arguments: BTreeMap::new(),
        payload: json!({"text": "hello"}),
        state: BTreeMap::new(),
        capabilities: grants(&[("ModelUse", json!("planner"))]),
    };
    let mut machine = Machine::start(program.clone(), request).expect("machine starts");

    let effect = loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => break request,
            other => panic!("expected effect yield, got {other:?}"),
        }
    };
    assert_eq!(effect.kind, EffectKind::Model);
    assert_eq!(effect.identity, "Parse");

    let json = machine
        .snapshot()
        .expect("snapshot is safe")
        .to_json()
        .expect("snapshot serializes");
    let snapshot = MachineSnapshot::from_json(&json).expect("snapshot validates");
    let mut resumed = Machine::restore(program, snapshot).expect("snapshot restores");
    assert_eq!(resumed.step(), Step::Yield(effect.clone()));
    assert_eq!(
        resumed.supply(&EffectResolution {
            request_hash: "wrong".to_owned(),
            payload: json!({"value": "answer"}),
            actual_usage: BTreeMap::new(),
        }),
        Err(MachineError::ResolutionMismatch)
    );
    resumed
        .reserve_pending_usage(&BTreeMap::from([("model_tokens".to_owned(), 10)]))
        .expect("maximum usage reserves before resolution");
    resumed
        .supply(&EffectResolution {
            request_hash: effect.request_hash,
            payload: json!({"value": "answer"}),
            actual_usage: BTreeMap::from([("model_tokens".to_owned(), 3)]),
        })
        .expect("matching resolution resumes");

    loop {
        match resumed.step() {
            Step::Continue => {}
            Step::Completed(outcome) => {
                assert_eq!(outcome.state, BTreeMap::new());
                break;
            }
            other => panic!("expected completion, got {other:?}"),
        }
    }
}

#[test]
fn missing_exact_runtime_capability_is_rejected_at_admission() {
    let program = inference_program();
    let result = Machine::start(
        program,
        StartRequest {
            agent: "A".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::new(),
            payload: json!({"text": "hello"}),
            state: BTreeMap::new(),
            capabilities: grants(&[("ModelUse", json!("other-model"))]),
        },
    );
    let Err(error) = result else {
        panic!("out-of-scope grant must fail before any effect exists");
    };
    assert_eq!(error, MachineError::MissingCapability);
}

#[test]
fn undeclared_and_ill_typed_runtime_grants_are_rejected() {
    let base = StartRequest {
        agent: "A".to_owned(),
        event: "message".to_owned(),
        event_id: "evt-001".to_owned(),
        event_time: "2026-08-05T12:00:00Z".to_owned(),
        agent_arguments: BTreeMap::new(),
        payload: json!({"text": "hello"}),
        state: BTreeMap::new(),
        capabilities: grants(&[("Unknown", json!("planner"))]),
    };
    let result = Machine::start(inference_program(), base.clone());
    assert!(matches!(result, Err(MachineError::UnknownCapabilityGrant)));

    let mut ill_typed = base;
    ill_typed.capabilities = grants(&[("ModelUse", json!(7))]);
    let result = Machine::start(inference_program(), ill_typed);
    assert!(matches!(result, Err(MachineError::InvalidCapabilityGrant)));
}

#[test]
fn model_response_is_decoded_against_the_prompt_schema() {
    let program = inference_program();
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "A".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::new(),
            payload: json!({"text": "hello"}),
            state: BTreeMap::new(),
            capabilities: grants(&[("ModelUse", json!("planner"))]),
        },
    )
    .expect("machine starts");
    let request = loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => break request,
            other => panic!("expected model yield, got {other:?}"),
        }
    };
    assert_eq!(
        machine.supply(&EffectResolution {
            request_hash: request.request_hash,
            payload: json!({"value": 42}),
            actual_usage: BTreeMap::new(),
        }),
        Err(MachineError::TypeMismatch("Text".to_owned()))
    );
}

#[test]
fn tool_response_is_decoded_against_the_declared_result_schema() {
    let program = checked_program("tests/conformance/pass/direct_allow.aster");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "Writer".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::from([("owner".to_owned(), json!("user-001"))]),
            payload: json!("save"),
            state: BTreeMap::new(),
            capabilities: grants(&[("Read", json!("user-001")), ("Write", json!("user-001"))]),
        },
    )
    .expect("machine starts");
    let request = loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => break request,
            other => panic!("expected write yield, got {other:?}"),
        }
    };
    assert_eq!(request.kind, EffectKind::Write);
    assert_eq!(
        machine.supply(&EffectResolution {
            request_hash: request.request_hash,
            payload: json!("not-an-item"),
            actual_usage: BTreeMap::new(),
        }),
        Err(MachineError::TypeMismatch("Item".to_owned()))
    );
}

#[test]
fn failed_handler_keeps_state_update_unpublished() {
    let source = SourceFile::new(
        "atomic-state.aster",
        r"module atomic.example;
agent A() requires [] {
  state { count: Int = 0; }
  budget per_event {}
  on message(msg: Incoming<Text>) -> Result<Unit, Error> {
    update state { count = 1; }
    require false;
    return Ok(Unit);
  }
}
",
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "A".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::new(),
            payload: json!("go"),
            state: BTreeMap::from([("count".to_owned(), json!(0))]),
            capabilities: grants(&[]),
        },
    )
    .expect("machine starts");
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Failed(MachineError::RequirementFailed) => break,
            other => panic!("expected controlled requirement failure, got {other:?}"),
        }
    }
    let snapshot: JsonValue = serde_json::from_str(
        &machine
            .snapshot()
            .expect("failed continuation remains inspectable")
            .to_json()
            .expect("snapshot serializes"),
    )
    .expect("snapshot is JSON");
    assert_eq!(snapshot["current_state"]["count"]["value"], json!(0));
    assert_eq!(snapshot["pending_state"]["count"]["value"], json!(1));
}
