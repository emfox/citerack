//! One lookup at a time, off the thread that draws. The window starts a request and
//! keeps drawing; the answer arrives on a channel and is read when it does, so a
//! slow service costs a sentence rather than a frozen frame.

use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;

use crate::text;
use citerack_bib::Entry;
use citerack_index::{Found, Identifier, first_surname, from_arxiv, from_crossref, keyed};
use serde_json::Value;

/// What a lookup is doing, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Nothing asked for yet, or the last answer was read and let go.
    Idle,
    /// A request is out, named by the identifier it asked about.
    Asking(String),
    /// A record came back, ready to be filed.
    Read(Box<Entry>),
    /// Nothing came back, and this is what to say about it.
    Failed(String),
}

/// The request in flight, and where its answer will arrive.
#[derive(Default)]
pub struct Lookup {
    pending: Option<Receiver<Result<Entry, String>>>,
}

impl Lookup {
    /// Asks a service about an identifier, on a thread of its own. The entry comes
    /// back with a key of its own making; the library moves it if it is taken.
    pub fn ask(&mut self, found: &Found) {
        let (sender, receiver) = channel();
        self.pending = Some(receiver);
        let kind = found.kind;
        let id = found.id.clone();
        thread::spawn(move || {
            let answer = match kind {
                Identifier::Doi => crossref(&id),
                Identifier::Arxiv => arxiv(&id),
            };
            // A receiver that is gone means the window closed, which is no failure.
            let answer = answer.map(|entry| {
                let key = key_for(&entry);
                if key.is_empty() {
                    // A record with no author and no year is keyed by the id it
                    // was asked for, which the caller has and the record does not.
                    let tail = alphanumeric(&id);
                    return keyed(entry, &tail);
                }
                keyed(entry, &key)
            });
            let _ = sender.send(answer);
        });
    }

    /// The answer, if it has arrived, and whether one is still out.
    pub fn read(&mut self) -> Option<Result<Entry, String>> {
        let pending = self.pending.as_ref()?;
        match pending.try_recv() {
            Ok(answer) => {
                self.pending = None;
                Some(answer)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = None;
                Some(Err(text!(LookupStopped)))
            }
        }
    }

    /// Whether a request is still out.
    pub fn is_asking(&self) -> bool {
        self.pending.is_some()
    }

    /// Forgets a request whose answer nobody wants, which is what closing the box
    /// does.
    pub fn cancel(&mut self) {
        self.pending = None;
    }
}

/// The citation key a record deserves: the first author's surname and the year, or
/// the identifier itself turned into something a key can hold when it says neither.
pub fn key_for(entry: &Entry) -> String {
    let surname = entry
        .field("author")
        .and_then(|authors| authors.split(" and ").next())
        .and_then(|first| first.split(',').next())
        .map(first_surname)
        .unwrap_or_default();
    let year = entry.field("year").unwrap_or_default();
    let tail = format!("{surname}{year}");
    if tail.is_empty() {
        let known = entry
            .field("doi")
            .or(entry.field("eprint"))
            .unwrap_or_default();
        return alphanumeric(known);
    }
    tail
}

/// Everything a key may hold, in lowercase.
pub fn alphanumeric(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn crossref(id: &str) -> Result<Entry, String> {
    let url = format!("https://api.crossref.org/works/{id}");
    let body = get(&url)?;
    let answer: Value = serde_json::from_str(&body).map_err(|_| timed_out_or("Crossref"))?;
    let message = answer.get("message").ok_or("Crossref sent no record")?;
    from_crossref(id, message).map_err(|why| format!("Crossref: {why}"))
}

fn arxiv(id: &str) -> Result<Entry, String> {
    let url = format!("https://export.arxiv.org/api/query?id_list={id}");
    let body = get(&url)?;
    if body.contains("<opensearch:totalResults>0<") {
        return Err(text!(ArxivHasNo, id: id));
    }
    from_arxiv(id, &body).map_err(|why| format!("arXiv: {why}"))
}

/// One GET, with the timeout a lookup deserves: a reader waiting for a record will
/// wait a few seconds, not a minute.
fn get(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(8)))
        .user_agent("citerack (https://github.com/emfox/citerack)")
        .build()
        .into();
    match agent.get(url).call() {
        Ok(mut response) => response
            .body_mut()
            .read_to_string()
            .map_err(|why| text!(LookupUnreadable, why: why)),
        Err(ureq::Error::StatusCode(404)) => Err(text!(LookupNoRecord)),
        Err(ureq::Error::Timeout(_)) => Err(text!(LookupTimeout)),
        Err(why) => Err(text!(LookupFailed, why: why)),
    }
}

fn timed_out_or(service: &str) -> String {
    text!(LookupNotARecord, service: service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_the_surname_and_the_year() {
        let mut entry = Entry {
            kind: citerack_bib::EntryKind::new("Article"),
            key: "".into(),
            fields: Vec::new(),
        };
        let fields = [("author", "Doe, Jane and Smith, John"), ("year", "2021")];
        for (name, value) in fields {
            entry.fields.push(citerack_bib::Field {
                name_raw: name.into(),
                name: citerack_bib::FieldName::new(name),
                value: value.into(),
                delimiter: citerack_bib::Delimiter::Braces,
            });
        }
        assert_eq!(key_for(&entry), "doe2021");
    }

    #[test]
    fn a_key_with_nothing_to_build_from_falls_back_to_the_identifier() {
        let entry = Entry {
            kind: citerack_bib::EntryKind::new("Misc"),
            key: "".into(),
            fields: vec![citerack_bib::Field {
                name_raw: "eprint".into(),
                name: citerack_bib::FieldName::new("eprint"),
                value: "2101.01234".into(),
                delimiter: citerack_bib::Delimiter::Braces,
            }],
        };
        assert_eq!(key_for(&entry), "210101234");
    }
}
