//! Splitting folded text into terms, so a term can be found by lookup. A term is a
//! run of ASCII letters, digits or `_`, or a run of non-ASCII: `zhou2024` is one
//! term, `fast.filters` is two, and a CJK phrase is one.

/// One term: the bytes of a word in the folded text, and where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Term {
    /// Byte offset of the term in the folded text.
    pub start: usize,
    /// One past the last byte of the term.
    pub end: usize,
}

/// The terms of a folded string, in order. Called once per field per entry while
/// the index is built, so it walks bytes and stops for a character only when a
/// multi-byte one forces it.
pub fn terms(text: &str) -> Vec<Term> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if bytes[i].is_ascii() {
            match bytes[i] {
                b if b.is_ascii_alphanumeric() || b == b'_' => {
                    while i < bytes.len() && is_word_byte(bytes[i]) {
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                    continue;
                }
            }
        } else {
            // A non-ASCII character starts a run that ends at the next ASCII
            // byte, which keeps a CJK phrase one term and needs no table.
            while i < bytes.len() && !bytes[i].is_ascii() {
                i += char_len(text, i);
            }
        }
        out.push(Term { start, end: i });
    }
    out
}

/// Whether a byte continues an ASCII word.
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Length in bytes of the character starting at `i`, which must be non-ASCII.
fn char_len(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(1, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<&str> {
        terms(text).iter().map(|t| &text[t.start..t.end]).collect()
    }

    #[test]
    fn splits_on_spaces_and_punctuation() {
        assert_eq!(words("smith, john"), vec!["smith", "john"]);
    }

    #[test]
    fn keeps_digits_with_letters() {
        assert_eq!(words("zhou2024"), vec!["zhou2024"]);
    }

    #[test]
    fn a_cjk_run_is_one_term() {
        assert_eq!(words("神经网络 2024"), vec!["神经网络", "2024"]);
    }

    #[test]
    fn every_script_is_split_on_what_separates_its_words() {
        // A script that writes without spaces keeps its run whole, and one that
        // writes with them is split by them, so a search is not English-only.
        // Cyrillic, Greek and Arabic, spelled as escapes so this file stays ASCII.
        let cases: [(&str, [&str; 2]); 3] = [
            (
                "\u{41d}\u{435}\u{439}\u{440}\u{43e}\u{43d}\u{43d}\u{44b}\u{435} \u{441}\u{435}\u{442}\u{438}",
                [
                    "\u{41d}\u{435}\u{439}\u{440}\u{43e}\u{43d}\u{43d}\u{44b}\u{435}",
                    "\u{441}\u{435}\u{442}\u{438}",
                ],
            ),
            (
                "\u{3bd}\u{3b5}\u{3c5}\u{3c1}\u{3c9}\u{3bd}\u{3b9}\u{3ba}\u{3ac} \u{3b4}\u{3af}\u{3ba}\u{3c4}\u{3c5}\u{3b1}",
                [
                    "\u{3bd}\u{3b5}\u{3c5}\u{3c1}\u{3c9}\u{3bd}\u{3b9}\u{3ba}\u{3ac}",
                    "\u{3b4}\u{3af}\u{3ba}\u{3c4}\u{3c5}\u{3b1}",
                ],
            ),
            (
                "\u{645}\u{62d}\u{645}\u{62f} \u{639}\u{644}\u{64a}",
                ["\u{645}\u{62d}\u{645}\u{62f}", "\u{639}\u{644}\u{64a}"],
            ),
        ];
        for (text, want) in cases {
            assert_eq!(words(text), want.to_vec());
        }
    }

    #[test]
    fn terms_come_back_in_order_and_cover_the_words() {
        let text = "a study of something";
        let spans: Vec<&str> = terms(text).iter().map(|t| &text[t.start..t.end]).collect();
        assert_eq!(spans, vec!["a", "study", "of", "something"]);
    }
}
