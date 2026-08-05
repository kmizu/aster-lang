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
fn serializable_non_text_idempotency_reaches_the_write_boundary() {
    let source = SourceFile::new(
        "integer-idempotency.aster",
        r#"module integer_idempotency;
capability Write(owner: Text);
tool Store.put(owner: Text, request_id: Int) -> Unit {
  mode write;
  capability Write(owner);
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}
policy Direct(proposal: Proposal<Store.put>) {
  allow when true;
  deny "denied" otherwise;
}
agent Writer(owner: Text) requires [Write(owner)] {
  state {}
  budget per_event { external_writes <= 1; }
  on message(msg: Incoming<Int>) -> Result<Unit, Error> {
    let purpose = intent Save { actor = self; beneficiary = self; basis = [provenance(msg)]; expected = "saved"; expires_at = event.time; };
    let proposal = propose Store.put(owner, msg.value) for purpose;
    let permit = (authorize proposal using Direct)?;
    let receipt = (commit proposal with permit)?;
    return Ok(Unit);
  }
}
"#,
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "Writer".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::from([("owner".to_owned(), json!("user-001"))]),
            payload: json!(42),
            state: BTreeMap::new(),
            capabilities: grants(&[("Write", json!("user-001"))]),
        },
    )
    .expect("machine starts");

    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(effect) => {
                assert_eq!(effect.kind, EffectKind::Write);
                assert_eq!(effect.payload["arguments"]["request_id"], json!(42));
                break;
            }
            Step::Completed(_) => panic!("write effect was skipped"),
            Step::Failed(error) => panic!("machine failed before write: {error}"),
        }
    }
}

#[test]
fn policy_helpers_execute_typed_match_control_flow() {
    let source = SourceFile::new(
        "policy-match.aster",
        r#"module policy_match;
enum Decision { Yes, No }
fn permitted(value: Decision) -> Bool {
  require true;
  return match value { Decision.Yes => true, Decision.No => false };
}
capability Write(owner: Text, scope: Text);
tool Store.put(owner: Text, decision: Decision, request_id: Text) -> Unit {
  mode write;
  capability Write(scope = "store", owner = owner);
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}
policy Direct(proposal: Proposal<Store.put>) {
  allow when permitted(proposal.args.decision) && (match proposal.args.decision { Decision.Yes => true, Decision.No => false });
  deny "denied" otherwise;
}
agent Writer(owner: Text) requires [Write(scope = "store", owner = owner)] {
  state {}
  budget per_event { external_writes <= 1; }
  on message(msg: Incoming<Text>) -> Result<Unit, Error> {
    let purpose = intent Save { actor = self; beneficiary = self; basis = [provenance(msg)]; expected = "saved"; expires_at = event.time; };
    let proposal = propose Store.put(owner = owner, decision = Decision.Yes, request_id = event.id) for purpose;
    let permit = (authorize proposal using Direct)?;
    let receipt = (commit proposal with permit)?;
    return Ok(Unit);
  }
}
"#,
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
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
            capabilities: CapabilityGrants {
                schema_version: 1,
                grants: vec![CapabilityGrant {
                    capability: "Write".to_owned(),
                    arguments: vec![json!("user-001"), json!("store")],
                }],
            },
        },
    )
    .expect("machine starts");

    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(effect) => {
                assert_eq!(effect.kind, EffectKind::Write);
                break;
            }
            Step::Completed(_) => panic!("write effect was skipped"),
            Step::Failed(error) => panic!("policy helper failed: {error}"),
        }
    }
}

#[test]
fn stateless_authorization_executes_inside_a_flow_frame() {
    let source = SourceFile::new(
        "flow-authorization.aster",
        r#"module flow_authorization;
capability Write(scope: Text);
tool Store.put(request_id: Text) -> Unit {
  mode write;
  capability Write("store");
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}
policy Direct(proposal: Proposal<Store.put>) { allow when true; deny "no" otherwise; }
flow save(id: Text, purpose: Intent<Save>) -> Result<Unit, Error> uses [Write("store")] {
  let proposal = propose Store.put(id) for purpose;
  let permit = (authorize proposal using Direct)?;
  let receipt = (commit proposal with permit)?;
  return Ok(Unit);
}
agent Worker() requires [Write("store")] {
  state {}
  budget per_event { external_writes <= 1; }
  on message(msg: Incoming<Unit>) -> Result<Unit, Error> {
    let purpose = intent Save { actor = self; beneficiary = self; basis = [provenance(msg)]; expected = "saved"; expires_at = event.time; };
    return save(event.id, purpose);
  }
}
"#,
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "Worker".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::new(),
            payload: JsonValue::Null,
            state: BTreeMap::new(),
            capabilities: grants(&[("Write", json!("store"))]),
        },
    )
    .expect("machine starts");

    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(effect) => {
                assert_eq!(effect.kind, EffectKind::Write);
                break;
            }
            other => panic!("expected flow write, got {other:?}"),
        }
    }
}

#[test]
fn accepted_pure_constructs_execute_through_calls_and_state_update() {
    let source = SourceFile::new(
        "pure-constructs.aster",
        r"module pure_constructs;
type Summary = { score: Int, valid: Bool, later: Instant };
enum Choice { Add(Int), Skip }
fn recover(value: Int) -> Int {
  let result: Result<Int, Int> = Err(value);
  return match result { Ok(found) => found, Err(error) => error + 1 };
}
fn scoped(outer: Choice, inner: Choice) -> Int {
  return match outer { Choice.Add(value) => (match inner { Choice.Add(value) => value, Choice.Skip => 0 }) + value, Choice.Skip => 0 };
}
fn block_shadow(value: Int) -> Int {
  let branch = if true { let value = 2; value; } else { 0; };
  return branch + value;
}
fn difference(left: Int, right: Int) -> Int { return left - right; }
fn compute(values: List<Int>, choice: Choice, time: Instant) -> Result<Summary, Error> {
  let head = (first(values))?;
  let maybe = Some(head);
  let from_option = match maybe { Some(value) => value, None => 0 };
  let from_result = match Ok(from_option) { Ok(value) => value, Err(error) => 0 };
  let adjustment = match choice { Choice.Add(value) => value, Choice.Skip => 0 };
  let sign = if true { 1; } else { -1; };
  require contains(values, head);
  require subset([head], values);
  return Ok(Summary { score = (from_result + adjustment) * sign + recover(4) + difference(right = 3, left = 10) + scoped(Choice.Add(10), Choice.Add(2)) + block_shadow(10), valid = !false && (head == from_result), later = add_seconds(time, 60) });
}
agent Worker() requires [] {
  state { score: Int = 0; valid: Bool = false; later: Instant = event.time; seed: Int = if true { let value = 1; value + 1; } else { 0; }; }
  budget per_event {}
  on message(msg: Incoming<Int>) -> Result<Unit, Error> {
    let summary = (compute([msg.value, 2], Choice.Add(3), event.time))?;
    update state { score = summary.score; valid = summary.valid; later = summary.later; }
    return Ok(Unit);
  }
}
",
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
    let mut machine = Machine::start(
        program,
        StartRequest {
            agent: "Worker".to_owned(),
            event: "message".to_owned(),
            event_id: "evt-001".to_owned(),
            event_time: "2026-08-05T12:00:00Z".to_owned(),
            agent_arguments: BTreeMap::new(),
            payload: json!(42),
            state: BTreeMap::new(),
            capabilities: grants(&[]),
        },
    )
    .expect("machine starts");

    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Completed(outcome) => {
                assert_eq!(outcome.state["score"], json!(81));
                assert_eq!(outcome.state["valid"], json!(true));
                assert_eq!(outcome.state["later"], json!("2026-08-05T12:01:00Z"));
                assert_eq!(outcome.state["seed"], json!(2));
                break;
            }
            Step::Yield(effect) => panic!("pure program yielded {effect:?}"),
            Step::Failed(error) => panic!("pure program failed: {error}"),
        }
    }
}

#[test]
fn integer_division_and_overflow_fail_without_panicking() {
    let source = SourceFile::new(
        "arithmetic-errors.aster",
        r"module arithmetic_errors;
agent Worker() requires [] {
  state {}
  budget per_event {}
  on message(msg: Incoming<Int>) -> Result<Unit, Error> {
    let broken = if msg.value == 0 { 1 / 0; } else { msg.value + 1; };
    return Ok(Unit);
  }
}
",
    );
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");

    for payload in [json!(0), json!(i64::MAX)] {
        let mut machine = Machine::start(
            program.clone(),
            StartRequest {
                agent: "Worker".to_owned(),
                event: "message".to_owned(),
                event_id: "evt-001".to_owned(),
                event_time: "2026-08-05T12:00:00Z".to_owned(),
                agent_arguments: BTreeMap::new(),
                payload,
                state: BTreeMap::new(),
                capabilities: grants(&[]),
            },
        )
        .expect("machine starts");

        loop {
            match machine.step() {
                Step::Continue => {}
                Step::Failed(MachineError::TypeMismatch(message)) => {
                    assert_eq!(message, "integer arithmetic");
                    break;
                }
                other => panic!("expected controlled arithmetic failure, got {other:?}"),
            }
        }
    }
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
                assert_eq!(
                    outcome.state["last_event"],
                    json!({"some": {"id": "event-001"}})
                );
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
    let mut unknown_nested: JsonValue =
        serde_json::from_str(&json).expect("snapshot JSON is inspectable");
    unknown_nested["frames"][0]["unexpected"] = json!(true);
    assert!(matches!(
        MachineSnapshot::from_json(
            &serde_json::to_string(&unknown_nested).expect("mutated snapshot serializes")
        ),
        Err(MachineError::Serialization(_))
    ));
    let mut unsupported: JsonValue =
        serde_json::from_str(&json).expect("snapshot JSON is inspectable");
    unsupported["schema_version"] = json!(2);
    assert!(matches!(
        MachineSnapshot::from_json(
            &serde_json::to_string(&unsupported).expect("mutated snapshot serializes")
        ),
        Err(MachineError::SnapshotSchemaMismatch)
    ));
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

    let mut ill_typed = base.clone();
    ill_typed.capabilities = grants(&[("ModelUse", json!(7))]);
    let result = Machine::start(inference_program(), ill_typed);
    assert!(matches!(result, Err(MachineError::InvalidCapabilityGrant)));

    let mut duplicate = base;
    duplicate.capabilities = grants(&[
        ("ModelUse", json!("planner")),
        ("ModelUse", json!("planner")),
    ]);
    assert!(matches!(
        Machine::start(inference_program(), duplicate),
        Err(MachineError::Capability(_))
    ));
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
fn agent_arguments_are_an_exact_declared_object() {
    let mut request = StartRequest {
        agent: "A".to_owned(),
        event: "message".to_owned(),
        event_id: "evt-001".to_owned(),
        event_time: "2026-08-05T12:00:00Z".to_owned(),
        agent_arguments: BTreeMap::from([("unexpected".to_owned(), json!(true))]),
        payload: json!({"text": "hello"}),
        state: BTreeMap::new(),
        capabilities: grants(&[("ModelUse", json!("planner"))]),
    };

    assert_eq!(
        Machine::start(inference_program(), request.clone()).err(),
        Some(MachineError::UnknownAgentArgument)
    );
    request.agent_arguments.clear();
    assert!(Machine::start(inference_program(), request).is_ok());
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

fn sum_boundaries_program() -> aster_ir::Program {
    let source = SourceFile::new(
        "sum-boundaries.aster",
        r#"module sum_boundaries;
type Payload = { error: Text };
capability Read(scope: Text);
tool Remote.result() -> Result<Payload, Text> {
  mode read;
  capability Read("remote");
  sensitivity private;
}
tool Remote.option() -> Option<Unit> {
  mode read;
  capability Read("remote");
  sensitivity private;
}
agent Worker() requires [Read("remote")] {
  state {
    outcome: Result<Payload, Text> = Err("unset");
    optional: Option<Unit> = None;
  }
  budget per_event { external_reads <= 2; }
  on message(msg: Incoming<Unit>) -> Result<Unit, Error> {
    let outcome = (observe Remote.result())?;
    let optional = (observe Remote.option())?;
    update state { outcome = outcome.value; optional = optional.value; }
    return Ok(Unit);
  }
}
"#,
    );
    lower(&check_source(&source).expect("source checks")).expect("source lowers")
}

#[test]
fn option_and_result_boundary_encodings_are_unambiguous() {
    let program = sum_boundaries_program();
    let cases = [
        (
            [json!({"ok": {"error": "legit"}}), json!({"some": null})],
            json!({"ok": {"error": "legit"}}),
            json!({"some": null}),
        ),
        (
            [json!({"error": "bad"}), JsonValue::Null],
            json!({"error": "bad"}),
            JsonValue::Null,
        ),
    ];

    for (resolutions, expected_result, expected_option) in cases {
        let mut machine = Machine::start(
            program.clone(),
            StartRequest {
                agent: "Worker".to_owned(),
                event: "message".to_owned(),
                event_id: "evt-001".to_owned(),
                event_time: "2026-08-05T12:00:00Z".to_owned(),
                agent_arguments: BTreeMap::new(),
                payload: JsonValue::Null,
                state: BTreeMap::new(),
                capabilities: grants(&[("Read", json!("remote"))]),
            },
        )
        .expect("machine starts");
        let mut resolution = resolutions.into_iter();

        loop {
            match machine.step() {
                Step::Continue => {}
                Step::Yield(effect) => machine
                    .supply(&EffectResolution {
                        request_hash: effect.request_hash,
                        payload: resolution.next().expect("one payload per read"),
                        actual_usage: BTreeMap::new(),
                    })
                    .expect("sum boundary payload decodes"),
                Step::Completed(outcome) => {
                    assert_eq!(outcome.state["outcome"], expected_result);
                    assert_eq!(outcome.state["optional"], expected_option);
                    assert!(resolution.next().is_none());
                    break;
                }
                Step::Failed(error) => panic!("sum boundary program failed: {error}"),
            }
        }
    }

    let start_with_state = |state| {
        Machine::start(
            program.clone(),
            StartRequest {
                agent: "Worker".to_owned(),
                event: "message".to_owned(),
                event_id: "evt-001".to_owned(),
                event_time: "2026-08-05T12:00:00Z".to_owned(),
                agent_arguments: BTreeMap::new(),
                payload: JsonValue::Null,
                state,
                capabilities: grants(&[("Read", json!("remote"))]),
            },
        )
    };
    assert_eq!(
        start_with_state(BTreeMap::from([(
            "outcome".to_owned(),
            json!({"ok": {"error": "legit"}, "error": "ambiguous"}),
        )]))
        .err(),
        Some(MachineError::TypeMismatch("Result".to_owned()))
    );
    assert_eq!(
        start_with_state(BTreeMap::from([(
            "optional".to_owned(),
            json!({"some": null, "unexpected": true}),
        )]))
        .err(),
        Some(MachineError::TypeMismatch("Option".to_owned()))
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

#[test]
fn user_enum_decodes_constructs_and_matches_in_the_vm() {
    let source = SourceFile::new(
        "enum.aster",
        r#"module choices.example;
enum Choice { First, Other(Text), }
capability ModelUse(alias: Text);
prompt Parse(message: Untrusted<Text>) -> Choice {
  instruction """Choose one declared variant.""";
  data { message, };
}
validator Valid(x: Choice) { require true; }
agent A() requires [ModelUse("planner")] {
  state { choice: Choice = First; }
  budget per_event { model_calls <= 1; }
  on message(msg: Incoming<Untrusted<Text>>) -> Result<Unit, Error> {
    let candidate = (infer Parse(message = msg.value) using @planner)?;
    let checked = (validate candidate with Valid)?;
    let selected = match checked.value {
      First => 0,
      Other(value) => 1,
    };
    let conditional = if true { selected; } else { 0; };
    require (conditional == 1);
    return Ok(Unit);
  }
}
"#,
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
            state: BTreeMap::new(),
            capabilities: grants(&[("ModelUse", json!("planner"))]),
        },
    )
    .expect("enum state decodes");
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => machine
                .supply(&EffectResolution {
                    request_hash: request.request_hash,
                    payload: json!({"variant": "Other", "value": "selected"}),
                    actual_usage: BTreeMap::new(),
                })
                .expect("enum response decodes"),
            Step::Completed(_) => break,
            Step::Failed(error) => panic!("enum handler must complete, got {error:?}"),
        }
    }
}

#[test]
fn validator_failure_reports_every_requirement_and_candidate_provenance() {
    let source = SourceFile::new(
        "validator-evidence.aster",
        r#"module evidence.validation;
type Answer = { value: Int, };
capability ModelUse(alias: Text);
prompt Parse(message: Untrusted<Text>) -> Answer {
  instruction """Extract one integer.""";
  data { message, };
}
validator Rules(x: Answer) {
  require (x.value >= 10);
  require (x.value >= 30);
}
agent A() requires [ModelUse("planner")] {
  state {}
  budget per_event { model_calls <= 1; }
  on message(msg: Incoming<Untrusted<Text>>) -> Result<Unit, Error> {
    let candidate = (infer Parse(message = msg.value) using @planner)?;
    let checked = (validate candidate with Rules)?;
    return Ok(Unit);
  }
}
"#,
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
            state: BTreeMap::new(),
            capabilities: grants(&[("ModelUse", json!("planner"))]),
        },
    )
    .expect("machine starts");
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => machine
                .supply(&EffectResolution {
                    request_hash: request.request_hash,
                    payload: json!({"value": 5}),
                    actual_usage: BTreeMap::new(),
                })
                .expect("model response decodes"),
            Step::Failed(MachineError::PropagatedError(message)) => {
                let positions: Vec<_> = message
                    .match_indices("validator-evidence.aster:")
                    .map(|(position, _)| position)
                    .collect();
                assert_eq!(positions.len(), 2, "both requirement spans: {message}");
                let first = positions[0];
                let second = positions[1];
                assert!(first < second, "requirements remain in source order");
                assert!(message.contains("provenance"));
                break;
            }
            other => panic!("expected validation failure, got {other:?}"),
        }
    }
}

#[test]
fn committed_receipt_cannot_be_discarded_without_reconciliation() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("tests/conformance/pass/direct_allow.aster");
    let text = std::fs::read_to_string(path)
        .expect("fixture is readable")
        .replace(
            "    let actual = (observe Store.get(owner = owner, id = receipt.value.id))?;\n    let confirmed = (reconcile receipt against actual with Matches)?;\n",
            "",
        );
    let source = SourceFile::new("unreconciled.aster", text);
    let program = lower(&check_source(&source).expect("source checks")).expect("source lowers");
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
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Yield(request) => {
                assert_eq!(request.kind, EffectKind::Write);
                machine
                    .supply(&EffectResolution {
                        request_hash: request.request_hash,
                        payload: json!({"id": "created-001"}),
                        actual_usage: BTreeMap::new(),
                    })
                    .expect("write response decodes");
            }
            Step::Failed(MachineError::UnreconciledReceipt) => break,
            other => panic!("expected unreconciled receipt failure, got {other:?}"),
        }
    }
}

#[test]
fn add_seconds_normalizes_across_a_utc_day_boundary() {
    let source = SourceFile::new(
        "instant.aster",
        r"module instant.boundary;
agent A(start: Instant) requires [] {
  state { next: Instant = start; }
  budget per_event {}
  on message(msg: Incoming<Text>) -> Result<Unit, Error> {
    update state { next = add_seconds(event.time, 120); }
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
            event_time: "2026-08-05T23:59:30Z".to_owned(),
            agent_arguments: BTreeMap::from([("start".to_owned(), json!("2026-01-01T00:00:00Z"))]),
            payload: json!("go"),
            state: BTreeMap::new(),
            capabilities: grants(&[]),
        },
    )
    .expect("machine starts");
    loop {
        match machine.step() {
            Step::Continue => {}
            Step::Completed(outcome) => {
                assert_eq!(outcome.state["next"], json!("2026-08-06T00:01:30Z"));
                break;
            }
            other => panic!("expected completion, got {other:?}"),
        }
    }
}
