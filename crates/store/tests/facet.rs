//! The facet lists: each one narrows the table, and the count beside a value is
//! the table the other lists allow rather than the whole file.

use std::path::PathBuf;

use citerack_bib::ParseOptions;
use citerack_store::{Facet, Session};

const INPUT: &str = "\
@article{smith2024,
  Author  = {Smith, John},
  Title   = {A Study of Braces},
  Year    = 2024,
  Journal = {Journal of Braces},
}

@article{smith2023,
  Author  = {Smith, John},
  Title   = {Another Study},
  Year    = 2023,
  Journal = {Journal of Braces},
}

@article{jones2024,
  Author  = {Jones, Ann},
  Title   = {A Book Chapter},
  Year    = 2024,
  Journal = {Journal of Knots},
}

@book{jones1999,
  Author    = {Jones, Ann},
  Title     = {Another Book},
  Year      = 1999,
  Publisher = {Knot Press},
}

@misc{anon,
  Title = {Untitled Notes},
}
";

fn session() -> Session {
    Session::from_text(
        PathBuf::from("facet.bib"),
        INPUT,
        &ParseOptions { recover: true },
    )
    .expect("the input parses")
}

/// The counts one list offers, as `(value, count)` in the order shown.
fn listed(session: &mut Session, facet: Facet) -> Vec<(String, usize)> {
    session
        .facet_values(facet)
        .iter()
        .map(|value| (value.label.clone(), value.count))
        .collect()
}

fn pair(value: &str, count: usize) -> (String, usize) {
    (value.to_owned(), count)
}

#[test]
fn a_list_counts_every_row_it_would_leave_on_the_table() {
    let mut session = session();
    assert_eq!(
        listed(&mut session, Facet::Author),
        [pair("Jones", 2), pair("Smith", 2)],
        "biggest group first, with ties broken by name, and no entry with no author"
    );
    assert_eq!(
        listed(&mut session, Facet::Kind),
        [pair("article", 3), pair("book", 1), pair("misc", 1)],
        "biggest group first"
    );
}

#[test]
fn a_venue_is_the_journal_else_the_publisher() {
    let mut session = session();
    let venues = listed(&mut session, Facet::Venue);
    assert!(venues.contains(&pair("Journal of Braces", 2)));
    assert!(venues.contains(&pair("Knot Press", 1)));
    assert_eq!(venues.len(), 3, "one list of where, not one per field");
}

#[test]
fn picking_a_value_narrows_the_table_to_it() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Jones");
    assert_eq!(session.order(), [2, 3], "both of Jones's entries, in order");
    assert_eq!(session.facet_count(), 1);
}

#[test]
fn pressing_the_picked_value_again_lets_it_go() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Jones");
    session.toggle_facet(Facet::Author, "Jones");
    assert_eq!(session.order().len(), 5, "the whole library is back");
    assert_eq!(session.picked(Facet::Author), None);
}

#[test]
fn a_second_list_counts_what_the_first_one_allows() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Jones");
    assert_eq!(
        listed(&mut session, Facet::Year),
        [pair("1999", 1), pair("2024", 1)],
        "the years counted are Jones's, not the file's"
    );
    assert_eq!(
        listed(&mut session, Facet::Venue),
        [pair("Journal of Knots", 1), pair("Knot Press", 1)]
    );
}

#[test]
fn a_list_holding_a_value_still_offers_the_others() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Jones");
    let shown = listed(&mut session, Facet::Author);
    assert_eq!(
        shown.first(),
        Some(&pair("Jones", 2)),
        "the held value stays at the top: {shown:?}"
    );
}

#[test]
fn two_lists_narrow_together() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Jones");
    session.toggle_facet(Facet::Year, "2024");
    assert_eq!(session.order(), [2], "Jones in 2024");
    assert_eq!(session.facet_count(), 2);
}

#[test]
fn a_search_and_a_facet_narrow_the_same_table() {
    let mut session = session();
    session.search("knots");
    session.toggle_facet(Facet::Author, "Jones");
    assert_eq!(session.order(), [2], "the query found one entry for Jones");
    session.toggle_facet(Facet::Author, "Smith");
    assert_eq!(
        session.order(),
        [],
        "a surname the query did not find leaves nothing on the table"
    );
}

#[test]
fn letting_a_list_go_while_a_search_stands_keeps_the_search() {
    let mut session = session();
    session.search("braces");
    session.toggle_facet(Facet::Author, "Smith");
    session.clear_facet(Facet::Author);
    assert_eq!(session.order(), [0, 1], "the query is still in force");
    assert_eq!(session.facet_count(), 0);
}

#[test]
fn clearing_every_list_puts_the_whole_library_back() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Smith");
    session.toggle_facet(Facet::Year, "2024");
    session.clear_facets();
    assert_eq!(session.order().len(), 5);
    assert_eq!(session.facet_count(), 0);
}

#[test]
fn an_edit_that_changes_a_value_takes_its_row_out_of_the_group() {
    let mut session = session();
    session.toggle_facet(Facet::Author, "Smith");
    session.set_field(0, 0, "Brown, John").expect("a surname");
    assert_eq!(
        session.order(),
        [1],
        "the row that changed no longer carries the picked surname"
    );
}

#[test]
fn an_edit_the_lists_do_not_filter_by_still_recounts_them() {
    let mut session = session();
    assert_eq!(listed(&mut session, Facet::Author)[0], pair("Jones", 2));
    // No list is picked, so the edit moves no row: the count has to follow anyway.
    session.set_field(0, 0, "Brown, John").expect("a surname");
    assert_eq!(
        listed(&mut session, Facet::Author),
        [pair("Jones", 2), pair("Brown", 1), pair("Smith", 1)],
        "the group a row joined and the group it left are both recounted"
    );
}
