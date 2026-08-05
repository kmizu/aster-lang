#![forbid(unsafe_code)]

//! ASTER source text, syntax trees, parsing, and canonical formatting.

mod ast;
mod formatter;
mod lexer;
mod parser;
mod source;
mod token;

pub use ast::*;
pub use formatter::{format_module, format_source};
pub use lexer::{Lexed, lex};
pub use parser::parse;
pub use source::SourceFile;
pub use token::{Keyword, Symbol, Token, TokenKind};
