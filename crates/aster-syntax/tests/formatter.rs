use aster_syntax::{SourceFile, format_source, parse};

#[test]
fn canonical_format_is_idempotent_and_preserves_normalized_ast() {
    // Catches whitespace-only rewriting that leaves multiple canonical layouts.
    let input = "// hello\nmodule x;type R={a:Int,b:Text,};fn f(x:Int)->Bool{return x>0;}";
    let expected = concat!(
        "// hello\n",
        "module x;\n\n",
        "type R = {\n",
        "  a: Int,\n",
        "  b: Text,\n",
        "};\n\n",
        "fn f(x: Int) -> Bool {\n",
        "  return (x > 0);\n",
        "}\n",
    );
    let source = SourceFile::new("format.aster", input);
    let before = parse(&source)
        .expect("input parses")
        .normalized_json()
        .expect("AST normalizes");

    let once = format_source(&source).expect("input formats");
    let twice = format_source(&SourceFile::new("format.aster", &once)).expect("output formats");
    let after = parse(&SourceFile::new("format.aster", &once))
        .expect("formatted source parses")
        .normalized_json()
        .expect("AST normalizes");

    assert_eq!(once, expected);
    assert_eq!(twice, once);
    assert_eq!(after, before);
}

#[test]
fn comments_survive_and_prompt_instruction_contents_are_byte_identical() {
    // Catches formatters that drop trivia or reinterpret static instructions.
    let input = concat!(
        "module p; /* keep me */\n",
        "prompt P(x:Untrusted<Text>)->Text{instruction \"\"\"\n",
        "  Keep \\n literally.  \n",
        "\"\"\";data{x,};}",
    );
    let source = SourceFile::new("prompt.aster", input);
    let formatted = format_source(&source).expect("prompt formats");

    assert!(formatted.starts_with("module p;\n\n/* keep me */\nprompt P"));
    assert!(formatted.contains("\"\"\"\n  Keep \\n literally.  \n\"\"\""));
    assert_eq!(formatted.matches("/* keep me */").count(), 1);
}

#[test]
fn malformed_source_is_diagnosed_instead_of_partially_formatted() {
    // Catches a formatter that silently drops the malformed suffix.
    let source = SourceFile::new("bad.aster", "module bad; type X = {");
    let diagnostics = format_source(&source).expect_err("malformed source is rejected");

    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PARSE-0001");
}

#[test]
fn meeting_example_format_round_trip_preserves_its_normalized_ast() {
    // Catches formatter omissions in the full governed-action syntax.
    let source = SourceFile::new(
        "examples/meeting-scheduler/main.aster",
        include_str!("../../../examples/meeting-scheduler/main.aster"),
    );
    let before = parse(&source)
        .expect("example parses")
        .normalized_json()
        .expect("AST normalizes");
    let formatted = format_source(&source).expect("example formats");
    let after = parse(&SourceFile::new(source.path(), &formatted))
        .expect("formatted example parses")
        .normalized_json()
        .expect("AST normalizes");

    assert_eq!(after, before);
    assert_eq!(formatted, source.text());
}
