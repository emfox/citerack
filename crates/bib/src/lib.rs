//! A BibTeX reader that keeps everything it was given: the source text, each
//! entry's byte span, and the fields this crate does not model, so a rewrite
//! reproduces the input and an error names the offset it was found at.

mod edit;
mod error;
mod key;
mod library;
mod model;
mod parse;
mod schema;
mod write;

pub use edit::{FieldLayout, Layout};
pub use error::{ParseError, ParseErrorKind};
pub use key::{Case, DEFAULT as DEFAULT_KEY_PATTERN, Pattern, PatternError, unique};
pub use library::{EditError, EntryId, EntryState, Library, Span};
pub use model::{Delimiter, Entry, EntryKind, Field, FieldName};
pub use parse::{ParseOptions, parse_str};
pub use schema::{Wanted, is_required, satisfied, wanted};
pub use write::{WriteOptions, write_str};
