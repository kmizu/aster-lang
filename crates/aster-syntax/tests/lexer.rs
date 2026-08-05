use aster_syntax::{Keyword, SourceFile, Symbol, TokenKind, lex};

fn significant_kinds(source: &str) -> Vec<TokenKind> {
    let source = SourceFile::new("lexer.aster", source);
    lex(&source)
        .expect("fixture lexes")
        .tokens
        .into_iter()
        .filter_map(|token| (!token.kind.is_trivia()).then_some(token.kind))
        .collect()
}

#[test]
fn lexes_keywords_paths_aliases_literals_and_longest_operators() {
    // Catches keyword/name confusion, split dotted paths, and short operator matching.
    let kinds = significant_kinds(
        r#"module meeting.scheduler;
           infer ParseMeeting(message = "hi\nthere") using @planner;
           require count <= 120 && count != 0;
           let delta = -42;
        "#,
    );

    assert_eq!(
        kinds,
        vec![
            TokenKind::Keyword(Keyword::Module),
            TokenKind::Identifier("meeting".into()),
            TokenKind::Symbol(Symbol::Dot),
            TokenKind::Identifier("scheduler".into()),
            TokenKind::Symbol(Symbol::Semicolon),
            TokenKind::Keyword(Keyword::Infer),
            TokenKind::Identifier("ParseMeeting".into()),
            TokenKind::Symbol(Symbol::LeftParen),
            TokenKind::Identifier("message".into()),
            TokenKind::Symbol(Symbol::Equal),
            TokenKind::String("hi\nthere".into()),
            TokenKind::Symbol(Symbol::RightParen),
            TokenKind::Keyword(Keyword::Using),
            TokenKind::ModelAlias("planner".into()),
            TokenKind::Symbol(Symbol::Semicolon),
            TokenKind::Keyword(Keyword::Require),
            TokenKind::Identifier("count".into()),
            TokenKind::Symbol(Symbol::LessEqual),
            TokenKind::Integer(120),
            TokenKind::Symbol(Symbol::AndAnd),
            TokenKind::Identifier("count".into()),
            TokenKind::Symbol(Symbol::BangEqual),
            TokenKind::Integer(0),
            TokenKind::Symbol(Symbol::Semicolon),
            TokenKind::Keyword(Keyword::Let),
            TokenKind::Identifier("delta".into()),
            TokenKind::Symbol(Symbol::Equal),
            TokenKind::Symbol(Symbol::Minus),
            TokenKind::Integer(42),
            TokenKind::Symbol(Symbol::Semicolon),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn preserves_line_and_nested_block_comments_as_trivia() {
    // Catches dropping comments or ending a nested block comment too early.
    let source = SourceFile::new(
        "comments.aster",
        "// before\nmodule x; /* outer /* nested */ still outer */\n",
    );
    let lexed = lex(&source).expect("nested comments are supported");
    let comments: Vec<_> = lexed
        .tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::LineComment(text) | TokenKind::BlockComment(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(
        comments,
        vec!["// before", "/* outer /* nested */ still outer */"]
    );
}

#[test]
fn block_string_keeps_semantic_contents_and_span() {
    // Catches escape processing or indentation rewriting inside prompt instructions.
    let source = SourceFile::new(
        "prompt.aster",
        "instruction \"\"\"\n  Keep \\n literally.\n\"\"\";",
    );
    let lexed = lex(&source).expect("block string lexes");
    let block = lexed
        .tokens
        .iter()
        .find(|token| matches!(token.kind, TokenKind::BlockString(_)))
        .expect("block token exists");

    assert_eq!(
        block.kind,
        TokenKind::BlockString("\n  Keep \\n literally.\n".into())
    );
    assert_eq!(
        &source.text()[block.span.start..block.span.end],
        "\"\"\"\n  Keep \\n literally.\n\"\"\""
    );
}

#[test]
fn malformed_tokens_return_ordered_stable_diagnostics_with_exact_spans() {
    // Catches silently accepting invalid escapes, unknown tokens, or unterminated comments.
    let source = SourceFile::new("bad.aster", "\"bad\\q\" § /* open");
    let diagnostics = lex(&source).expect_err("fixture contains three lexical errors");

    let actual: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.as_str(),
                diagnostic.primary_span.start,
                diagnostic.primary_span.end,
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            ("ASTER-PARSE-0002", 4, 6),
            ("ASTER-PARSE-0003", 8, 10),
            ("ASTER-PARSE-0005", 11, 18),
        ]
    );
}

#[test]
fn integer_overflow_is_a_diagnostic_instead_of_wrapping_or_panicking() {
    // Catches unchecked decimal parsing at the source boundary.
    let source = SourceFile::new("integer.aster", "9223372036854775808");
    let diagnostics = lex(&source).expect_err("value exceeds Int range");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_str(), "ASTER-PARSE-0004");
    assert_eq!(diagnostics[0].primary_span.start, 0);
    assert_eq!(diagnostics[0].primary_span.end, 19);
}
