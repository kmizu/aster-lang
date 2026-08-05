use std::{fs, path::PathBuf};

use aster_semantics::check_source;
use aster_syntax::SourceFile;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn diagnostic_codes(source: &str) -> Vec<String> {
    let source = SourceFile::new("inline.aster", source);
    check_source(&source)
        .expect_err("source must be rejected")
        .into_iter()
        .map(|diagnostic| diagnostic.code.as_str().to_owned())
        .collect()
}

fn assert_checks(source: &str) {
    let source = SourceFile::new("inline.aster", source);
    check_source(&source).expect("source must check");
}

#[test]
fn unknown_declared_types_are_rejected() {
    let codes = diagnostic_codes("module test; type Broken = Missing;");

    assert_eq!(codes, ["ASTER-NAME-1001"]);
}

#[test]
fn duplicate_record_fields_are_rejected() {
    let codes = diagnostic_codes("module test; type Pair = { value: Int, value: Int };");

    assert_eq!(codes, ["ASTER-NAME-1002"]);
}

#[test]
fn tools_require_complete_mode_specific_metadata() {
    let codes = diagnostic_codes("module test; tool Store.get(id: Text) -> Text { mode read; }");

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
}

#[test]
fn write_idempotency_names_a_real_parameter() {
    let codes = diagnostic_codes(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Text {
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk reversible;
  idempotency request_id;
}",
    );

    assert_eq!(codes, ["ASTER-TYPE-2004"]);
}

#[test]
fn cyclic_type_aliases_are_rejected() {
    let codes = diagnostic_codes("module test; type First = Second; type Second = First;");

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
}

#[test]
fn duplicate_parameter_names_are_rejected() {
    let codes =
        diagnostic_codes("module test; fn choose(value: Int, value: Int) -> Int { return value; }");

    assert_eq!(codes, ["ASTER-NAME-1002"]);
}

#[test]
fn duplicate_enum_variants_are_rejected() {
    let codes = diagnostic_codes("module test; enum Choice { First, First }");

    assert_eq!(codes, ["ASTER-NAME-1002"]);
}

#[test]
fn duplicate_state_fields_and_handlers_are_rejected() {
    let codes = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state { value: Int = 1; value: Int = 2; }
  budget per_event {}
  on tick() -> Result<Unit, Error> { return Ok(Unit); }
  on tick() -> Result<Unit, Error> { return Ok(Unit); }
}",
    );

    assert_eq!(codes, ["ASTER-NAME-1002", "ASTER-NAME-1002"]);
}

#[test]
fn prompt_data_names_are_exact_and_unique() {
    let codes = diagnostic_codes(
        r#"module test;
prompt Parse(value: Text) -> Text {
  instruction """static""";
  data { value, value };
}"#,
    );

    assert_eq!(codes, ["ASTER-NAME-1002"]);
}

#[test]
fn tool_capabilities_must_be_declared() {
    let codes = diagnostic_codes(
        r"module test;
tool Store.get(id: Text) -> Text {
  mode read;
  capability Missing(id);
  sensitivity private;
}",
    );

    assert_eq!(codes, ["ASTER-NAME-1001"]);
}

#[test]
fn capability_requests_are_declared_and_exactly_typed() {
    let unknown = diagnostic_codes(
        r"module test;
flow bad(id: Text) -> Text uses [Missing(id)] { return id; }",
    );
    let wrong_type = diagnostic_codes(
        r"module test;
capability StoreRead(id: Text);
flow bad(id: Int) -> Int uses [StoreRead(id)] { return id; }",
    );
    let missing = diagnostic_codes(
        r"module test;
capability StoreRead(id: Text);
agent Worker() requires [StoreRead()] {
  state {}
  budget per_event {}
}",
    );

    assert_eq!(unknown, ["ASTER-NAME-1001"]);
    assert_eq!(wrong_type, ["ASTER-TYPE-2002"]);
    assert_eq!(missing, ["ASTER-TYPE-2002"]);
}

#[test]
fn secret_types_are_confined_to_secret_tool_parameters() {
    let function_parameter =
        diagnostic_codes("module test; fn bad(value: Secret<Text>) -> Unit { return Unit; }");
    let record_field = diagnostic_codes("module test; type Bad = { value: Secret<Text> };");
    let non_secret_tool = diagnostic_codes(
        r"module test;
capability VaultWrite(id: Text);
tool Vault.put(id: Text, value: Secret<Text>) -> Unit {
  mode write;
  capability VaultWrite(id);
  sensitivity private;
  risk irreversible;
  idempotency id;
}",
    );
    let secret_return = diagnostic_codes(
        r"module test;
capability VaultRead(id: Text);
tool Vault.get(id: Text) -> Secret<Text> {
  mode read;
  capability VaultRead(id);
  sensitivity secret;
}",
    );

    assert_eq!(function_parameter, ["ASTER-SECRET-8002"]);
    assert_eq!(record_field, ["ASTER-SECRET-8002"]);
    assert_eq!(non_secret_tool, ["ASTER-SECRET-8002"]);
    assert_eq!(secret_return, ["ASTER-SECRET-8002"]);

    assert_checks(
        r"module test;
capability VaultWrite(id: Text);
tool Vault.put(id: Text, value: Secret<Text>) -> Unit {
  mode write;
  capability VaultWrite(id);
  sensitivity secret;
  risk irreversible;
  idempotency id;
}",
    );
}

#[test]
fn built_in_types_cannot_be_shadowed() {
    let codes = diagnostic_codes("module test; type Text = Int;");

    assert_eq!(codes, ["ASTER-NAME-1002"]);
}

#[test]
fn validator_arity_is_one_or_two() {
    let codes = diagnostic_codes(
        "module test; validator Empty() {} validator TooMany(a: Int, b: Int, c: Int) {}",
    );

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
}

#[test]
fn prompt_data_matches_parameters_exactly() {
    let codes = diagnostic_codes(
        r#"module test;
prompt Parse(first: Text, second: Text) -> Text {
  instruction """static""";
  data { first, missing };
}"#,
    );

    assert_eq!(codes, ["ASTER-NAME-1001", "ASTER-TYPE-2002"]);
}

#[test]
fn policies_require_proposal_and_optional_agent_state_parameters() {
    let codes = diagnostic_codes(
        r#"module test;
policy Invalid(value: Text) {
  deny "no" otherwise;
}"#,
    );

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

#[test]
fn read_tools_reject_write_only_metadata() {
    let codes = diagnostic_codes(
        r"module test;
capability StoreRead(id: Text);
tool Store.get(id: Text) -> Text {
  mode read;
  capability StoreRead(id);
  sensitivity private;
  risk reversible;
  idempotency id;
}",
    );

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
}

#[test]
fn calls_require_every_parameter_exactly_once() {
    let missing = diagnostic_codes(
        "module test; fn add(x: Int, y: Int) -> Int { return x + y; } fn bad() -> Int { return add(x = 1); }",
    );
    let duplicate = diagnostic_codes(
        "module test; fn id(x: Int) -> Int { return x; } fn bad() -> Int { return id(x = 1, x = 2); }",
    );

    assert_eq!(missing, ["ASTER-TYPE-2002"]);
    assert_eq!(duplicate, ["ASTER-NAME-1002"]);
}

#[test]
fn calls_reject_unknown_and_extra_arguments() {
    let unknown = diagnostic_codes(
        "module test; fn id(x: Int) -> Int { return x; } fn bad() -> Int { return id(other = 1); }",
    );
    let extra = diagnostic_codes(
        "module test; fn id(x: Int) -> Int { return x; } fn bad() -> Int { return id(1, 2); }",
    );

    assert_eq!(unknown, ["ASTER-TYPE-2002", "ASTER-NAME-1001"]);
    assert_eq!(extra, ["ASTER-TYPE-2002"]);
}

#[test]
fn record_construction_requires_an_exact_field_set() {
    let missing = diagnostic_codes(
        "module test; type Pair = { left: Int, right: Int }; fn bad() -> Pair { return Pair { left = 1 }; }",
    );
    let extra = diagnostic_codes(
        "module test; type Pair = { left: Int }; fn bad() -> Pair { return Pair { left = 1, right = 2 }; }",
    );
    let duplicate = diagnostic_codes(
        "module test; type Pair = { left: Int }; fn bad() -> Pair { return Pair { left = 1, left = 2 }; }",
    );

    assert_eq!(missing, ["ASTER-TYPE-2002"]);
    assert_eq!(extra, ["ASTER-NAME-1001"]);
    assert_eq!(duplicate, ["ASTER-NAME-1002"]);
}

#[test]
fn if_expression_unifies_branch_result_types() {
    let codes =
        diagnostic_codes("module test; fn bad() -> Int { return if true { 1; } else { false; }; }");

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

#[test]
fn opaque_values_are_not_equatable() {
    let candidate = diagnostic_codes(
        "module test; fn bad(a: Candidate<Text>, b: Candidate<Text>) -> Bool { return a == b; }",
    );
    let secret = diagnostic_codes(
        "module test; fn bad(a: Secret<Text>, b: Secret<Text>) -> Bool { return a == b; }",
    );

    assert_eq!(candidate, ["ASTER-TYPE-2002"]);
    assert_eq!(
        secret,
        ["ASTER-SECRET-8002", "ASTER-SECRET-8002", "ASTER-TYPE-2002"]
    );
}

#[test]
fn ordering_requires_integers_and_collection_builtins_are_homogeneous() {
    let ordering =
        diagnostic_codes("module test; fn bad(a: Text, b: Text) -> Bool { return a < b; }");
    let contains = diagnostic_codes(
        "module test; fn bad(values: List<Int>) -> Bool { return contains(values, \"x\"); }",
    );

    assert_eq!(ordering, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
    assert_eq!(contains, ["ASTER-TYPE-2002"]);
}

#[test]
fn enum_match_binds_payload_and_is_exhaustive() {
    assert_checks(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  return match choice { Choice.First => 0, Choice.Other(value) => value };
}",
    );

    let non_exhaustive = diagnostic_codes(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  return match choice { Choice.First => 0 };
}",
    );
    assert_eq!(non_exhaustive, ["ASTER-TYPE-2002"]);
}

#[test]
fn intent_requires_the_exact_governance_fields_and_types() {
    let missing = diagnostic_codes(
        r#"module test;
fn make(time: Instant, source: ProvenanceRef) -> Intent<Purpose> {
  return intent Purpose {
    actor = "agent";
    beneficiary = "user";
    basis = [source];
    expires_at = time;
  };
}"#,
    );
    let wrong_basis = diagnostic_codes(
        r#"module test;
fn make(time: Instant) -> Intent<Purpose> {
  return intent Purpose {
    actor = "agent";
    beneficiary = "user";
    basis = ["not provenance"];
    expected = "event";
    expires_at = time;
  };
}"#,
    );

    assert_eq!(missing, ["ASTER-TYPE-2002"]);
    assert_eq!(wrong_basis, ["ASTER-TYPE-2002"]);
}

#[test]
fn intent_basis_is_non_empty_and_unknown_fields_are_rejected() {
    let codes = diagnostic_codes(
        r#"module test;
fn make(time: Instant) -> Intent<Purpose> {
  return intent Purpose {
    actor = "agent";
    beneficiary = "user";
    basis = [];
    expected = "event";
    expires_at = time;
    surprise = 1;
  };
}"#,
    );

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-NAME-1001"]);
}

#[test]
fn validator_use_requires_a_declared_compatible_arity() {
    let unknown = diagnostic_codes(
        "module test; fn bad(value: Candidate<Text>) -> Result<Checked<Text>, Error> { return validate value with Missing; }",
    );
    let reconciliation_only = diagnostic_codes(
        "module test; validator Pair(a: Text, b: Text) {} fn bad(value: Candidate<Text>) -> Result<Checked<Text>, Error> { return validate value with Pair; }",
    );

    assert_eq!(unknown, ["ASTER-NAME-1001"]);
    assert_eq!(reconciliation_only, ["ASTER-TYPE-2002"]);
}

#[test]
fn effect_references_must_name_declared_boundaries() {
    let prompt = diagnostic_codes(
        r#"module test;
capability ModelUse(alias: Text);
flow bad() -> Unit uses [ModelUse("planner")] {
  infer Missing() using @planner;
  return Unit;
}"#,
    );
    let tool = diagnostic_codes(
        "module test; flow bad() -> Unit uses [] { observe Missing(); return Unit; }",
    );
    let policy = diagnostic_codes(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Unit {
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk irreversible;
  idempotency id;
}
flow bad(proposal: Proposal<Store.put>) -> Unit uses [] {
  authorize proposal using Missing;
  return Unit;
}",
    );

    assert_eq!(prompt, ["ASTER-NAME-1001"]);
    assert_eq!(tool, ["ASTER-NAME-1001"]);
    assert_eq!(policy, ["ASTER-NAME-1001"]);
}

#[test]
fn reconciliation_requires_a_declared_two_parameter_validator() {
    let unknown = diagnostic_codes(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Text {
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk irreversible;
  idempotency id;
}
flow bad(receipt: Receipt<Store.put>, actual: Observation<Text>) -> Unit uses [] {
  reconcile receipt against actual with Missing;
  return Unit;
}",
    );
    let wrong_shape = diagnostic_codes(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Text {
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk irreversible;
  idempotency id;
}
validator Wrong(actual: Int) { require true; }
flow bad(receipt: Receipt<Store.put>, actual: Observation<Text>) -> Unit uses [] {
  reconcile receipt against actual with Wrong;
  return Unit;
}",
    );

    assert_eq!(unknown, ["ASTER-NAME-1001"]);
    assert_eq!(wrong_shape, ["ASTER-TYPE-2002"]);
}

#[test]
fn governance_action_types_name_declared_write_tools() {
    let codes = diagnostic_codes(
        r#"module test;
policy Invalid(proposal: Proposal<Missing.put>) {
  deny "no" otherwise;
}"#,
    );

    assert_eq!(codes, ["ASTER-NAME-1001"]);
}

#[test]
fn policy_decision_payloads_are_typed() {
    let codes = diagnostic_codes(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Text {
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk reversible;
  idempotency id;
}
policy Invalid(proposal: Proposal<Store.put>) {
  deny 1 otherwise;
}",
    );

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

#[test]
fn executable_bodies_require_an_explicit_return() {
    let codes = diagnostic_codes("module test; fn missing() -> Unit { Unit; }");

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

fn affine_prelude(body: &str) -> String {
    format!(
        r"module test;
capability StoreWrite(id: Text);
tool Store.put(id: Text) -> Text {{
  mode write;
  capability StoreWrite(id);
  sensitivity private;
  risk reversible;
  idempotency id;
}}
{body}",
    )
}

#[test]
fn assigning_an_affine_value_transfers_ownership() {
    let source = affine_prelude(
        r#"flow bad(
  proposal: Proposal<Store.put>,
  permit: Permit<Store.put>
) -> Result<Text, Error> uses [StoreWrite("scope")] {
  let alias = proposal;
  let receipt = (commit alias with permit)?;
  let reused = proposal.args.id;
  return Ok(receipt.value);
}"#,
    );
    let codes = diagnostic_codes(&source);

    assert_eq!(codes, ["ASTER-AFFINE-5002"]);
}

#[test]
fn affine_consumption_is_joined_across_if_branches() {
    let source = affine_prelude(
        r#"flow bad(
  condition: Bool,
  proposal: Proposal<Store.put>,
  permit: Permit<Store.put>
) -> Result<Unit, Error> uses [StoreWrite("scope")] {
  if condition {
    let receipt = (commit proposal with permit)?;
    Unit;
  } else {
    Unit;
  };
  let reused = proposal.args.id;
  return Ok(Unit);
}"#,
    );
    let codes = diagnostic_codes(&source);

    assert_eq!(codes, ["ASTER-AFFINE-5002"]);
}

#[test]
fn commit_is_rejected_in_a_pure_function() {
    let source = affine_prelude(
        r"fn bad(
  proposal: Proposal<Store.put>,
  permit: Permit<Store.put>
) -> Result<Text, Error> {
  let receipt = (commit proposal with permit)?;
  return Ok(receipt.value);
}",
    );
    let codes = diagnostic_codes(&source);

    assert_eq!(codes, ["ASTER-EFFECT-3004"]);
}

#[test]
fn mandatory_compile_fail_fixtures_have_stable_codes_and_relevant_spans() {
    // Catches missing governance rules and diagnostics that point away from the unsafe construct.
    let cases = [
        (
            "candidate_used_without_validation.aster",
            "ASTER-TYPE-2001",
            "candidate.value",
        ),
        ("duplicate_declaration.aster", "ASTER-NAME-1002", "fn same"),
        (
            "candidate_passed_to_write.aster",
            "ASTER-TYPE-2001",
            "candidate",
        ),
        (
            "write_tool_observed.aster",
            "ASTER-EFFECT-3001",
            "Store.put",
        ),
        ("read_tool_proposed.aster", "ASTER-EFFECT-3002", "Store.get"),
        ("direct_tool_call.aster", "ASTER-EFFECT-3003", "Store.get"),
        (
            "commit_without_permit.aster",
            "ASTER-TYPE-2003",
            "commit proposal",
        ),
        ("permit_action_mismatch.aster", "ASTER-TYPE-2005", "permit"),
        ("permit_reused.aster", "ASTER-AFFINE-5001", "permit"),
        ("proposal_reused.aster", "ASTER-AFFINE-5002", "proposal"),
        (
            "effect_in_policy.aster",
            "ASTER-EFFECT-3004",
            "observe Store.get",
        ),
        (
            "non_total_policy.aster",
            "ASTER-POLICY-4001",
            "policy Incomplete",
        ),
        ("missing_capability.aster", "ASTER-CAP-6001", "Store.get"),
        (
            "dynamic_prompt_instruction.aster",
            "ASTER-PROMPT-7001",
            "message",
        ),
        ("secret_to_model.aster", "ASTER-SECRET-8001", "Secret<Text>"),
        ("secret_in_state.aster", "ASTER-SECRET-8002", "Secret<Text>"),
        ("direct_recursion.aster", "ASTER-EFFECT-3005", "fn recurse"),
        ("mutual_recursion.aster", "ASTER-EFFECT-3005", "fn first"),
        (
            "unknown_budget.aster",
            "ASTER-BUDGET-11001",
            "network_calls",
        ),
        (
            "duplicate_budget.aster",
            "ASTER-BUDGET-11002",
            "model_calls",
        ),
        (
            "write_without_idempotency.aster",
            "ASTER-TYPE-2004",
            "tool Store.put",
        ),
    ];
    for (file, expected_code, offending_text) in cases {
        let path = repository_root().join("tests/conformance/fail").join(file);
        let text = fs::read_to_string(&path).expect("fixture is readable UTF-8");
        let source = SourceFile::new(path.display().to_string(), &text);
        let diagnostics = check_source(&source).expect_err("fixture must be rejected");
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code.as_str() == expected_code)
            .unwrap_or_else(|| {
                panic!(
                    "{file}: expected {expected_code}, got {:?}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.code.as_str())
                        .collect::<Vec<_>>()
                )
            });
        let span_text = &text[diagnostic.primary_span.start..diagnostic.primary_span.end];
        assert!(
            offending_text.contains(span_text) || span_text.contains(offending_text),
            "{file}: diagnostic span {span_text:?} is not relevant to {offending_text:?}"
        );
    }
}

#[test]
fn bundled_meeting_scheduler_is_a_compile_pass_program() {
    // Catches semantic rules that reject the required governed-action path.
    let path = repository_root().join("examples/meeting-scheduler/main.aster");
    let text = fs::read_to_string(&path).expect("example is readable UTF-8");
    let source = SourceFile::new(path.display().to_string(), text);

    let checked = check_source(&source).expect("meeting scheduler checks");

    assert_eq!(checked.module_name(), "meeting.scheduler");
    assert_eq!(checked.agent_names(), vec!["Scheduler"]);
}

#[test]
fn direct_allow_governed_write_is_a_compile_pass_program() {
    // Catches an authorization checker that incorrectly requires human approval.
    let path = repository_root().join("tests/conformance/pass/direct_allow.aster");
    let text = fs::read_to_string(&path).expect("fixture is readable UTF-8");
    let source = SourceFile::new(path.display().to_string(), text);

    let checked = check_source(&source).expect("direct-allow fixture checks");

    assert_eq!(checked.agent_names(), vec!["Writer"]);
}
