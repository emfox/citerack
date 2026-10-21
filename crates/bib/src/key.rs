//! A citation key read out of the entry it names: a pattern of literal text and
//! `{field}` references, so the reader who wants `smith2024` and the reader who
//! wants `Smith:2024` both get what they asked for.

use crate::library::{EntryId, Library};

/// A key pattern: the text to write, and the fields to write into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// Literal text and field references, in the order they were written.
    parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// Text written as it stands.
    Text(String),
    /// The value of a field, shortened and folded as the pattern asked.
    Field {
        /// The field name, lower case.
        name: String,
        /// How many words of it to keep.
        words: usize,
        /// How many letters to keep of those words.
        letters: usize,
        /// Which case to write them in.
        case: Case,
    },
}

/// The case a field reference is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// `Smith`
    AsIs,
    /// `smith`
    Lower,
    /// `SMITH`
    Upper,
}

/// The pattern a library uses unless the user says otherwise: the first author's
/// surname and the year, which is the key most people already write by hand.
pub const DEFAULT: &str = "{author}{year}";

/// Why a pattern cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    /// No `{field}` at all, so every entry would be named the same thing.
    NoField,
    /// A `{` that is never closed.
    Unclosed,
    /// A `}` with nothing open.
    Stray,
    /// `{}` with no name in it.
    Empty,
    /// Literal text that cannot stand in a citation key.
    BadText,
}

impl PatternError {
    /// A short English phrase for a front end to put beside the box.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoField => "a pattern needs a field, written {author} or {year}",
            Self::Unclosed => "a { is never closed",
            Self::Stray => "a } closes nothing",
            Self::Empty => "{} names no field",
            Self::BadText => "a key holds no braces, commas or spaces outside the fields",
        }
    }
}

impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for PatternError {}

impl Pattern {
    /// Reads a pattern, or says why it cannot be one. A field is `{name}`, or
    /// carries a form after a colon: `{author:2}` keeps two words, `{author:1:3}`
    /// keeps three letters of one word, and `{year:l}` sets the case.
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut rest = pattern;
        while let Some(at) = rest.find(['{', '}']) {
            text.push_str(&rest[..at]);
            let after = &rest[at + 1..];
            if rest.as_bytes()[at] == b'}' {
                return Err(PatternError::Stray);
            }
            let Some(end) = after.find('}') else {
                return Err(PatternError::Unclosed);
            };
            let inner = &after[..end];
            if inner.trim().is_empty() {
                return Err(PatternError::Empty);
            }
            if !text.is_empty() {
                parts.push(Part::Text(std::mem::take(&mut text)));
            }
            parts.push(Part::Field {
                name: String::from(""),
                words: 1,
                letters: 0,
                case: Case::AsIs,
            });
            let field = parts.len() - 1;
            let (name, words, letters, case) = field_of(inner);
            parts[field] = Part::Field {
                name,
                words,
                letters,
                case,
            };
            rest = &after[end + 1..];
        }
        text.push_str(rest);
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        if !parts.iter().any(|part| matches!(part, Part::Field { .. })) {
            return Err(PatternError::NoField);
        }
        if let Some(Part::Text(text)) = parts
            .iter()
            .find(|part| matches!(part, Part::Text(text) if text.bytes().any(crate::edit::is_key_end)))
            .map(|part| match part {
                Part::Text(text) => Part::Text(text.clone()),
                _ => unreachable!(),
            })
        {
            if text.bytes().any(crate::edit::is_key_end) {
                return Err(PatternError::BadText);
            }
        }
        Ok(Self { parts })
    }

    /// The pattern as it was written, for a settings box to show.
    pub fn as_str(&self) -> String {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Field {
                    name,
                    words,
                    letters,
                    case,
                } => {
                    out.push('{');
                    out.push_str(name);
                    if *case != Case::AsIs {
                        out.push(':');
                        out.push(match case {
                            Case::Lower => 'l',
                            Case::AsIs => 'a',
                            Case::Upper => 'u',
                        });
                    }
                    if *letters > 0 {
                        out.push(':');
                        out.push_str(&letters.to_string());
                    }
                    if *words > 1 {
                        out.push(':');
                        out.push_str(&words.to_string());
                    }
                    out.push('}');
                }
            }
        }
        out
    }

    /// The key this pattern makes for one entry, before any clash is settled.
    pub fn key_of(&self, library: &Library, id: EntryId) -> String {
        let Some(entry) = library.entry(id) else {
            return String::new();
        };
        let mut out = String::new();
        for part in &self.parts {
            match part {
                Part::Text(text) => out.push_str(text),
                Part::Field {
                    name,
                    words,
                    letters,
                    case,
                } => {
                    let value = entry.field(name).unwrap_or("");
                    let taken = shorten(value, *words, *letters);
                    out.push_str(&match case {
                        Case::AsIs => taken,
                        Case::Lower => taken.to_lowercase(),
                        Case::Upper => taken.to_uppercase(),
                    });
                }
            }
        }
        out
    }
}

/// A prefix, a field reference and a suffix. Separate from [`Pattern`] only so a
/// caller can read the parts of the shorthand without writing a whole pattern.
fn field_of(inner: &str) -> (String, usize, usize, Case) {
    let mut pieces = inner.split(':');
    let name = pieces.next().unwrap_or("").trim().to_ascii_lowercase();
    let mut words = 1;
    let mut letters = 0;
    let mut case = Case::AsIs;
    let mut first_number = true;
    for piece in pieces {
        let piece = piece.trim();
        match piece {
            "l" => case = Case::Lower,
            "u" => case = Case::Upper,
            _ => {
                // The numbers read left to right: how many words, then how many
                // letters of each, so `{author:3:1}` is three initials.
                let Ok(number) = piece.parse::<usize>() else {
                    continue;
                };
                if first_number {
                    words = number.max(1);
                    first_number = false;
                } else {
                    letters = number;
                }
            }
        }
    }
    (name, words.max(1), letters, case)
}

/// The first `words` words of `value`, cut to `letters` letters each; zero letters
/// keeps a word whole. One word stands on its own, and more than one is joined by
/// dashes so a key never carries the space a comma would have left behind.
fn shorten(value: &str, words: usize, letters: usize) -> String {
    let plain = plain(value);
    let kept: Vec<String> = plain
        .split_whitespace()
        .take(words)
        .map(|word| cut(word, letters))
        .filter(|word| !word.is_empty())
        .collect();
    kept.join("-")
}

/// One word of a field, with only its letters and digits, cut to `letters`.
fn cut(word: &str, letters: usize) -> String {
    let mut out = String::new();
    for ch in word.chars().filter(|ch| ch.is_alphanumeric()) {
        if letters > 0 && out.chars().count() == letters {
            break;
        }
        out.push(ch);
    }
    out
}

/// A field value without the markup a key cannot carry: the command of an accent,
/// the braces that protect a capital, and the punctuation a surname does not need.
fn plain(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => while chars.next().is_some_and(|next| next.is_ascii_alphabetic()) {},
            '{' | '}' => {}
            ',' | ';' | '(' | ')' | '.' | '\'' | '"' | '/' | '&' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

/// The key an entry gets when the one the pattern makes is already taken: the
/// same key with `a`, `b`, `c`… appended, which is the lettering BibTeX users
/// already read as "the second of these".
pub fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    // A pattern that named nothing for this entry still has to give it a key, and
    // "entry" is a word a reader can search for to find what was left unnamed.
    let base = if base.is_empty() {
        String::from("entry")
    } else {
        base.to_owned()
    };
    (b'a'..=b'z')
        .map(|letter| format!("{base}{}", letter as char))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| {
            (2..)
                .map(|n| format!("{base}{n}"))
                .find(|candidate| !taken(candidate))
                .unwrap_or_else(|| base.to_owned())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseOptions, parse_str};

    /// One library with an author whose surname carries an accent, and a year.
    fn library() -> Library {
        let text = "@Article{a,\n  author = {Erd{\\H o}s, Paul and Smith, John},\n  title = {On Graphs},\n  year = 1959,\n}";
        parse_str(text, &ParseOptions::default()).expect("parses")
    }

    fn key(pattern: &str) -> String {
        let library = library();
        let pattern = Pattern::parse(pattern).expect("a pattern");
        pattern.key_of(&library, EntryId(0))
    }

    #[test]
    fn the_default_pattern_is_the_first_surname_and_the_year() {
        assert_eq!(key(DEFAULT), "Erdos1959");
    }

    #[test]
    fn a_field_can_be_shortened_and_case_folded() {
        assert_eq!(key("{author:1:3}{year}"), "Erd1959");
        assert_eq!(key("{author:2}{year}"), "Erdos-Paul1959");
        assert_eq!(key("{author:l}:{year}"), "erdos:1959");
        assert_eq!(key("{author:u}_{title:l}"), "ERDOS_on");
        assert_eq!(key("{author:3:1}{year}"), "E-P-a1959", "three initials");
    }

    #[test]
    fn a_field_the_entry_lacks_writes_nothing() {
        assert_eq!(key("{journal}{year}"), "1959");
    }

    #[test]
    fn a_pattern_without_a_field_is_refused() {
        assert_eq!(Pattern::parse("smith"), Err(PatternError::NoField));
        assert_eq!(Pattern::parse("{},  x"), Err(PatternError::Empty));
        assert_eq!(Pattern::parse("{author"), Err(PatternError::Unclosed));
        assert_eq!(Pattern::parse("author}"), Err(PatternError::Stray));
    }

    #[test]
    fn a_key_that_is_taken_is_given_a_letter() {
        let taken = |key: &str| ["smith2024", "smith2024a"].contains(&key);
        assert_eq!(unique("smith2024b", taken), "smith2024b");
        assert_eq!(unique("smith2024", taken), "smith2024b");
    }

    #[test]
    fn the_pattern_reads_back_as_it_was_written() {
        for text in [
            DEFAULT,
            "{author:2:3:l}{year}",
            "{author}-{year}",
            "k{title}",
        ] {
            let pattern = Pattern::parse(text).expect("a pattern");
            Pattern::parse(&pattern.as_str()).expect("a pattern again");
            assert!(!pattern.as_str().is_empty());
        }
    }

    #[test]
    fn a_literal_space_is_not_a_key() {
        assert_eq!(
            Pattern::parse("{author} {year}"),
            Err(PatternError::BadText)
        );
    }
}
