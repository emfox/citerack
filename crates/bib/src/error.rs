//! Parse failures, each carrying the byte offset it was found at.

use std::fmt;

/// What went wrong, without the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// A byte that cannot start a top-level item was found where one was expected.
    UnexpectedByte,
    /// `@` was followed by something that is not an ident, or by no `{`/`(`.
    MalformedHeader,
    /// A braced or quoted value ran to the end of the input.
    UnterminatedValue,
    /// An entry was opened and the input ended before its closing brace.
    UnterminatedEntry,
    /// A field name was expected and something else was found.
    ExpectedFieldName,
    /// A field name was found without the `=` that must follow it.
    ExpectedEquals,
    /// A `@comment`, `@preamble` or other free-form block ran to the end of input.
    UnterminatedBlock,
}

impl ParseErrorKind {
    /// A short English phrase for this kind, used in `Display`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnexpectedByte => "unexpected byte",
            Self::MalformedHeader => "malformed entry header",
            Self::UnterminatedValue => "unterminated value",
            Self::UnterminatedEntry => "unterminated entry",
            Self::ExpectedFieldName => "expected a field name",
            Self::ExpectedEquals => "expected '='",
            Self::UnterminatedBlock => "unterminated block",
        }
    }
}

/// A parse failure with the offset that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// What went wrong.
    pub kind: ParseErrorKind,
    /// Byte offset into the input where the parser stopped.
    pub offset: usize,
    /// 1-based line number of `offset`, for an editor to jump to.
    pub line: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at byte {}, line {}",
            self.kind.as_str(),
            self.offset,
            self.line
        )
    }
}

impl std::error::Error for ParseError {}
