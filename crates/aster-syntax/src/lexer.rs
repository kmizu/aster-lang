use aster_diagnostics::{Diagnostic, KnownDiagnosticCode, Span};

use crate::{Keyword, SourceFile, Symbol, Token, TokenKind};

/// Complete token stream, including comments, whitespace, and EOF.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lexed {
    /// Tokens in byte order.
    pub tokens: Vec<Token>,
}

/// Lexes one UTF-8 source file and returns all lexical diagnostics in byte order.
///
/// # Errors
///
/// Returns every recoverable invalid escape, token, integer, or unterminated
/// construct. A stream is returned only when no lexical error occurred.
pub fn lex(source: &SourceFile) -> Result<Lexed, Vec<Diagnostic>> {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a SourceFile,
    bytes: &'a [u8],
    cursor: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a SourceFile) -> Self {
        Self {
            source,
            bytes: source.text().as_bytes(),
            cursor: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Lexed, Vec<Diagnostic>> {
        while self.cursor < self.bytes.len() {
            let start = self.cursor;
            let byte = self.bytes[self.cursor];
            match byte {
                b if b.is_ascii_whitespace() => self.lex_whitespace(),
                b'/' if self.starts_with(b"//") => self.lex_line_comment(),
                b'/' if self.starts_with(b"/*") => self.lex_block_comment(),
                b'"' if self.starts_with(b"\"\"\"") => self.lex_block_string(),
                b'"' => self.lex_string(),
                b'@' => self.lex_model_alias(),
                b if is_identifier_start(b) => self.lex_identifier(),
                b if b.is_ascii_digit() => self.lex_integer(),
                _ => {
                    if !self.lex_symbol() {
                        let width = self.source.text()[start..]
                            .chars()
                            .next()
                            .map_or(1, char::len_utf8);
                        self.cursor += width;
                        self.push_error(
                            KnownDiagnosticCode::UnknownToken,
                            "unknown source token",
                            start,
                            self.cursor,
                            "remove this character or replace it with ASTER punctuation",
                        );
                    }
                }
            }
        }
        self.push_token(TokenKind::Eof, self.cursor, self.cursor);
        if self.diagnostics.is_empty() {
            Ok(Lexed {
                tokens: self.tokens,
            })
        } else {
            self.diagnostics.sort_by_key(|diagnostic| {
                (
                    diagnostic.primary_span.start,
                    diagnostic.primary_span.end,
                    diagnostic.code.as_str().to_owned(),
                )
            });
            Err(self.diagnostics)
        }
    }

    fn starts_with(&self, needle: &[u8]) -> bool {
        self.bytes[self.cursor..].starts_with(needle)
    }

    fn lex_whitespace(&mut self) {
        let start = self.cursor;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.cursor += 1;
        }
        self.push_source_text(TokenTextKind::Whitespace, start, self.cursor);
    }

    fn lex_line_comment(&mut self) {
        let start = self.cursor;
        self.cursor += 2;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| *byte != b'\n')
        {
            self.cursor += 1;
        }
        self.push_source_text(TokenTextKind::LineComment, start, self.cursor);
    }

    fn lex_block_comment(&mut self) {
        let start = self.cursor;
        self.cursor += 2;
        let mut depth = 1_u32;
        while self.cursor < self.bytes.len() {
            if self.starts_with(b"/*") {
                depth += 1;
                self.cursor += 2;
            } else if self.starts_with(b"*/") {
                depth -= 1;
                self.cursor += 2;
                if depth == 0 {
                    self.push_source_text(TokenTextKind::BlockComment, start, self.cursor);
                    return;
                }
            } else {
                self.cursor += self.next_char_width();
            }
        }
        self.push_error(
            KnownDiagnosticCode::UnterminatedBlockComment,
            "unterminated block comment",
            start,
            self.cursor,
            "close every nested `/*` comment with `*/`",
        );
    }

    fn lex_block_string(&mut self) {
        let start = self.cursor;
        self.cursor += 3;
        let content_start = self.cursor;
        while self.cursor < self.bytes.len() && !self.starts_with(b"\"\"\"") {
            self.cursor += self.next_char_width();
        }
        if self.cursor == self.bytes.len() {
            self.push_error(
                KnownDiagnosticCode::UnterminatedBlockString,
                "unterminated block string",
                start,
                self.cursor,
                "add a closing triple quote",
            );
            return;
        }
        let content = self.source.text()[content_start..self.cursor].to_owned();
        self.cursor += 3;
        self.push_token(TokenKind::BlockString(content), start, self.cursor);
    }

    fn lex_string(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        let mut valid = true;
        let mut terminated = false;
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'"' => {
                    self.cursor += 1;
                    terminated = true;
                    break;
                }
                b'\\' => {
                    let escape_start = self.cursor;
                    self.cursor += 1;
                    if self.cursor >= self.bytes.len() {
                        break;
                    }
                    match self.bytes[self.cursor] {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                            self.cursor += 1;
                        }
                        b'u' => {
                            self.cursor += 1;
                            let digits_end = self.cursor.saturating_add(4);
                            if digits_end <= self.bytes.len()
                                && self.bytes[self.cursor..digits_end]
                                    .iter()
                                    .all(u8::is_ascii_hexdigit)
                            {
                                self.cursor = digits_end;
                            } else {
                                valid = false;
                                while self.cursor < digits_end.min(self.bytes.len())
                                    && self.bytes[self.cursor].is_ascii_alphanumeric()
                                {
                                    self.cursor += 1;
                                }
                                self.push_invalid_escape(escape_start, self.cursor);
                            }
                        }
                        _ => {
                            valid = false;
                            self.cursor += self.next_char_width();
                            self.push_invalid_escape(escape_start, self.cursor);
                        }
                    }
                }
                b'\n' | b'\r' | 0x00..=0x1f => {
                    valid = false;
                    let error_start = self.cursor;
                    self.cursor += 1;
                    self.push_error(
                        KnownDiagnosticCode::InvalidStringEscape,
                        "unescaped control character in string",
                        error_start,
                        self.cursor,
                        "escape control characters in JSON-style strings",
                    );
                }
                _ => self.cursor += self.next_char_width(),
            }
        }
        if !terminated {
            self.push_error(
                KnownDiagnosticCode::UnterminatedString,
                "unterminated string literal",
                start,
                self.cursor,
                "add a closing quote on the same source line",
            );
            return;
        }
        if valid {
            let literal = &self.source.text()[start..self.cursor];
            match serde_json::from_str::<String>(literal) {
                Ok(value) => self.push_token(TokenKind::String(value), start, self.cursor),
                Err(_) => self.push_error(
                    KnownDiagnosticCode::InvalidStringEscape,
                    "invalid JSON-style string literal",
                    start,
                    self.cursor,
                    "use JSON-compatible string escapes",
                ),
            }
        }
    }

    fn push_invalid_escape(&mut self, start: usize, end: usize) {
        self.push_error(
            KnownDiagnosticCode::InvalidStringEscape,
            "invalid string escape",
            start,
            end,
            "use a valid JSON-style escape",
        );
    }

    fn lex_model_alias(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        let name_start = self.cursor;
        if self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| is_identifier_start(*byte))
        {
            self.cursor += 1;
            while self
                .bytes
                .get(self.cursor)
                .is_some_and(|byte| is_identifier_continue(*byte))
            {
                self.cursor += 1;
            }
            let alias = self.source.text()[name_start..self.cursor].to_owned();
            self.push_token(TokenKind::ModelAlias(alias), start, self.cursor);
        } else {
            self.push_error(
                KnownDiagnosticCode::UnknownToken,
                "model alias requires an identifier after `@`",
                start,
                self.cursor,
                "write an alias such as `@planner`",
            );
        }
    }

    fn lex_identifier(&mut self) {
        let start = self.cursor;
        self.cursor += 1;
        while self
            .bytes
            .get(self.cursor)
            .is_some_and(|byte| is_identifier_continue(*byte))
        {
            self.cursor += 1;
        }
        let identifier = &self.source.text()[start..self.cursor];
        let kind = Keyword::from_identifier(identifier).map_or_else(
            || TokenKind::Identifier(identifier.to_owned()),
            TokenKind::Keyword,
        );
        self.push_token(kind, start, self.cursor);
    }

    fn lex_integer(&mut self) {
        let start = self.cursor;
        while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
            self.cursor += 1;
        }
        let literal = &self.source.text()[start..self.cursor];
        match literal.parse::<i64>() {
            Ok(value) => self.push_token(TokenKind::Integer(value), start, self.cursor),
            Err(_) => self.push_error(
                KnownDiagnosticCode::InvalidInteger,
                "integer literal is outside the ASTER `Int` range",
                start,
                self.cursor,
                "use a signed 64-bit integer magnitude",
            ),
        }
    }

    fn lex_symbol(&mut self) -> bool {
        let start = self.cursor;
        let pair = [
            (b"->".as_slice(), Symbol::Arrow),
            (b"=>".as_slice(), Symbol::FatArrow),
            (b"==".as_slice(), Symbol::EqualEqual),
            (b"!=".as_slice(), Symbol::BangEqual),
            (b"<=".as_slice(), Symbol::LessEqual),
            (b">=".as_slice(), Symbol::GreaterEqual),
            (b"&&".as_slice(), Symbol::AndAnd),
            (b"||".as_slice(), Symbol::OrOr),
        ];
        for (spelling, symbol) in pair {
            if self.starts_with(spelling) {
                self.cursor += spelling.len();
                self.push_token(TokenKind::Symbol(symbol), start, self.cursor);
                return true;
            }
        }
        let symbol = match self.bytes[self.cursor] {
            b'(' => Symbol::LeftParen,
            b')' => Symbol::RightParen,
            b'{' => Symbol::LeftBrace,
            b'}' => Symbol::RightBrace,
            b'[' => Symbol::LeftBracket,
            b']' => Symbol::RightBracket,
            b',' => Symbol::Comma,
            b':' => Symbol::Colon,
            b';' => Symbol::Semicolon,
            b'.' => Symbol::Dot,
            b'?' => Symbol::Question,
            b'+' => Symbol::Plus,
            b'-' => Symbol::Minus,
            b'*' => Symbol::Star,
            b'/' => Symbol::Slash,
            b'!' => Symbol::Bang,
            b'=' => Symbol::Equal,
            b'<' => Symbol::Less,
            b'>' => Symbol::Greater,
            _ => return false,
        };
        self.cursor += 1;
        self.push_token(TokenKind::Symbol(symbol), start, self.cursor);
        true
    }

    fn next_char_width(&self) -> usize {
        self.source.text()[self.cursor..]
            .chars()
            .next()
            .map_or(1, char::len_utf8)
    }

    fn push_source_text(&mut self, kind: TokenTextKind, start: usize, end: usize) {
        let text = self.source.text()[start..end].to_owned();
        let token = match kind {
            TokenTextKind::Whitespace => TokenKind::Whitespace(text),
            TokenTextKind::LineComment => TokenKind::LineComment(text),
            TokenTextKind::BlockComment => TokenKind::BlockComment(text),
        };
        self.push_token(token, start, end);
    }

    fn push_token(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, end),
        });
    }

    fn push_error(
        &mut self,
        code: KnownDiagnosticCode,
        message: &str,
        start: usize,
        end: usize,
        help: &str,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code.into(), message, self.span(start, end)).with_help(help));
    }

    fn span(&self, start: usize, end: usize) -> Span {
        match Span::from_offsets(self.source.path(), self.source.text(), start, end) {
            Ok(span) => span,
            Err(_) => Span {
                file: self.source.path().to_owned(),
                start: start.min(self.source.text().len()),
                end: end.min(self.source.text().len()),
                line: 1,
                column: 1,
            },
        }
    }
}

#[derive(Clone, Copy)]
enum TokenTextKind {
    Whitespace,
    LineComment,
    BlockComment,
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}
