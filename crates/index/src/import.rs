//! What a lookup service answers, turned into an entry: Crossref's JSON for a DOI
//! and Atom XML for an arXiv id. Neither reader touches the network, so a malformed
//! answer is a test here rather than a crash in the window.

use citerack_bib::{Delimiter, Entry, EntryKind, Field, FieldName};
use serde_json::Value;

/// Reads a Crossref `message` object.
pub fn from_crossref(id: &str, message: &Value) -> Result<Entry, String> {
    let title = text(message.get("title")).ok_or("the record has no title")?;
    let mut fields = Vec::new();
    push(&mut fields, "title", &title);
    for author in authors(message.get("author")) {
        push_multiple(&mut fields, "author", &author);
    }
    if let Some(container) = text(message.get("container-title")) {
        push(&mut fields, "journal", &container);
    }
    if let Some(short) = text(message.get("short-container-title")) {
        push(&mut fields, "shortjournal", &short);
    }
    for (name, key) in [
        ("volume", "volume"),
        ("issue", "number"),
        ("page", "pages"),
        ("publisher", "publisher"),
        ("issn", "issn"),
        ("language", "language"),
        ("abstract", "abstract"),
    ] {
        if let Some(value) = text(message.get(name)) {
            push(&mut fields, key, &strip_tags(&value));
        }
    }
    if let Some(year) = issued_year(message) {
        push(&mut fields, "year", &year);
    }
    push(&mut fields, "doi", id);
    if let Some(url) = text(message.get("URL")) {
        push(&mut fields, "url", &url);
    }
    Ok(entry(kind_of(message), id, fields))
}

/// Reads an arXiv Atom feed entry, which carries one `<entry>` per version.
pub fn from_arxiv(id: &str, xml: &str) -> Result<Entry, String> {
    let block = between(xml, "<entry>", "</entry>").ok_or("the answer held no entry")?;
    let title = squashed(inside(block, "title").ok_or("the entry has no title")?);
    if title.is_empty() {
        return Err(String::from("the entry has an empty title"));
    }
    let mut fields = Vec::new();
    push(&mut fields, "title", &title);
    for name in inside_all(block, "name") {
        let name = squashed(name);
        if let Some(author) = as_bibtex_name(&name) {
            push_multiple(&mut fields, "author", &author);
        }
    }
    if let Some(published) = inside(block, "published").and_then(year_of) {
        push(&mut fields, "year", &published);
    }
    if let Some(doi) = inside_all(block, "arxiv:doi").into_iter().next() {
        let doi = squashed(doi);
        if !doi.is_empty() {
            push(&mut fields, "doi", &doi);
        }
    }
    let journal = inside(block, "arxiv:journal_ref").map(squashed);
    if let Some(journal) = journal.filter(|j| !j.is_empty()) {
        push(&mut fields, "journal", &journal);
    }
    push(&mut fields, "eprint", id);
    push(&mut fields, "archiveprefix", "arXiv");
    if let Some(category) =
        attribute_of(block, "arxiv:primary_category", "term").filter(|term| !term.is_empty())
    {
        push(&mut fields, "primaryclass", category);
    }
    let mut entry = entry("Article", id, fields);
    entry.kind = EntryKind::new("Misc");
    Ok(entry)
}

/// A record with the key the caller chose, so an import never has to rename one.
pub fn keyed(mut entry: Entry, key: &str) -> Entry {
    entry.key = key.into();
    entry
}

fn entry(kind: &str, key: &str, fields: Vec<Field>) -> Entry {
    Entry {
        kind: EntryKind::new(kind),
        key: key.into(),
        fields,
    }
}

/// The BibTeX type a Crossref `type` deserves, defaulting to what a library that
/// has never heard of the service would write.
fn kind_of(message: &Value) -> &'static str {
    match text(message.get("type")).as_deref() {
        Some("journal-article") | Some("posted-content") => "Article",
        Some("proceedings-article") => "InProceedings",
        Some("book") => "Book",
        Some("book-chapter") => "InCollection",
        Some("dissertation") => "PhdThesis",
        Some("report") => "TechReport",
        _ => "Misc",
    }
}

/// The year the DOI was issued, which Crossref gives as date parts rather than as
/// a field.
fn issued_year(message: &Value) -> Option<String> {
    let parts = message.get("issued")?.get("date-parts")?.as_array()?;
    let first = parts.first()?.as_array()?;
    Some(first.first()?.as_i64()?.to_string())
}

/// The first string in a value Crossref sometimes sends as an array.
fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()).filter(|t| !t.trim().is_empty()),
        Value::Array(items) => items.iter().find_map(|item| text(Some(item))),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// One `Family, Given` per author, in the order the service listed them.
fn authors(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(people)) = value else {
        return Vec::new();
    };
    people
        .iter()
        .filter_map(|person| {
            let family = text(person.get("family"));
            let given = text(person.get("given"));
            match (family, given) {
                (Some(family), Some(given)) => Some(format!("{family}, {given}")),
                (Some(family), None) => Some(family),
                (None, given) => text(person.get("name")).or(given),
            }
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// `Smith, John and Doe, Jane`, which is how BibTeX separates names.
fn push_multiple(fields: &mut Vec<Field>, name: &str, value: &str) {
    match fields
        .iter_mut()
        .find(|field| field.name.as_str() == name && field.name_raw.as_ref() == name)
    {
        Some(field) => {
            let joined = format!("{} and {}", field.value, value);
            field.value = joined.into();
        }
        None => push(fields, name, value),
    }
}

fn push(fields: &mut Vec<Field>, name: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    fields.push(Field {
        name_raw: name.into(),
        name: FieldName::new(name),
        value: value.into(),
        delimiter: Delimiter::Braces,
    });
}

/// An arXiv name, `Jane Doe`, as BibTeX writes it, `Doe, Jane`.
fn as_bibtex_name(name: &str) -> Option<String> {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    if words.len() < 2 {
        return words.pop().map(str::to_owned);
    }
    let family = words.pop()?;
    Some(format!("{family}, {}", words.join(" ")))
}

fn year_of(text: &str) -> Option<String> {
    let year: String = text.chars().take(4).collect();
    (year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit())).then_some(year)
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = text[start..].find(close)? + start;
    Some(&text[start..end])
}

fn inside<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let close = format!("</{tag}>");
    let open = text.find(&format!("<{tag}"))?;
    let body = text[open..].split_once('>')?.1;
    let end = body.find(&close)?;
    Some(&body[..end])
}

fn inside_all<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(found) = inside(rest, tag) {
        out.push(found);
        // Past this element, since the next one starts after it and a title can
        // hold the tag's own name.
        let past = rest.find(found).unwrap_or(0) + found.len();
        rest = rest.get(past..).unwrap_or("");
    }
    out
}

/// An attribute of a self-closing element such as `<arxiv:primary_category term="cs.LG"/>`,
/// which has no body for `inside` to return.
fn attribute_of<'a>(text: &'a str, tag: &str, name: &str) -> Option<&'a str> {
    let start = text.find(&format!("<{tag}"))?;
    let tag_text = text[start..].split_once('>')?.0;
    let at = tag_text.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag_text[at..].find('"')? + at;
    Some(&tag_text[at..end])
}

/// The text of an XML element with its whitespace flattened, which is what a title
/// with a line break in the feed needs.
fn squashed(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Crossref answers with JATS markup in its abstract, which a BibTeX field has no
/// use for.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    squashed(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CROSSREF: &str = r#"{
      "type": "journal-article",
      "title": ["A Study of Things"],
      "author": [
        {"family": "Doe", "given": "Jane"},
        {"family": "Smith", "given": "John A."}
      ],
      "container-title": ["Journal of Studies"],
      "volume": "12",
      "issue": "3",
      "page": "45-67",
      "issued": {"date-parts": [[2021, 5, 1]]},
      "publisher": "A Press",
      "abstract": "<jats:p>We study things.</jats:p>",
      "URL": "https://doi.org/10.1000/xyz"
    }"#;

    const ARXIV: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom">
      <entry>
        <title>A Study of
            Things</title>
        <author><name>Jane Doe</name></author>
        <author><name>John Smith</name></author>
        <published>2021-05-01T00:00:00Z</published>
        <arxiv:doi xmlns:arxiv="http://arxiv.org/schemas/atom">10.1000/xyz</arxiv:doi>
        <arxiv:primary_category xmlns:arxiv="http://arxiv.org/schemas/atom" term="cs.LG"/>
      </entry>
    </feed>"#;

    #[test]
    fn a_doi_becomes_an_article_with_its_authors_in_order() {
        let message: Value = serde_json::from_str(CROSSREF).unwrap();
        let entry = from_crossref("10.1000/xyz", &message).unwrap();
        assert_eq!(entry.kind.as_str(), "Article");
        assert_eq!(entry.field("title"), Some("A Study of Things"));
        assert_eq!(entry.field("author"), Some("Doe, Jane and Smith, John A."));
        assert_eq!(entry.field("journal"), Some("Journal of Studies"));
        assert_eq!(entry.field("number"), Some("3"));
        assert_eq!(entry.field("pages"), Some("45-67"));
        assert_eq!(entry.field("year"), Some("2021"));
        assert_eq!(entry.field("doi"), Some("10.1000/xyz"));
        assert_eq!(entry.field("abstract"), Some("We study things."));
    }

    #[test]
    fn an_arxiv_id_becomes_a_misc_with_its_eprint() {
        let entry = from_arxiv("2101.01234", ARXIV).unwrap();
        assert_eq!(entry.kind.as_str(), "Misc");
        assert_eq!(entry.field("title"), Some("A Study of Things"));
        assert_eq!(entry.field("author"), Some("Doe, Jane and Smith, John"));
        assert_eq!(entry.field("year"), Some("2021"));
        assert_eq!(entry.field("eprint"), Some("2101.01234"));
        assert_eq!(entry.field("archiveprefix"), Some("arXiv"));
        assert_eq!(entry.field("primaryclass"), Some("cs.LG"));
        assert_eq!(entry.field("doi"), Some("10.1000/xyz"));
    }

    #[test]
    fn a_record_with_no_title_is_refused_rather_than_filed_empty() {
        let message: Value = serde_json::from_str("{}").unwrap();
        assert!(from_crossref("10.1000/xyz", &message).is_err());
        assert!(from_arxiv("2101.01234", "<feed></feed>").is_err());
    }

    #[test]
    fn the_key_the_caller_chose_is_the_key_the_entry_gets() {
        let message: Value = serde_json::from_str(CROSSREF).unwrap();
        let entry = keyed(from_crossref("10.1000/xyz", &message).unwrap(), "doe2021");
        assert_eq!(entry.key.as_ref(), "doe2021");
    }
}
