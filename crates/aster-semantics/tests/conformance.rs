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
fn write_idempotency_requires_a_deterministically_serializable_type() {
    let codes = diagnostic_codes(
        r#"module test;
capability StoreWrite(scope: Text);
tool Store.put(value: Candidate<Text>) -> Unit {
  mode write;
  capability StoreWrite("scope");
  sensitivity private;
  risk irreversible;
  idempotency value;
}"#,
    );

    assert_eq!(codes, ["ASTER-TYPE-2004", "ASTER-TYPE-2001"]);

    assert_checks(
        r#"module test;
capability StoreWrite(scope: Text);
tool Store.put(request_id: Int) -> Unit {
  mode write;
  capability StoreWrite("scope");
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}"#,
    );
}

#[test]
fn tool_boundaries_cannot_accept_opaque_candidates() {
    let direct = diagnostic_codes(
        r#"module test;
capability StoreWrite(scope: Text);
tool Store.put(value: Candidate<Text>, request_id: Text) -> Unit {
  mode write;
  capability StoreWrite("scope");
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}"#,
    );
    let nested = diagnostic_codes(
        r#"module test;
type Envelope = { value: Candidate<Text> };
capability StoreWrite(scope: Text);
tool Store.put(value: Envelope, request_id: Text) -> Unit {
  mode write;
  capability StoreWrite("scope");
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}"#,
    );

    assert_eq!(direct, ["ASTER-TYPE-2001"]);
    assert_eq!(nested, ["ASTER-TYPE-2001"]);
}

#[test]
fn json_boundaries_reject_privileged_wrapper_shapes() {
    let prompt_result = diagnostic_codes(
        r#"module test;
prompt Parse() -> Observation<Text> { instruction """parse"""; data {}; }"#,
    );
    let tool_result = diagnostic_codes(
        r#"module test;
capability Read(scope: Text);
tool Store.get() -> Checked<Text> { mode read; capability Read("store"); sensitivity private; }"#,
    );
    let persistent_wrapper = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state { value: Incoming<Text> = Unit; }
  budget per_event {}
  on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );
    let capability_wrapper =
        diagnostic_codes("module test; capability Invalid(value: Checked<Text>);");

    assert_eq!(prompt_result, ["ASTER-TYPE-2002"]);
    assert_eq!(tool_result, ["ASTER-TYPE-2002"]);
    assert_eq!(persistent_wrapper, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
    assert_eq!(capability_wrapper, ["ASTER-TYPE-2002"]);
}

#[test]
fn cyclic_type_aliases_are_rejected() {
    let codes = diagnostic_codes("module test; type First = Second; type Second = First;");
    let nested =
        diagnostic_codes("module test; type First = List<Second>; type Second = Option<First>;");

    assert_eq!(codes, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
    assert_eq!(nested, ["ASTER-TYPE-2002", "ASTER-TYPE-2002"]);
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
  on tick(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
  on tick(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );

    assert_eq!(codes, ["ASTER-NAME-1002", "ASTER-NAME-1002"]);
}

#[test]
fn event_handlers_accept_one_incoming_payload() {
    let arity = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state {}
  budget per_event {}
  on message(first: Incoming<Text>, second: Incoming<Text>) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );
    let wrapper = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state {}
  budget per_event {}
  on message(value: Text) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );

    assert_eq!(arity, ["ASTER-TYPE-2002"]);
    assert_eq!(wrapper, ["ASTER-TYPE-2002"]);
}

#[test]
fn agents_require_at_least_one_event_handler() {
    let codes = diagnostic_codes(
        "module test; agent Worker() requires [] { state {} budget per_event {} }",
    );

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
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
  on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
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
    let enum_payload = diagnostic_codes("module test; enum Bad { Value(Secret<Text>) }");
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
    assert_eq!(enum_payload, ["ASTER-SECRET-8002"]);
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
fn policy_otherwise_rule_is_unique_and_final() {
    let codes = diagnostic_codes(
        r#"module test;
capability StoreWrite(key: Text);
tool Store.put(key: Text) -> Unit {
  mode write;
  capability StoreWrite(key);
  sensitivity private;
  risk reversible;
  idempotency key;
}
policy Invalid(proposal: Proposal<Store.put>) {
  deny "first" otherwise;
  deny "unreachable" otherwise;
}"#,
    );

    assert_eq!(codes, ["ASTER-POLICY-4001"]);
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
fn builtins_and_enum_constructors_reject_named_arguments() {
    let builtin =
        diagnostic_codes("module test; fn bad() -> Option<Int> { return Some(value = 1); }");
    let constructor = diagnostic_codes(
        "module test; enum Choice { Pick(Int) } fn bad() -> Choice { return Choice.Pick(value = 1); }",
    );

    assert_eq!(builtin, ["ASTER-TYPE-2002"]);
    assert_eq!(constructor, ["ASTER-TYPE-2002"]);
}

#[test]
fn governance_action_symbols_cannot_have_type_arguments() {
    let codes = diagnostic_codes(
        r#"module test;
capability Write(scope: Text);
tool Store.put(request_id: Text) -> Unit {
  mode write;
  capability Write("store");
  sensitivity private;
  risk irreversible;
  idempotency request_id;
}
policy Direct(proposal: Proposal<Store.put<Text>>) { deny "no" otherwise; }"#,
    );

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
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
fn result_propagation_requires_a_result_returning_context() {
    let codes = diagnostic_codes(
        "module test; fn bad(value: Result<Int, Error>) -> Int { return value?; }",
    );

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
fn provenance_and_collection_search_require_supported_value_types() {
    let provenance = diagnostic_codes(
        "module test; fn bad(value: Int) -> ProvenanceRef { return provenance(value); }",
    );
    let contains = diagnostic_codes(
        "module test; fn bad(values: List<Candidate<Text>>, value: Candidate<Text>) -> Bool { return contains(values, value); }",
    );
    let subset = diagnostic_codes(
        "module test; fn bad(left: List<Candidate<Text>>, right: List<Candidate<Text>>) -> Bool { return subset(left, right); }",
    );

    assert_eq!(provenance, ["ASTER-TYPE-2002"]);
    assert_eq!(contains, ["ASTER-TYPE-2002"]);
    assert_eq!(subset, ["ASTER-TYPE-2002"]);

    assert_checks(
        "module test; fn source(value: Incoming<Text>) -> ProvenanceRef { return provenance(value); }",
    );
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
fn match_patterns_belong_to_the_scrutinee_type_and_wildcard_is_final() {
    let wrong_qualifier = diagnostic_codes(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  return match choice { Wrong.First => 0, Choice.Other(value) => value };
}",
    );
    let unreachable = diagnostic_codes(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  return match choice { _ => 0, Choice.First => 1 };
}",
    );
    let duplicate_wildcard = diagnostic_codes(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  return match choice { _ => 0, _ => 1 };
}",
    );

    assert_eq!(wrong_qualifier, ["ASTER-NAME-1001"]);
    assert_eq!(unreachable, ["ASTER-TYPE-2002"]);
    assert_eq!(duplicate_wildcard, ["ASTER-TYPE-2002"]);
}

#[test]
fn option_and_result_matches_bind_payloads_exactly() {
    assert_checks(
        r"module test;
fn from_option(value: Option<Int>) -> Int {
  return match value { None => 0, Some(number) => number };
}
fn from_result(value: Result<Int, Error>) -> Int {
  return match value { Ok(number) => number, Err(error) => 0 };
}",
    );

    let escaped = diagnostic_codes(
        r"module test;
enum Choice { First, Other(Int) }
fn value(choice: Choice) -> Int {
  let selected = match choice { Choice.First => 0, Choice.Other(payload) => payload };
  return payload;
}",
    );
    assert_eq!(escaped, ["ASTER-NAME-1001"]);
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
fn human_approval_principals_are_text() {
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
  approve by Human(1) otherwise;
}",
    );

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

#[test]
fn stateful_policies_match_the_authorizing_agent_context() {
    let wrong_agent = diagnostic_codes(
        r#"module test;
capability Write(scope: Text);
tool Store.put(request_id: Text) -> Unit {
  mode write;
  capability Write("store");
  sensitivity private;
  risk reversible;
  idempotency request_id;
}
agent Other() requires [] { state { enabled: Bool = true; } budget per_event {} on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); } }
policy Direct(proposal: Proposal<Store.put>, snapshot: Other.State) { allow when snapshot.enabled; deny "no" otherwise; }
agent Worker() requires [Write("store")] {
  state {}
  budget per_event { external_writes <= 1; }
  on message(msg: Incoming<Unit>) -> Result<Unit, Error> {
    let purpose = intent Save { actor = self; beneficiary = self; basis = [provenance(msg)]; expected = "saved"; expires_at = event.time; };
    let proposal = propose Store.put(event.id) for purpose;
    let permit = (authorize proposal using Direct)?;
    return Ok(Unit);
  }
}"#,
    );
    let stateful_flow = diagnostic_codes(
        r#"module test;
capability Write(scope: Text);
tool Store.put(request_id: Text) -> Unit {
  mode write;
  capability Write("store");
  sensitivity private;
  risk reversible;
  idempotency request_id;
}
agent Worker() requires [] { state { enabled: Bool = true; } budget per_event {} on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); } }
policy Direct(proposal: Proposal<Store.put>, snapshot: Worker.State) { allow when snapshot.enabled; deny "no" otherwise; }
flow save(id: Text, purpose: Intent<Save>) -> Result<Unit, Error> uses [Write("store")] {
  let proposal = propose Store.put(id) for purpose;
  let permit = (authorize proposal using Direct)?;
  return Ok(Unit);
}"#,
    );

    assert_eq!(wrong_agent, ["ASTER-TYPE-2002"]);
    assert_eq!(stateful_flow, ["ASTER-TYPE-2002"]);
}

#[test]
fn executable_bodies_require_an_explicit_return() {
    let codes = diagnostic_codes("module test; fn missing() -> Unit { Unit; }");

    assert_eq!(codes, ["ASTER-TYPE-2002"]);
}

#[test]
fn state_updates_are_handler_only_and_name_each_mutable_field_once() {
    let outside_handler = diagnostic_codes(
        "module test; fn bad() -> Unit { update state { missing = 1; } return Unit; }",
    );
    let fields = diagnostic_codes(
        r#"module test;
agent Worker(owner: Text) requires [] {
  state { count: Int = 0; }
  budget per_event {}
  on tick(value: Incoming<Unit>) -> Result<Unit, Error> {
    update state { count = 1; count = 2; missing = 3; owner = "changed"; }
    return Ok(Unit);
  }
}"#,
    );

    assert_eq!(outside_handler, ["ASTER-TYPE-2002"]);
    assert_eq!(
        fields,
        ["ASTER-NAME-1002", "ASTER-NAME-1001", "ASTER-NAME-1001"]
    );
}

#[test]
fn state_defaults_cannot_read_partially_initialized_self() {
    let codes = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state { value: Int = self.value; }
  budget per_event {}
  on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );

    assert_eq!(codes, ["ASTER-NAME-1001"]);
}

#[test]
fn metadata_expressions_reject_return_statements() {
    let codes = diagnostic_codes(
        r"module test;
agent Worker() requires [] {
  state { value: Unit = if true { return Unit; } else { Unit; }; }
  budget per_event {}
  on message(value: Incoming<Unit>) -> Result<Unit, Error> { return Ok(Unit); }
}",
    );

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
fn affine_values_move_through_calls_and_nested_containers() {
    let through_call = affine_prelude(
        r"fn consume(proposal: Proposal<Store.put>) -> Unit { return Unit; }
flow bad(proposal: Proposal<Store.put>) -> Unit uses [] {
  consume(proposal);
  let reused = proposal.args.id;
  return Unit;
}",
    );
    let nested = affine_prelude(
        r"flow bad(proposal: Proposal<Store.put>) -> Unit uses [] {
  let proposals = [proposal];
  let moved = proposals;
  let reused = proposals;
  return Unit;
}",
    );

    assert_eq!(diagnostic_codes(&through_call), ["ASTER-AFFINE-5002"]);
    assert_eq!(diagnostic_codes(&nested), ["ASTER-AFFINE-5002"]);
}

#[test]
fn affine_consumption_is_joined_across_match_arms() {
    let source = affine_prelude(
        r"enum Choice { Use, Skip }
fn consume(proposal: Proposal<Store.put>) -> Unit { return Unit; }
flow bad(
  choice: Choice,
  proposal: Proposal<Store.put>
) -> Unit uses [] {
  match choice {
    Choice.Use => consume(proposal),
    Choice.Skip => Unit
  };
  let reused = proposal.args.id;
  return Unit;
}",
    );

    assert_eq!(diagnostic_codes(&source), ["ASTER-AFFINE-5002"]);
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
