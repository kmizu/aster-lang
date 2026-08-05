use aster_diagnostics::{Diagnostic, DiagnosticCode, Severity, Span, explain};

#[test]
fn span_reports_utf8_byte_offsets_with_one_based_line_and_column() {
    // Catches treating byte offsets as character indices or reporting zero-based positions.
    let source = "module example;\nlet café = 1;\n";
    let start = source.find("café").expect("literal is present");
    let end = start + "café".len();

    let span = Span::from_offsets("example.aster", source, start, end)
        .expect("offsets are valid UTF-8 boundaries");

    assert_eq!(span.file, "example.aster");
    assert_eq!(span.start, 20);
    assert_eq!(span.end, 25);
    assert_eq!(span.line, 2);
    assert_eq!(span.column, 5);
}

#[test]
fn invalid_span_offsets_return_a_typed_error() {
    // Catches accepting an offset in the middle of a UTF-8 code point.
    let source = "é";
    let error = Span::from_offsets("bad.aster", source, 1, 2)
        .expect_err("offset 1 splits the UTF-8 character");

    assert_eq!(error.to_string(), "span offsets are not UTF-8 boundaries");
}

#[test]
fn diagnostic_json_has_a_stable_schema_and_field_order() {
    // Catches nondeterministic map serialization or accidental schema drift.
    let span = Span::from_offsets("example.aster", "let x = y;\n", 8, 9)
        .expect("literal offsets are valid");
    let diagnostic = Diagnostic::error(
        DiagnosticCode::new("ASTER-NAME-1001").expect("registered code shape"),
        "unknown name `y`",
        span,
    )
    .with_label("not declared in this module")
    .with_note("declaration order does not affect lookup")
    .with_help("declare `y` before checking this module");

    let json = diagnostic.to_json().expect("diagnostic serializes");

    assert_eq!(
        json,
        concat!(
            "{\"code\":\"ASTER-NAME-1001\",\"severity\":\"error\",",
            "\"message\":\"unknown name `y`\",\"primary_span\":{",
            "\"file\":\"example.aster\",\"start\":8,\"end\":9,",
            "\"line\":1,\"column\":9},\"labels\":[",
            "\"not declared in this module\"],\"notes\":[",
            "\"declaration order does not affect lookup\"],",
            "\"help\":\"declare `y` before checking this module\"}"
        )
    );
}

#[test]
fn human_render_includes_location_excerpt_and_help() {
    // Catches diagnostics that lose the source location or actionable remediation.
    let source = "module example;\nlet x = candidate.value;\n";
    let start = source.find("candidate.value").expect("literal is present");
    let span = Span::from_offsets("candidate.aster", source, start, start + 15)
        .expect("literal offsets are valid");
    let diagnostic = Diagnostic::error(
        DiagnosticCode::new("ASTER-TYPE-2001").expect("registered code shape"),
        "candidate data must be validated before use",
        span,
    )
    .with_help("use `validate candidate with <Validator>` to obtain `Checked<T>`");

    let rendered = diagnostic.render_human(source);

    assert!(
        rendered
            .starts_with("error[ASTER-TYPE-2001]: candidate data must be validated before use\n")
    );
    assert!(rendered.contains(" --> candidate.aster:2:9\n"));
    assert!(rendered.contains("2 | let x = candidate.value;\n"));
    assert!(
        rendered
            .contains("help: use `validate candidate with <Validator>` to obtain `Checked<T>`\n")
    );
}

#[test]
fn diagnostic_registry_explains_known_codes_and_rejects_unknown_codes() {
    // Catches an `explain` command that silently invents or accepts undocumented codes.
    let known = explain("ASTER-TYPE-2001").expect("mandatory code is registered");
    assert_eq!(known.code.as_str(), "ASTER-TYPE-2001");
    assert_eq!(known.severity, Severity::Error);
    assert!(known.remediation.contains("validate candidate"));

    assert!(explain("ASTER-TYPE-2999").is_none());
}

#[test]
fn diagnostic_code_shape_supports_four_and_five_digit_registered_families() {
    // Catches rejecting the specified REPLAY-10xxx and BUDGET-11xxx families.
    assert!(DiagnosticCode::new("ASTER-TYPE-2001").is_ok());
    assert!(DiagnosticCode::new("ASTER-BUDGET-11001").is_ok());
    assert!(DiagnosticCode::new("ASTER-TYPE-201").is_err());
    assert!(DiagnosticCode::new("ASTER-BUDGET-110001").is_err());
}
