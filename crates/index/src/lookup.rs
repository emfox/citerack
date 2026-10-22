//! What a typed identifier names: a DOI or an arXiv id, read from the loose ways
//! people paste them. This is text and nothing else, so it is tested without a
//! network and a caller can refuse before a request is made.

/// One identifier, resolved to what each service calls it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identifier {
    /// A DOI, lowercased, with the `10.` prefix and the publisher's slash.
    Doi,
    /// An arXiv id with any version suffix kept, since a version is a paper too.
    Arxiv,
}

/// What a typed identifier names, normalised: a URL is read as readily as the bare
/// form, since copying a DOI out of a page rarely gives anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Which service the id belongs to.
    pub kind: Identifier,
    /// The id as that service spells it, lowercased.
    pub id: String,
}

/// Reads what a user typed, or says why it names nothing.
pub fn read(input: &str) -> Result<Found, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("type a DOI or an arXiv id".into());
    }
    let lowered = text.to_ascii_lowercase();
    let body = strip_prefixes(&lowered);
    if arxiv_body(body) {
        return Ok(Found {
            kind: Identifier::Arxiv,
            id: body.to_owned(),
        });
    }
    if doi_shape(body) {
        return Ok(Found {
            kind: Identifier::Doi,
            id: body.to_owned(),
        });
    }
    Err(format!("{text} is neither a DOI nor an arXiv id"))
}

/// Everything up to the first character that cannot continue an identifier.
fn trim_tail(text: &str) -> &str {
    text.split(['?', '#', ' ', '\t'])
        .next()
        .unwrap_or(text)
        .trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\''))
        .trim_end_matches(['.', ',', ';'])
}

/// Drops the URL a browser shows for each service, so one reader handles `doi.org`,
/// `dx.doi.org`, `arxiv.org/abs` and the `arXiv:` prefix alike.
fn strip_prefixes(text: &str) -> &str {
    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi.org/",
        "dx.doi.org/",
        "https://www.arxiv.org/abs/",
        "https://arxiv.org/abs/",
        "http://arxiv.org/abs/",
        "arxiv.org/abs/",
        "arxiv:",
        "doi:",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return trim_tail(rest);
        }
    }
    trim_tail(text)
}

/// The id an arXiv path names, which is either a new-style `2101.01234v2` or the
/// older `math.GT/0309136`.
fn arxiv_body(text: &str) -> bool {
    let head = match text.find('v') {
        Some(at) if at > 0 && text[at + 1..].bytes().all(|b| b.is_ascii_digit()) => &text[..at],
        _ => text,
    };
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit()) && !part.is_empty();
    let new_style = match head.split_once('.') {
        Some((before, after)) => digits(before) && before.len() == 4 && digits(after),
        None => false,
    };
    let old_style = match head.split_once('/') {
        Some((before, after)) => before.contains('.') && digits(after) && after.len() == 7,
        None => false,
    };
    new_style || old_style
}

/// Whether this is a DOI: `10.` then a registrant and a suffix, with no whitespace.
fn doi_shape(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("10.") else {
        return false;
    };
    let Some((registrant, suffix)) = rest.split_once('/') else {
        return false;
    };
    !registrant.is_empty()
        && !suffix.is_empty()
        && registrant.bytes().all(|b| b.is_ascii_digit())
        && !suffix.bytes().any(|b| b.is_ascii_whitespace())
}

/// A name the entry should be filed under, from a list the service returns, so an
/// entry is never keyed `a` for a paper four people wrote.
pub fn first_surname(family: &str) -> String {
    family
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doi(id: &str) -> Result<Found, String> {
        Ok(Found {
            kind: Identifier::Doi,
            id: id.into(),
        })
    }

    fn arxiv(id: &str) -> Result<Found, String> {
        Ok(Found {
            kind: Identifier::Arxiv,
            id: id.into(),
        })
    }

    #[test]
    fn reads_a_bare_doi() {
        assert_eq!(
            read("10.1145/3292500.3330701"),
            doi("10.1145/3292500.3330701")
        );
    }

    #[test]
    fn reads_a_doi_shown_as_a_link() {
        assert_eq!(
            read("https://doi.org/10.1038/nphys1170"),
            doi("10.1038/nphys1170")
        );
        assert_eq!(read("doi:10.1038/NPHYS1170"), doi("10.1038/nphys1170"));
        assert_eq!(
            read("  http://dx.doi.org/10.1038/nphys1170  "),
            doi("10.1038/nphys1170")
        );
    }

    #[test]
    fn reads_an_arxiv_id_in_both_shapes() {
        assert_eq!(read("2101.01234v2"), arxiv("2101.01234v2"));
        assert_eq!(read("arXiv:math.GT/0309136"), arxiv("math.gt/0309136"));
        assert_eq!(
            read("https://arxiv.org/abs/2101.01234"),
            arxiv("2101.01234")
        );
    }

    #[test]
    fn a_trailing_punctuation_mark_is_not_part_of_the_id() {
        assert_eq!(read("10.1038/nphys1170."), doi("10.1038/nphys1170"));
        assert_eq!(read("(10.1038/nphys1170)"), doi("10.1038/nphys1170"));
    }

    #[test]
    fn anything_that_names_nothing_is_refused_and_says_so() {
        for input in ["", "   ", "nature", "10.1038", "10./x", "doi:", "10."] {
            assert!(read(input).is_err(), "{input:?} should name nothing");
        }
        assert_eq!(
            read("nature").unwrap_err(),
            "nature is neither a DOI nor an arXiv id"
        );
    }

    #[test]
    fn a_surname_is_letters_only_and_lowercase() {
        assert_eq!(first_surname("van der Berg"), "vanderberg");
        assert_eq!(first_surname("O'Brien"), "obrien");
    }
}
