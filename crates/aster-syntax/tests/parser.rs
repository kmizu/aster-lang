use aster_syntax::{DeclarationKind, SourceFile, parse};

#[test]
fn parses_the_complete_meeting_scheduler_without_example_special_cases() {
    // Catches a parser that supports only isolated declarations or skips effect syntax.
    let source = SourceFile::new(
        "examples/meeting-scheduler/main.aster",
        include_str!("../../../examples/meeting-scheduler/main.aster"),
    );
    let module = parse(&source).expect("the normative example parses");

    assert_eq!(module.name.segments, vec!["meeting", "scheduler"]);
    assert_eq!(module.declarations.len(), 21);
    assert_eq!(
        module
            .declarations
            .iter()
            .map(|declaration| declaration.kind.category())
            .collect::<Vec<_>>(),
        vec![
            "type",
            "type",
            "type",
            "type",
            "type",
            "type",
            "type",
            "type",
            "capability",
            "capability",
            "capability",
            "capability",
            "prompt",
            "validator",
            "function",
            "tool",
            "tool",
            "tool",
            "validator",
            "policy",
            "agent",
        ]
    );
    assert!(matches!(
        module
            .declarations
            .last()
            .map(|declaration| &declaration.kind),
        Some(DeclarationKind::Agent(_))
    ));
}

#[test]
fn declaration_order_is_syntax_independent_and_spans_cover_each_declaration() {
    // Catches parser lookups that incorrectly require declarations before uses.
    let text = "module order; fn f(x: Later) -> Bool { return true; } type Later = Text;";
    let source = SourceFile::new("order.aster", text);
    let module = parse(&source).expect("syntax does not resolve names");

    assert_eq!(module.declarations.len(), 2);
    for declaration in &module.declarations {
        assert!(declaration.span.start < declaration.span.end);
        assert_eq!(
            &text[declaration.span.start..=declaration.span.start],
            match declaration.kind.category() {
                "function" => "f",
                "type" => "t",
                other => panic!("unexpected declaration category {other}"),
            }
        );
    }
}

#[test]
fn malformed_declaration_reports_the_unexpected_token_span() {
    // Catches generic whole-file errors that lose the actionable token location.
    let source = SourceFile::new("missing.aster", "module missing\ntype X = Text;");
    let diagnostics = parse(&source).expect_err("module semicolon is mandatory");

    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PARSE-0001");
    assert_eq!(diagnostics[0].primary_span.start, 15);
    assert_eq!(diagnostics[0].primary_span.end, 19);
    assert!(diagnostics[0].message.contains("expected `;`"));
}

#[test]
fn duplicate_tool_metadata_is_rejected_instead_of_overwritten() {
    let source = SourceFile::new(
        "duplicate-tool-metadata.aster",
        r"module duplicate;
tool A.one() -> Unit { mode read; mode write; }
tool A.two() -> Unit { capability C(); capability C(); }
tool A.three() -> Unit { sensitivity public; sensitivity private; }
tool A.four() -> Unit { risk reversible; risk irreversible; }
tool A.five() -> Unit { idempotency first; idempotency second; }",
    );
    let diagnostics = parse(&source).expect_err("duplicate metadata must fail parsing");

    assert_eq!(diagnostics.len(), 5);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() == "ASTER-PARSE-0001")
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("duplicate tool metadata"))
    );
}

#[test]
fn parser_recovers_at_declaration_boundaries_and_reports_errors_in_source_order() {
    // Catches fail-fast parsing that hides independent errors from one check run.
    let source = SourceFile::new(
        "many-errors.aster",
        "module bad; type A = ; type B = { x Text }; capability C(x Text);",
    );
    let diagnostics = parse(&source).expect_err("three declarations are malformed");

    assert_eq!(diagnostics.len(), 3);
    assert!(
        diagnostics
            .windows(2)
            .all(|pair| { pair[0].primary_span.start < pair[1].primary_span.start })
    );
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.primary_span.start)
            .collect::<Vec<_>>(),
        vec![21, 36, 59]
    );
}

#[test]
fn ast_json_is_machine_readable_and_deterministic() {
    // Catches debug-string output or maps whose order changes across runs.
    let source = SourceFile::new("tiny.aster", "module tiny; type Name = Text;");
    let module = parse(&source).expect("tiny module parses");

    let first = module.to_json().expect("AST serializes");
    let second = module.to_json().expect("AST serializes repeatedly");
    assert_eq!(first, second);
    let json: serde_json::Value = serde_json::from_str(&first).expect("valid JSON");
    assert_eq!(json["name"]["segments"][0], "tiny");
    assert_eq!(json["declarations"][0]["kind"], "type");
}

#[test]
fn parses_flow_enum_if_and_match_without_recursive_or_dynamic_syntax() {
    // Catches gaps outside the scheduler's concrete expression subset.
    let source = SourceFile::new(
        "control.aster",
        concat!(
            "module control;",
            "enum Choice { First, Other(Text), }",
            "capability ModelUse(alias: Text);",
            "flow choose(x: Option<Text>) -> Result<Text, Error> ",
            "uses [ModelUse(\"planner\")] {",
            "return if true { return match x { Some(v) => v, None => \"none\", }; } ",
            "else { return -(1 + 2); };",
            "}",
        ),
    );

    let module = parse(&source).expect("finite control expressions parse");
    assert_eq!(module.declarations.len(), 3);
    assert!(matches!(
        module.declarations[2].kind,
        DeclarationKind::Flow(_)
    ));
}

#[test]
fn prompt_instruction_must_be_a_static_block_string_at_parse_time() {
    // Catches accepting an expression that could promote runtime data to instructions.
    let source = SourceFile::new(
        "dynamic-prompt.aster",
        "module p; prompt P(x: Text) -> Text { instruction x; data { x, }; }",
    );
    let diagnostics = parse(&source).expect_err("dynamic instruction syntax is invalid");

    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PROMPT-7001");
    assert_eq!(diagnostics[0].primary_span.start, 50);
}

#[test]
fn empty_record_declarations_are_rejected_to_keep_block_syntax_unambiguous() {
    // Catches a parser/spec mismatch around `if value {}` and empty record construction.
    let source = SourceFile::new("empty.aster", "module empty; type Empty = {};");
    let diagnostics = parse(&source).expect_err("0.1 records require a field");

    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PARSE-0001");
    assert_eq!(diagnostics[0].primary_span.start, 28);
}

#[test]
fn empty_enums_are_rejected_because_matchable_types_need_a_variant() {
    // Catches accepting an uninhabited user enum that 0.1 match analysis cannot totalize.
    let source = SourceFile::new("empty-enum.aster", "module empty; enum Empty {}");
    let diagnostics = parse(&source).expect_err("0.1 enums require a variant");

    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PARSE-0001");
    assert_eq!(diagnostics[0].primary_span.start, 26);
}
