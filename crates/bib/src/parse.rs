//! A single-pass parser over bytes, so a 80 MB file never becomes 80 MB of chars.

use crate::error::{ParseError, ParseErrorKind};
use crate::library::{Library, Span};
use crate::model::{Delimiter, Entry, EntryKind, Field, FieldName};

/// Knobs a caller can turn; every one defaults to the strict choice.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Keep going after a malformed entry instead of returning the first error.
    pub recover: bool,
}

/// Parses one BibTeX file in a single pass; a regular entry contributes an
/// [`Entry`] and a span, and any other block contributes only a span, so a
/// rewrite puts it back unchanged.
pub fn parse_str(input: &str, options: &ParseOptions) -> Result<Library, ParseError> {
    let bytes = input.as_bytes();
    let mut p = Parser {
        bytes,
        pos: 0,
        recover: options.recover,
        entries: Vec::new(),
        spans: Vec::new(),
        skipped: Vec::new(),
        bad: 0,
    };
    p.run()?;
    Ok(Library::new(
        input.to_owned(),
        p.entries,
        p.spans,
        p.skipped,
    ))
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
    recover: bool,
    entries: Vec<Entry>,
    spans: Vec<Span>,
    skipped: Vec<Span>,
    bad: usize,
}

/// One raw field before names are normalised: the slice plus where it sat.
struct RawField {
    name: Span,
    value: Span,
    delimiter: Delimiter,
}

impl<'a> Parser<'a> {
    fn run(&mut self) -> Result<(), ParseError> {
        while let Some(at) = self.next_at() {
            self.pos = at;
            match self.parse_item() {
                Ok(()) => {}
                Err(err) if self.recover => {
                    self.bad += 1;
                    self.pos = self.recover_to_next_at(at);
                    let _ = err;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(())
    }

    /// Moves to the next `@` that starts a line item, or returns `None` at end of input.
    fn next_at(&self) -> Option<usize> {
        memchr::memchr(b'@', &self.bytes[self.pos..]).map(|i| i + self.pos)
    }

    /// Skips to the next `@` after `from`, so a broken entry cannot eat the rest.
    fn recover_to_next_at(&self, from: usize) -> usize {
        memchr::memchr(b'@', &self.bytes[from + 1..])
            .map(|i| i + from + 1)
            .unwrap_or(self.bytes.len())
    }

    fn parse_item(&mut self) -> Result<(), ParseError> {
        let start = self.pos;
        self.pos += 1;
        self.skip_ws();
        let kind_span = self
            .take_ident()
            .ok_or_else(|| self.err_at(ParseErrorKind::MalformedHeader, start))?;
        self.skip_ws();
        let open = *self
            .bytes
            .get(self.pos)
            .ok_or_else(|| self.err_at(ParseErrorKind::MalformedHeader, start))?;
        let close = match open {
            b'{' => b'}',
            b'(' => b')',
            _ => return Err(self.err_at(ParseErrorKind::MalformedHeader, self.pos)),
        };
        self.pos += 1;
        let kind_text = self.text(kind_span);

        if !EntryKind::new(kind_text).is_regular() {
            let end = self.skip_balanced(close)?;
            self.skipped.push(Span { start, end });
            return Ok(());
        }

        self.skip_ws();
        let key_span = self.take_key();
        let key = self.text(key_span);

        let mut raw = Vec::new();
        loop {
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(&b) if b == close => {
                    self.pos += 1;
                    break;
                }
                Some(&b',') => {
                    self.pos += 1;
                }
                // A value that ran past its entry ends at whatever follows;
                // a new `@` there means this entry swallowed the next one.
                Some(&b'@') => return Err(self.err_at(ParseErrorKind::UnterminatedValue, self.pos)),
                Some(_) => raw.push(self.parse_field(close)?),
                None => return Err(self.err_at(ParseErrorKind::UnterminatedEntry, start)),
            }
        }

        let mut fields = Vec::with_capacity(raw.len());
        for f in raw {
            let name_text = self.text(f.name);
            fields.push(Field {
                name_raw: name_text.into(),
                name: FieldName::new(name_text),
                value: self.text(f.value).into(),
                delimiter: f.delimiter,
            });
        }

        self.entries.push(Entry {
            kind: EntryKind::new(kind_text),
            key: key.into(),
            fields,
        });
        self.spans.push(Span {
            start,
            end: self.pos,
        });
        Ok(())
    }

    fn parse_field(&mut self, entry_close: u8) -> Result<RawField, ParseError> {
        let name = self
            .take_ident()
            .ok_or_else(|| self.err_at(ParseErrorKind::ExpectedFieldName, self.pos))?;
        self.skip_ws();
        if self.bytes.get(self.pos) != Some(&b'=') {
            return Err(self.err_at(ParseErrorKind::ExpectedEquals, self.pos));
        }
        self.pos += 1;
        self.skip_ws();
        let (value, delimiter) = self.take_value(entry_close)?;
        Ok(RawField {
            name,
            value,
            delimiter,
        })
    }

    /// Reads one field value, returning its inner span and how it was delimited.
    fn take_value(&mut self, entry_close: u8) -> Result<(Span, Delimiter), ParseError> {
        match self.bytes.get(self.pos) {
            Some(&b'{') => {
                let start = self.pos + 1;
                self.pos += 1;
                let end = self.skip_braced_value(start)?;
                Ok((Span { start, end }, Delimiter::Braces))
            }
            Some(&b'"') => {
                let start = self.pos + 1;
                self.pos += 1;
                let end = self.skip_quoted_value(start)?;
                Ok((Span { start, end }, Delimiter::Quotes))
            }
            Some(_) => {
                let start = self.pos;
                while let Some(&b) = self.bytes.get(self.pos) {
                    if b == b',' || b == entry_close || b.is_ascii_whitespace() {
                        break;
                    }
                    self.pos += 1;
                }
                Ok((
                    Span {
                        start,
                        end: self.pos,
                    },
                    Delimiter::Bare,
                ))
            }
            None => Err(self.err_at(ParseErrorKind::UnterminatedValue, self.pos)),
        }
    }

    /// Consumes a braced value body; returns the offset of the closing brace.
    fn skip_braced_value(&mut self, start: usize) -> Result<usize, ParseError> {
        let mut depth = 1usize;
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                // A brace must not be balanced by the entry's own closing brace,
                // or an unclosed value silently swallows the rest of the file.
                b'{' => depth += 1,
                b'}' if depth == 1 => {
                    let end = self.pos;
                    self.pos += 1;
                    return Ok(end);
                }
                b'}' => depth -= 1,
                _ => {}
            }
            self.pos += 1;
        }
        Err(self.err_at(ParseErrorKind::UnterminatedValue, start.saturating_sub(1)))
    }

    /// Consumes a quoted value body; braces inside it protect a quote.
    fn skip_quoted_value(&mut self, start: usize) -> Result<usize, ParseError> {
        let mut depth = 0usize;
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b'{' => depth += 1,
                b'}' => depth = depth.saturating_sub(1),
                b'"' if depth == 0 => {
                    let end = self.pos;
                    self.pos += 1;
                    return Ok(end);
                }
                _ => {}
            }
            self.pos += 1;
        }
        Err(self.err_at(ParseErrorKind::UnterminatedValue, start.saturating_sub(1)))
    }

    /// Consumes to the closing delimiter of a non-entry block, counting nesting.
    fn skip_balanced(&mut self, close: u8) -> Result<usize, ParseError> {
        let open = if close == b'}' { b'{' } else { b'(' };
        let mut depth = 1usize;
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    self.pos += 1;
                    return Ok(self.pos);
                }
            }
            self.pos += 1;
        }
        Err(self.err_at(ParseErrorKind::UnterminatedBlock, self.pos))
    }

    fn take_ident(&mut self) -> Option<Span> {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':' | b'+' | b'.' | b'/') {
                self.pos += 1;
            } else {
                break;
            }
        }
        (self.pos > start).then_some(Span {
            start,
            end: self.pos,
        })
    }

    fn take_key(&mut self) -> Span {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == b',' || b == b'}' || b == b')' || b.is_ascii_whitespace() {
                break;
            }
            self.pos += 1;
        }
        Span {
            start,
            end: self.pos,
        }
    }

    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn text(&self, span: Span) -> &'a str {
        // Safe: every span boundary follows an ASCII delimiter, so it is a char boundary.
        std::str::from_utf8(&self.bytes[span.start..span.end]).unwrap_or_default()
    }

    fn err_at(&self, kind: ParseErrorKind, offset: usize) -> ParseError {
        let offset = offset.min(self.bytes.len());
        let line = 1 + memchr::memchr_iter(b'\n', &self.bytes[..offset]).count();
        ParseError { kind, offset, line }
    }
}
