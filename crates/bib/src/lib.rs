//! A BibTeX reader that keeps everything it was given: the source text, each
//! entry's byte span, and the fields this crate does not model, so a rewrite
//! reproduces the input and an error names the offset it was found at.

mod error;
mod library;
mod model;
mod parse;
mod write;

pub use error::{ParseError, ParseErrorKind};
pub use library::{EntryId, Library, Span};
pub use model::{Delimiter, Entry, EntryKind, Field, FieldName};
pub use parse::{ParseOptions, parse_str};
pub use write::{WriteOptions, write_str};
