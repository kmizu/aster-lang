use std::collections::BTreeMap;

use aster_runtime::{
    AuthorityError, AuthorityLedger, Budget, BudgetDimension, BudgetError, DriverError,
    EffectDriver, EffectKind, EffectRequest, FixtureDriver, FixtureEntry, FixtureSet, Intent,
    Proposal, ProposalMetadata, RuntimeValue, SnapshotError, Trace, TraceError, canonical_json,
};
use serde_json::json;

fn metadata(
    risk: &str,
    sensitivity: &str,
    capability_request: serde_json::Value,
    idempotency_key: &str,
    program_hash: &str,
) -> ProposalMetadata {
    ProposalMetadata {
        risk: risk.to_owned(),
        sensitivity: sensitivity.to_owned(),
        capability_request,
        idempotency_key: idempotency_key.to_owned(),
        program_hash: program_hash.to_owned(),
    }
}

fn proposal() -> Proposal {
    Proposal::new(
        "Calendar.create",
        BTreeMap::from([
            ("owner".to_owned(), json!("user-001")),
            ("request_id".to_owned(), json!("evt-001")),
        ]),
        Intent {
            purpose: "ScheduleMeeting".to_owned(),
            fields: BTreeMap::from([("expected".to_owned(), json!("created"))]),
            expires_at: "2026-08-05T12:02:00Z".to_owned(),
        },
        metadata(
            "reversible",
            "private",
            json!({"capability": "CalendarWrite", "arguments": ["user-001"]}),
            "evt-001",
            "program-hash",
        ),
    )
    .expect("proposal is canonicalizable")
}

fn assert_proposal_hash_changes(
    base: &Proposal,
    action: &str,
    arguments: BTreeMap<String, serde_json::Value>,
    intent: Intent,
    metadata: ProposalMetadata,
) {
    let changed = Proposal::new(action, arguments, intent, metadata).expect("mutation hashes");
    assert_ne!(changed.hash(), base.hash());
}

#[test]
fn proposal_hash_binds_every_authority_relevant_field() {
    // Catches permits that authorize a mutable or incompletely hashed proposal.
    let base = proposal();
    let same = || {
        metadata(
            &base.risk,
            &base.sensitivity,
            base.capability_request.clone(),
            &base.idempotency_key,
            &base.program_hash,
        )
    };
    assert_proposal_hash_changes(
        &base,
        "Calendar.other",
        base.arguments.clone(),
        base.intent.clone(),
        same(),
    );
    assert_proposal_hash_changes(
        &base,
        &base.action,
        BTreeMap::from([("owner".to_owned(), json!("other"))]),
        base.intent.clone(),
        same(),
    );
    assert_proposal_hash_changes(
        &base,
        &base.action,
        base.arguments.clone(),
        Intent {
            purpose: "Other".to_owned(),
            ..base.intent.clone()
        },
        same(),
    );
    for changed in [
        metadata(
            "irreversible",
            &base.sensitivity,
            base.capability_request.clone(),
            &base.idempotency_key,
            &base.program_hash,
        ),
        metadata(
            &base.risk,
            "secret",
            base.capability_request.clone(),
            &base.idempotency_key,
            &base.program_hash,
        ),
        metadata(
            &base.risk,
            &base.sensitivity,
            json!({"capability": "Other"}),
            &base.idempotency_key,
            &base.program_hash,
        ),
        metadata(
            &base.risk,
            &base.sensitivity,
            base.capability_request.clone(),
            "other-key",
            &base.program_hash,
        ),
        metadata(
            &base.risk,
            &base.sensitivity,
            base.capability_request.clone(),
            &base.idempotency_key,
            "other-program",
        ),
    ] {
        assert_proposal_hash_changes(
            &base,
            &base.action,
            base.arguments.clone(),
            base.intent.clone(),
            changed,
        );
    }
}

#[test]
fn permit_is_bound_expiring_and_single_use() {
    // Catches low-level callers bypassing source affine analysis.
    let first = proposal();
    let second = Proposal::new(
        &first.action,
        BTreeMap::from([("request_id".to_owned(), json!("evt-002"))]),
        first.intent.clone(),
        metadata(
            &first.risk,
            &first.sensitivity,
            first.capability_request.clone(),
            "evt-002",
            &first.program_hash,
        ),
    )
    .unwrap();
    let mut ledger = AuthorityLedger::default();
    let permit = ledger.issue(
        &first,
        "DirectPolicy",
        "grant-a",
        "2026-08-05T12:00:00Z",
        "2026-08-05T12:01:00Z",
        "direct_allow",
    );
    let mut forged_json = serde_json::to_value(&permit).expect("permit serializes for persistence");
    forged_json["policy"] = json!("ForgedPolicy");
    let forged: aster_runtime::Permit =
        serde_json::from_value(forged_json).expect("forged shape remains decodable");
    assert_eq!(
        ledger.consume(&first, &forged, "grant-a", "2026-08-05T12:00:30Z"),
        Err(AuthorityError::ForgedPermit)
    );

    assert_eq!(
        ledger.consume(&second, &permit, "grant-a", "2026-08-05T12:00:30Z"),
        Err(AuthorityError::ProposalMismatch)
    );
    assert_eq!(
        ledger.consume(&first, &permit, "grant-b", "2026-08-05T12:00:30Z"),
        Err(AuthorityError::GrantMismatch)
    );
    assert_eq!(
        ledger.consume(&first, &permit, "grant-a", "2026-08-05T12:01:01Z"),
        Err(AuthorityError::Expired)
    );
    ledger
        .consume(&first, &permit, "grant-a", "2026-08-05T12:00:30Z")
        .expect("matching permit is consumed once");
    assert_eq!(
        ledger.consume(&first, &permit, "grant-a", "2026-08-05T12:00:30Z"),
        Err(AuthorityError::AlreadyConsumed)
    );
}

#[test]
fn budget_reservation_precedes_and_bounds_actual_usage() {
    // Catches drivers being invoked before deterministic budget admission.
    let mut budget = Budget::new(BTreeMap::from([
        (BudgetDimension::ModelCalls, 1),
        (BudgetDimension::ModelTokens, 100),
    ]));
    let reservation = budget
        .reserve(BudgetDimension::ModelCalls, 1)
        .expect("first model call fits");
    assert_eq!(
        budget.reserve(BudgetDimension::ModelCalls, 1),
        Err(BudgetError::Exhausted(BudgetDimension::ModelCalls))
    );
    assert_eq!(
        budget.settle(reservation, 2),
        Err(BudgetError::ActualExceedsReservation)
    );
}

#[test]
fn fixture_actual_usage_above_maximum_is_rejected_after_one_driver_call() {
    let request = EffectRequest {
        kind: EffectKind::Model,
        identity: "Parse".to_owned(),
        payload: json!({"model_alias": "planner"}),
        request_hash: "request-001".to_owned(),
    };
    let mut driver = FixtureDriver::new(FixtureSet {
        schema_version: 1,
        entries: vec![FixtureEntry {
            kind: EffectKind::Model,
            identity: "Parse".to_owned(),
            match_request: json!({"model_alias": "planner"}),
            response: json!({"value": "answer"}),
            max_usage: BTreeMap::from([("model_tokens".to_owned(), 5)]),
            actual_usage: BTreeMap::from([("model_tokens".to_owned(), 6)]),
        }],
    })
    .expect("fixture set is structurally valid");
    let preview = driver.preview(&request).expect("fixture matches");
    assert_eq!(
        driver.resolve(&request, &preview),
        Err(DriverError::ActualExceedsMaximum)
    );
    assert_eq!(driver.call_count(EffectKind::Model), 1);
}

#[test]
fn opaque_secret_sentinel_cannot_escape_through_generic_serialization_or_debug() {
    let sentinel = "UNIQUE-ASTER-SECRET-SENTINEL-7f31";
    let secret = RuntimeValue::secret_for_test(sentinel);
    let debug = format!("{secret:?}");
    let error = serde_json::to_string(&secret).expect_err("secret serialization must fail");
    assert!(!debug.contains(sentinel));
    assert!(!error.to_string().contains(sentinel));
}

#[test]
fn canonical_json_and_trace_chain_are_deterministic_and_tamper_evident() {
    let left = json!({"z": 1, "a": {"d": 2, "b": 1}});
    let right = json!({"a": {"b": 1, "d": 2}, "z": 1});
    assert_eq!(
        canonical_json(&left).unwrap(),
        canonical_json(&right).unwrap()
    );

    let mut trace = Trace::new("run-001");
    trace.append("event_received", left).unwrap();
    trace.append("run_completed", json!({"ok": true})).unwrap();
    trace.verify().expect("original chain verifies");
    trace.entries[0].payload = json!({"tampered": true});
    assert_eq!(
        trace.verify(),
        Err(TraceError::HashMismatch { sequence: 0 })
    );
}

#[test]
fn secret_is_rejected_before_snapshot_serialization() {
    let values = BTreeMap::from([(
        "token".to_owned(),
        RuntimeValue::secret_for_test("UNIQUE-SECRET-SENTINEL"),
    )]);
    let error = aster_runtime::snapshot_values(&values).expect_err("secret must not serialize");
    assert_eq!(error, SnapshotError::SecretPresent);
    assert!(!error.to_string().contains("UNIQUE-SECRET-SENTINEL"));
}
