//! What each entry type needs, for a pane that answers a reader's question rather
//! than only echoing the file. The table is BibTeX's own modest one: the fields
//! without which an entry of that type says nothing about a work.

use crate::model::{Entry, EntryKind};

/// One field an entry type asks for, and what else would satisfy it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// The name as BibTeX spells it, lower case.
    pub name: &'static str,
    /// Whether an entry of this type is incomplete without it.
    pub required: bool,
    /// Names that stand in for this one, empty for a field that stands alone.
    pub either: &'static [&'static str],
}

/// A field an entry type wants, required or merely usual.
const fn wants(name: &'static str, required: bool) -> Wanted {
    Wanted {
        name,
        required,
        either: &[],
    }
}

/// The fields an entry type asks for, in the order a reader fills them in, or an
/// empty list for a type this table does not know.
pub fn wanted(kind: &EntryKind) -> &'static [Wanted] {
    if !kind.is_regular() {
        return &[];
    }
    // The names are matched as they stand: `@Article`, `@ARTICLE` and `@article`
    // are one type, so folding the case here is what makes the table reach them.
    match kind.as_str().to_ascii_lowercase().as_str() {
        "article" => ARTICLE,
        "book" => BOOK,
        "inbook" | "incollection" => INCOLLECTION,
        "inproceedings" | "conference" => INPROCEEDINGS,
        "phdthesis" | "mastersthesis" => THESIS,
        "techreport" => TECHREPORT,
        // A manual or a proceedings describes itself the way a book does, where a
        // report is the one type whose "where" is an institution, not a publisher.
        "manual" => MANUAL,
        "proceedings" => PROCEEDINGS,
        "unpublished" => UNPUBLISHED,
        "misc" | "online" => MISC,
        _ => NO_FIELDS,
    }
}

/// Whether an entry already answers this field, by itself or through a stand-in.
/// A value of spaces counts as empty, since it reads as nothing on the page.
pub fn satisfied(entry: &Entry, wanted: &Wanted) -> bool {
    let names = std::iter::once(wanted.name).chain(wanted.either.iter().copied());
    names
        .filter_map(|name| entry.field(name))
        .any(|value| !value.trim().is_empty())
}

/// Whether a field name is the one this type calls required.
pub fn is_required(kind: &EntryKind, name: &str) -> bool {
    wanted(kind)
        .iter()
        .any(|field| field.required && field.name.eq_ignore_ascii_case(name))
}

const NO_FIELDS: &[Wanted] = &[];

const ARTICLE: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("journal", true),
    wants("year", true),
    wants("volume", false),
    wants("number", false),
    wants("pages", false),
    wants("doi", false),
];

/// A book is satisfied by an author or by an editor, which is the one place this
/// table says "either": an anthology nobody wrote is still a book.
const BOOK: &[Wanted] = &[
    Wanted {
        name: "author",
        required: true,
        either: &["editor"],
    },
    Wanted {
        name: "editor",
        required: true,
        either: &["author"],
    },
    wants("title", true),
    wants("publisher", true),
    wants("year", true),
    wants("volume", false),
    wants("series", false),
    wants("edition", false),
    wants("doi", false),
];

const INCOLLECTION: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("booktitle", true),
    wants("publisher", true),
    wants("year", true),
    wants("editor", false),
    wants("chapter", false),
    wants("pages", false),
    wants("doi", false),
];

const INPROCEEDINGS: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("booktitle", true),
    wants("year", true),
    wants("editor", false),
    wants("pages", false),
    wants("organization", false),
    wants("publisher", false),
    wants("address", false),
    wants("doi", false),
];

/// Either kind of thesis: the degree is the type's own name, the school is not.
const THESIS: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("school", true),
    wants("year", true),
    wants("address", false),
    wants("doi", false),
];

const TECHREPORT: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("institution", true),
    wants("year", true),
    wants("number", false),
    wants("address", false),
    wants("doi", false),
];

const MANUAL: &[Wanted] = &[
    wants("title", true),
    wants("author", false),
    wants("organization", false),
    wants("address", false),
    wants("edition", false),
    wants("year", false),
];

const PROCEEDINGS: &[Wanted] = &[
    wants("title", true),
    wants("year", true),
    wants("editor", false),
    wants("publisher", false),
    wants("organization", false),
    wants("volume", false),
    wants("series", false),
];

const UNPUBLISHED: &[Wanted] = &[
    wants("author", true),
    wants("title", true),
    wants("note", true),
    wants("month", false),
    wants("year", false),
];

/// `@misc` is the type a file reaches for when nothing else fits, so it asks for
/// nothing and suggests everything.
const MISC: &[Wanted] = &[
    wants("author", false),
    wants("title", false),
    wants("howpublished", false),
    wants("year", false),
    wants("note", false),
];

#[cfg(test)]
mod tests {
    use super::*;

    const ARTICLE_TEXT: &str = "@Article{smith2024,\n  Author = {Smith, John},\n  Year = 2024,\n}";

    fn entry(text: &str) -> Entry {
        let library = crate::parse_str(text, &crate::ParseOptions::default()).expect("parses");
        library.entries()[0].clone()
    }

    fn required(kind: &str) -> Vec<&'static str> {
        wanted(&EntryKind::new(kind))
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name)
            .collect()
    }

    #[test]
    fn an_article_asks_for_four_fields() {
        assert_eq!(required("article"), ["author", "title", "journal", "year"]);
    }

    #[test]
    fn the_table_reads_the_type_in_any_case() {
        assert_eq!(required("Article"), required("article"));
        assert_eq!(required("INPROCEEDINGS"), required("inproceedings"));
    }

    #[test]
    fn a_book_is_satisfied_by_an_editor_as_well_as_an_author() {
        let book = entry("@book{doe2019,\n  editor = {Doe, Jane},\n}");
        let people: Vec<&Wanted> = wanted(&book.kind)
            .iter()
            .filter(|field| field.name == "author")
            .collect();
        assert!(
            satisfied(&book, people[0]),
            "an editor stands in for an author"
        );
    }

    #[test]
    fn a_present_but_blank_field_does_not_satisfy_a_type() {
        let thin = entry(ARTICLE_TEXT);
        let journal = wanted(&thin.kind)
            .iter()
            .find(|field| field.name == "journal")
            .expect("an article wants a journal");
        assert!(!satisfied(&thin, journal), "the field is absent");
        let blank = entry("@article{a,\n  journal = {  },\n}");
        assert!(!satisfied(&blank, journal), "spaces read as nothing");
    }

    #[test]
    fn a_type_the_table_does_not_know_requires_nothing() {
        let made_up = entry("@software{tool2024,\n  title = {A Tool},\n}");
        assert!(wanted(&made_up.kind).is_empty());
        for special in ["comment", "preamble", "string"] {
            assert!(
                wanted(&EntryKind::new(special)).is_empty(),
                "{special} is not a regular entry"
            );
        }
    }

    #[test]
    fn a_required_name_is_recognised_in_the_case_the_file_used() {
        let article = EntryKind::new("Article");
        assert!(is_required(&article, "Journal"));
        assert!(!is_required(&article, "volume"), "volume is usual only");
    }
}
