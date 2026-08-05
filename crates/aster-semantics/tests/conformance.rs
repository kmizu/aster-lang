use std::{fs, path::PathBuf};

use aster_semantics::check_source;
use aster_syntax::SourceFile;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
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
