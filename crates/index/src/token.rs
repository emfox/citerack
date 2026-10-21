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
    fn terms_come_back_in_order_and_cover_the_words() {
        let text = "a study of something";
        let spans: Vec<&str> = terms(text).iter().map(|t| &text[t.start..t.end]).collect();
        assert_eq!(spans, vec!["a", "study", "of", "something"]);
    }
}
