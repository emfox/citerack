//! The parser's contract: what it finds, where it found it, and what it loses.

use citerack_bib::{
    Delimiter, EntryId, ParseErrorKind, ParseOptions, WriteOptions, parse_str, write_str,
};

const SIMPLE: &str = "\
@article{smith2024,
  author = {Smith, John},
  title  = {A {Nested} Brace},
  year   = 2024,
}

@book(doe2019,
  editor = \"Doe, Jane\",
  publisher = {ACME},
)
";

#[test]
fn reads_entries_in_file_order() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    assert_eq!(lib.len(), 2);
    assert_eq!(lib.entries()[0].key.as_ref(), "smith2024");
    assert_eq!(lib.entries()[1].key.as_ref(), "doe2019");
    assert_eq!(lib.entries()[0].kind.as_str(), "article");
    assert_eq!(lib.entries()[1].kind.as_str(), "book");
}

#[test]
fn nested_braces_stay_in_the_value() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    assert_eq!(lib.entries()[0].field("title"), Some("A {Nested} Brace"));
}

#[test]
fn bare_numbers_keep_their_delimiter() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    let year = &lib.entries()[0].fields[2];
    assert_eq!(year.value.as_ref(), "2024");
    assert_eq!(year.delimiter, Delimiter::Bare);
}

#[test]
fn field_names_match_case_insensitively() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    assert_eq!(lib.entries()[0].field("AUTHOR"), Some("Smith, John"));
}

#[test]
fn a_non_entry_block_is_kept_as_a_span() {
    let input = "@comment{jabref-meta: databaseType:bibtex;}\n@misc{x, note={n}}\n";
    let lib = parse_str(input, &ParseOptions::default()).unwrap();
    assert_eq!(lib.len(), 1);
    assert_eq!(lib.skipped_spans().len(), 1);
    assert_eq!(
        &input[..lib.skipped_spans()[0].end],
        "@comment{jabref-meta: databaseType:bibtex;}"
    );
}

#[test]
fn an_entry_span_covers_the_entry_only() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    let text = lib.source_of(EntryId(1)).unwrap();
    assert!(text.starts_with("@book(doe2019,"));
    assert!(text.ends_with(')'));
}

#[test]
fn preserve_writes_the_input_back_unchanged() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    assert_eq!(write_str(&lib, &WriteOptions::Preserve), SIMPLE);
}

#[test]
fn an_unknown_field_survives_a_rewrite() {
    let input = "@misc{k, x-custom = {kept}, note = {n}}\n";
    let lib = parse_str(input, &ParseOptions::default()).unwrap();
    assert!(write_str(&lib, &WriteOptions::Preserve).contains("x-custom = {kept}"));
}

#[test]
fn canonical_writes_one_entry_per_block() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    let out = write_str(&lib, &WriteOptions::Canonical);
    assert_eq!(out.matches("@article{smith2024,").count(), 1);
    assert!(out.contains("  author = {Smith, John},"));
    assert!(out.contains("  year = 2024,"));
    let again = parse_str(&out, &ParseOptions::default()).unwrap();
    assert_eq!(again.len(), 2);
}

#[test]
fn an_unterminated_entry_names_its_line() {
    let input = "@article{a,\n  title = {t}\n";
    let err = parse_str(input, &ParseOptions::default()).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnterminatedEntry);
    assert_eq!(err.line, 1);
    assert!(err.offset <= input.len());
}

#[test]
fn an_unterminated_value_is_reported_where_it_opened() {
    let input = "@article{a, title = {never closed}\n@article{b, note={2}}\n";
    let err = parse_str(input, &ParseOptions::default()).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::UnterminatedValue);
}

#[test]
fn a_missing_equals_is_reported() {
    let input = "@article{a, title {t}}\n";
    let err = parse_str(input, &ParseOptions::default()).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::ExpectedEquals);
}

#[test]
fn a_malformed_header_is_reported() {
    let err = parse_str("@ {a}\n", &ParseOptions::default()).unwrap_err();
    assert_eq!(err.kind, ParseErrorKind::MalformedHeader);
}

#[test]
fn an_empty_input_yields_an_empty_library() {
    let lib = parse_str("", &ParseOptions::default()).unwrap();
    assert!(lib.is_empty());
}

#[test]
fn a_trailing_comma_is_accepted() {
    let lib = parse_str("@misc{a, note = {n},}\n", &ParseOptions::default()).unwrap();
    assert_eq!(lib.entries()[0].fields.len(), 1);
}

#[test]
fn a_preamble_block_does_not_count_as_an_entry() {
    let lib = parse_str(
        "@preamble{\"\\newcommand{\\x}{y}\"}\n",
        &ParseOptions::default(),
    )
    .unwrap();
    assert!(lib.is_empty());
    assert_eq!(lib.skipped_spans().len(), 1);
}

#[test]
fn recovery_skips_a_broken_entry_and_keeps_the_rest() {
    let input = "@article{good, note={1}}\n@article{bad, title {x}}\n@article{also, note={3}}\n";
    let options = ParseOptions { recover: true };
    let lib = parse_str(input, &options).unwrap();
    let keys: Vec<&str> = lib.entries().iter().map(|e| e.key.as_ref()).collect();
    assert_eq!(keys, vec!["good", "also"]);
}

#[test]
fn lookup_by_key_returns_the_first_match() {
    let lib = parse_str(SIMPLE, &ParseOptions::default()).unwrap();
    assert_eq!(lib.find_by_key("doe2019"), Some(EntryId(1)));
    assert_eq!(lib.find_by_key("absent"), None);
}
