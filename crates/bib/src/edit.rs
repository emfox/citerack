//! One entry's byte layout, and how a rewrite reproduces every byte a caller did
//! not type. Nothing here models a field name: a span is bytes and a copy is a
//! copy.

use crate::library::Span;
use crate::model::{Delimiter, Entry, Field, FieldName};

/// Where one field sat in the entry's own text, so an edit replaces those bytes
/// and leaves the whitespace between fields alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    /// First byte of the field name.
    pub name: usize,
    /// The value, without its delimiters.
    pub value: Span,
    /// How the value was delimited.
    pub delimiter: Delimiter,
    /// One past the last byte of the value, closing delimiter included.
    pub after: usize,
}

/// The offsets inside an entry's text: the parts an edit has to keep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// First byte of the citation key.
    pub key: Span,
    /// One per field, in the same order as the entry's fields.
    pub fields: Vec<FieldLayout>,
    /// The closing delimiter: `}` for a braced entry, `)` for a parenthesised one.
    pub close: u8,
    /// The last byte of the closing delimiter, so a new field goes in front of it.
    pub end: usize,
}

impl Layout {
    /// Reads the layout of `text`, which is one entry starting at its `@`. A pair
    /// this reader cannot take apart is left out, so the value the parser kept is
    /// what a save writes for that field.
    pub fn of(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.first() != Some(&b'@') {
            return None;
        }
        let mut at = skip_space(bytes, 1);
        at = skip_while(bytes, at, is_name_byte)?;
        at = skip_space(bytes, at);
        let close = match bytes.get(at)? {
            b'{' => b'}',
            b'(' => b')',
            _ => return None,
        };
        at = skip_space(bytes, at + 1);
        // The key ends at the first byte that is punctuation or whitespace, which
        // is the rule the parser reads a header with.
        let key_start = at;
        at = skip_while(bytes, at, |b| !is_key_end(b))?;
        let key = Span {
            start: key_start,
            end: at,
        };

        let (fields, end) = Self::read_fields(bytes, at, close)?;
        Some(Self {
            key,
            fields,
            close,
            end,
        })
    }

    /// Walks the pairs after the key, each of which is a name, an `=` and a value
    /// that is either delimited or ends at a comma. Returns the fields and the
    /// offset of the closing delimiter.
    fn read_fields(bytes: &[u8], mut at: usize, close: u8) -> Option<(Vec<FieldLayout>, usize)> {
        let mut fields = Vec::new();
        loop {
            at = skip_space(bytes, at);
            match *bytes.get(at)? {
                b',' => {
                    at += 1;
                    continue;
                }
                byte if byte == close => return Some((fields, at)),
                b'@' => return None,
                _ => {}
            }
            let (field, after) = read_field(bytes, at, close)?;
            fields.push(field);
            at = after;
        }
    }

    /// The entry's text with `entry`'s field values put in place, keeping every
    /// byte between them, and adding a field the entry gained at the end.
    /// `None` when a value cannot be written inside its own delimiter.
    pub fn rebuild(&self, text: &str, entry: &Entry) -> Option<String> {
        if entry.fields.len() < self.fields.len() {
            return None;
        }
        let mut out = String::with_capacity(text.len());
        let mut copied = 0usize;
        out.push_str(text.get(copied..self.key.start)?);
        out.push_str(&entry.key);
        copied = self.key.end;
        for (field, at) in entry.fields.iter().zip(&self.fields) {
            if !survives(field) {
                return None;
            }
            out.push_str(text.get(copied..at.value.start)?);
            out.push_str(&field.value);
            // The closing delimiter is the file's, not this writer's.
            out.push_str(text.get(at.value.end..at.after)?);
            copied = at.after;
        }
        // A new field goes after the last one, which means inside the closing
        // delimiter: the indent is the file's own, and a comma goes before every
        // field this adds. A comma the file already had there is not written twice.
        let gap = text.get(copied..self.end)?;
        let indent = gap.rsplit_once('\n').map_or("  ", |(_, line)| line);
        let indent = if indent.is_empty() { "  " } else { indent };
        let added = &entry.fields[self.fields.len()..];
        let tail = if added.is_empty() {
            gap
        } else {
            gap.trim_start_matches([' ', '\t'])
                .strip_prefix(',')
                .unwrap_or(gap)
        };
        for field in added {
            if field.name_raw.is_empty() || field.name_raw.bytes().any(|b| !is_name_byte(b)) {
                return None;
            }
            if !survives(field) {
                return None;
            }
            out.push(',');
            out.push('\n');
            out.push_str(indent);
            out.push_str(field.name_raw.as_ref());
            out.push_str(" = ");
            push_value(&mut out, field);
        }
        if !added.is_empty() {
            // The closing delimiter expects a comma after the last field here.
            out.push(',');
        }
        out.push_str(tail);
        out.push_str(text.get(self.end..)?);
        Some(out)
    }
}

/// Writes one value the way its own delimiter holds it.
fn push_value(out: &mut String, field: &Field) {
    match field.delimiter {
        Delimiter::Braces => {
            out.push('{');
            out.push_str(&field.value);
            out.push('}');
        }
        Delimiter::Quotes => {
            out.push('"');
            out.push_str(&field.value);
            out.push('"');
        }
        Delimiter::Bare => out.push_str(&field.value),
    }
}

/// Reads one `name = value` pair that starts at `at`, and says where the value
/// sits and where the next pair could start.
fn read_field(bytes: &[u8], at: usize, close: u8) -> Option<(FieldLayout, usize)> {
    let start = skip_name_start(bytes, at);
    let name_end = skip_while(bytes, start, is_name_byte)?;
    if name_end == start {
        return None;
    }
    let equals = skip_space(bytes, name_end);
    if bytes.get(equals) != Some(&b'=') {
        return None;
    }
    let at = skip_space(bytes, equals + 1);
    let (value, delimiter, after) = match *bytes.get(at)? {
        b'{' => {
            let value = braced(bytes, at + 1)?;
            (value, Delimiter::Braces, value.end + 1)
        }
        b'"' => {
            let value = quoted(bytes, at + 1)?;
            (value, Delimiter::Quotes, value.end + 1)
        }
        _ => {
            let end = skip_while(bytes, at, |b| !is_bare_end(b, close))?;
            if end == at {
                return None;
            }
            (Span { start: at, end }, Delimiter::Bare, end)
        }
    };
    Some((
        FieldLayout {
            name: start,
            value,
            delimiter,
            after,
        },
        after,
    ))
}

/// The body of a braced value whose opening brace is behind `from`, ending at the
/// brace that closes it.
fn braced(bytes: &[u8], from: usize) -> Option<Span> {
    let mut depth = 1usize;
    let mut at = from;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(Span {
                        start: from,
                        end: at,
                    });
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// The body of a quoted value whose opening quote is behind `from`, where a brace
/// protects a quote inside it.
fn quoted(bytes: &[u8], from: usize) -> Option<Span> {
    let mut depth = 0usize;
    let mut at = from;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'"' if depth == 0 => {
                return Some(Span {
                    start: from,
                    end: at,
                });
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// The entry's text as it would be written from nothing but the model, for an
/// entry the source did not hold.
pub fn render(entry: &Entry) -> Option<String> {
    let kind = entry.kind.as_str();
    if kind.is_empty() || kind.bytes().any(|b| !is_name_byte(b)) {
        return None;
    }
    if entry.key.is_empty() || entry.key.bytes().any(is_key_end) {
        return None;
    }
    let mut out = String::new();
    out.push('@');
    out.push_str(kind);
    out.push('{');
    out.push_str(&entry.key);
    out.push(',');
    for field in &entry.fields {
        if field.name_raw.is_empty() || field.name_raw.bytes().any(|b| !is_name_byte(b)) {
            return None;
        }
        if !survives(field) {
            return None;
        }
        out.push('\n');
        out.push_str("  ");
        out.push_str(&field.name_raw);
        out.push_str(" = ");
        push_value(&mut out, field);
        out.push(',');
    }
    out.push_str("\n}");
    Some(out)
}

/// Whether a value can be written inside its own delimiter without escaping.
pub fn survives(field: &Field) -> bool {
    match field.delimiter {
        Delimiter::Braces => balanced(&field.value),
        Delimiter::Quotes => balanced(&field.value) && quotes_are_protected(&field.value),
        Delimiter::Bare => bare_holds(&field.value),
    }
}

/// Whether every closing brace in `value` closes one opened inside it, since an
/// unbalanced value would end its entry early.
fn balanced(value: &str) -> bool {
    let mut depth = 0i32;
    for byte in value.bytes() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Whether every quote in `value` sits inside braces, which is what keeps it from
/// closing a quoted value early.
fn quotes_are_protected(value: &str) -> bool {
    let mut depth = 0usize;
    for byte in value.bytes() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'"' if depth == 0 => return false,
            _ => {}
        }
    }
    true
}

/// Whether a value can stand without delimiters, which BibTeX allows only for a
/// number or a macro name.
fn bare_holds(value: &str) -> bool {
    !value.is_empty()
        && !value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || matches!(b, b',' | b'{' | b'}' | b'(' | b')'))
}

/// A field a caller could add: a name it can write and a value that survives.
pub fn usable(name: &str, value: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(is_name_byte)
        && balanced(value)
        && quotes_are_protected(value)
}

/// A new field for a name and a value, delimited so the value needs no escape.
pub fn new_field(name: &str, value: &str) -> Field {
    let digits = !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit());
    Field {
        name_raw: name.into(),
        name: FieldName::new(name),
        value: value.into(),
        delimiter: if digits {
            Delimiter::Bare
        } else {
            Delimiter::Braces
        },
    }
}

/// A byte that continues a field name or an entry type.
fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':' | b'+' | b'.' | b'/')
}

/// A byte that ends a citation key.
pub(crate) fn is_key_end(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b',' | b'}' | b')' | b'{' | b'(' | b'=' | b'"')
}

/// A byte that ends a bare value: whitespace, a comma, or the entry's own close.
fn is_bare_end(b: u8, close: u8) -> bool {
    b.is_ascii_whitespace() || b == b',' || b == close
}

/// Stops at the first byte `keep` rejects, or at the end of the text.
fn skip_while(bytes: &[u8], from: usize, keep: impl Fn(u8) -> bool) -> Option<usize> {
    let mut at = from;
    while matches!(bytes.get(at), Some(&b) if keep(b)) {
        at += 1;
    }
    Some(at.min(bytes.len()))
}

/// The offset of the first byte that is not a space, from `from` onward.
fn skip_space(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

/// The offset of the first byte that could start a field name, so the comma that
/// ended the previous value is stepped over.
fn skip_name_start(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while bytes
        .get(at)
        .is_some_and(|b| b.is_ascii_whitespace() || *b == b',')
    {
        at += 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "@Article{smith2024,\n  Author   = {Smith, John},\n  year = 2024,\n}\n";

    #[test]
    fn a_layout_reads_every_field_of_an_entry() {
        let layout = Layout::of(TEXT).expect("a layout");
        assert_eq!(layout.close, b'}');
        assert_eq!(&TEXT[layout.key.start..layout.key.end], "smith2024");
        assert_eq!(layout.fields.len(), 2);
        let first = &layout.fields[0];
        assert_eq!(&TEXT[first.value.start..first.value.end], "Smith, John");
        let second = &layout.fields[1];
        assert_eq!(&TEXT[second.value.start..second.value.end], "2024");
        assert_eq!(second.delimiter, Delimiter::Bare);
        assert_eq!(layout.end, TEXT.len() - 2, "the closing brace");
    }

    #[test]
    fn a_layout_reads_a_parenthesised_entry() {
        let text = "@book(doe2019,\n  editor = {Doe, Jane}\n)";
        let layout = Layout::of(text).expect("a layout");
        assert_eq!(layout.close, b')');
        assert_eq!(layout.fields.len(), 1);
        assert_eq!(
            &text[layout.fields[0].value.start..layout.fields[0].value.end],
            "Doe, Jane"
        );
    }

    #[test]
    fn a_layout_reads_an_entry_holding_no_field() {
        let text = "@misc{lonely}";
        let layout = Layout::of(text).expect("a layout");
        assert!(layout.fields.is_empty());
        assert_eq!(
            layout.end,
            text.len() - 1,
            "the closing brace is the last byte"
        );
    }

    #[test]
    fn a_rebuild_with_every_model_field_unchanged_is_the_same_text() {
        let layout = Layout::of(TEXT).expect("a layout");
        let entry = Entry {
            kind: crate::model::EntryKind::new("Article"),
            key: "smith2024".into(),
            fields: vec![
                new_field("Author", "Smith, John"),
                new_field("year", "2024"),
            ],
        };
        let mut entry = entry;
        entry.fields[0].delimiter = Delimiter::Braces;
        assert_eq!(layout.rebuild(TEXT, &entry).as_deref(), Some(TEXT));
    }

    #[test]
    fn a_rebuild_that_gains_a_field_puts_it_after_the_last_one() {
        let layout = Layout::of(TEXT).expect("a layout");
        let mut entry = Entry {
            kind: crate::model::EntryKind::new("Article"),
            key: "smith2024".into(),
            fields: vec![
                new_field("Author", "Smith, John"),
                new_field("year", "2024"),
            ],
        };
        entry.fields.push(new_field("doi", "10.1000/x"));
        let out = layout.rebuild(TEXT, &entry).expect("a rewrite");
        assert_eq!(
            out,
            "@Article{smith2024,\n  Author   = {Smith, John},\n  year = 2024,\n  doi = {10.1000/x},\n}\n"
        );
    }
}
