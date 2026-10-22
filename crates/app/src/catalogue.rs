//! Every word the window shows, in one file. A string is a key here and stays a
//! key everywhere else, so a second language would be a second module with the
//! same names and nothing else in the tree would move.

/// One piece of text the window shows. Each variant names a sentence rather than a
/// widget, so `Undo` the button and `Undid` the line it leads to can part ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Text {
    // The toolbar.
    Open,
    Import,
    Sort,
    SortKey,
    SortAuthor,
    SortYear,
    SortTitle,
    Save,
    SaveUnsaved,
    Undo,
    Redo,
    Key,
    GenerateKeys,
    Duplicates,
    DuplicatesShown,
    ShowDuplicates,
    ShowEveryEntry,
    ShowDuplicatePairs,
    System,
    SystemDark,
    SystemLight,
    Dark,
    Light,

    // The search box and the groups list.
    Search,
    SearchHint,
    Groups,
    LetGo,
    Clear,
    ShowEveryValue,
    ShortcutOpen,
    ShortcutSave,
    ShortcutUndo,
    ShortcutRedo,
    ImportTooltip,
    PatternTooltip,
    KindAndKey,
    FacetHeading,
    FacetHeadingPicked,
    FacetValue,
    AndMore,
    Entries,
    FoundResults,
    EntriesNarrowed,
    NoFile,
    Opening,
    ReadyIn,

    // The table.
    ColumnKey,
    ColumnType,
    ColumnAuthor,
    ColumnTitle,
    ColumnYear,
    ColumnVenue,
    ColumnGenerated,

    // The detail pane.
    SelectAnEntry,
    FieldMissing,
    FieldEmpty,
    Wants,
    AlsoEnteredAs,
    FileMissing,
    OpenFailed,

    // The batch box.
    EntriesPicked,
    PickedHint,
    Field,
    FieldHint,
    Value,
    ValueHint,
    WriteToAll,
    NoEntryTookName,
    NoEntryTookValue,
    OneValue,
    Values,
    NoneHasIt,
    AllSay,
    TheySay,
    TheySaySomeMissing,
    WroteTo,
    WroteToSome,
    CouldNotTake,
    ListMore,

    // The import box.
    ImportHeading,
    Close,
    ImportHint,
    ImportPlaceholder,
    LookUp,
    AskingFor,
    AddToLibrary,
    ImportNote,
    Imported,

    // Keys.
    KeysFollow,
    PatternRefused,
    GenerateHint,
    Renamed,
    RenamedCount,
    EveryKeyFollows,

    // The status line.
    Saved,
    SaveFailed,
    Undid,
    NothingToUndo,
    Redid,
    NothingToRedo,
    EditedNotSaved,
    Opened,
    NoProgramOpens,

    // A lookup that went wrong.
    LookupStopped,
    LookupUnreadable,
    LookupNoRecord,
    LookupTimeout,
    LookupFailed,
    LookupNotARecord,
    RecordNotFiled,
    ArxivHasNo,
}

impl Text {
    /// The English words for a key. A template may name its arguments, `{count}` or
    /// `{name}`, which `text!` fills; a brace the template wants shown is doubled.
    pub fn english(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Import => "Import",
            Self::Sort => "Sort",
            Self::SortKey => "Key",
            Self::SortAuthor => "Author",
            Self::SortYear => "Year",
            Self::SortTitle => "Title",
            Self::Save => "Save",
            Self::SaveUnsaved => "Save *",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Key => "Key",
            Self::GenerateKeys => "Generate keys",
            Self::Duplicates => "Duplicates {count}",
            Self::DuplicatesShown => "Duplicates {count} (shown)",
            Self::ShowDuplicates => "show only the entries that look like the same work twice",
            Self::ShowEveryEntry => "Showing every entry",
            Self::ShowDuplicatePairs => "Showing the entries in {count} duplicate {pairs}",
            Self::System => "System",
            Self::SystemDark => "System (dark)",
            Self::SystemLight => "System (light)",
            Self::Dark => "Dark",
            Self::Light => "Light",

            Self::Search => "Search",
            Self::SearchHint => "key, author, title... (space separates terms)",
            Self::Groups => "Groups",
            Self::LetGo => "{value}  (let go)",
            Self::Clear => "Clear",
            Self::ShowEveryValue => "Show every value",
            Self::ShortcutOpen => "Ctrl+O",
            Self::ShortcutSave => "Ctrl+S",
            Self::ShortcutUndo => "Ctrl+Z",
            Self::ShortcutRedo => "Ctrl+Shift+Z",
            Self::ImportTooltip => "an entry from a DOI or an arXiv id",
            Self::PatternTooltip => "the fields a generated key is made of, as {author}{year}",
            Self::KindAndKey => "{kind} \u{b7} {key}",
            Self::FacetHeading => "{heading} ({count})",
            Self::FacetHeadingPicked => "{heading}: {value}",
            Self::FacetValue => "{value} ({count})",
            Self::AndMore => "and {count} more",
            Self::Entries => "{count} entries",
            Self::FoundResults => "Found {total} results  |  shown {shown}  |  {seconds:.3} s",
            Self::EntriesNarrowed => "{count} entries  |  narrowed by {facets} facet {lists}",
            Self::NoFile => "Open a .bib file to begin.",
            Self::Opening => "Opening...",
            Self::ReadyIn => "ready in {seconds:.2} s",

            Self::ColumnKey => "Key",
            Self::ColumnType => "Type",
            Self::ColumnAuthor => "Author",
            Self::ColumnTitle => "Title",
            Self::ColumnYear => "Year",
            Self::ColumnVenue => "Journal / Book",
            Self::ColumnGenerated => "Generated",

            Self::SelectAnEntry => "Select an entry to see its fields.",
            Self::FieldMissing => "missing",
            Self::FieldEmpty => "empty",
            Self::Wants => "This {kind} wants {fields}.",
            Self::AlsoEnteredAs => "Also entered as {key}: {reason}",
            Self::FileMissing => "missing",
            Self::OpenFailed => "no program on this system opens files: {why}",

            Self::EntriesPicked => "{count} entries picked",
            Self::PickedHint => {
                "\u{b7} Shift-click or Shift-\u{2191}\u{2193} to add, Ctrl+A for all"
            }
            Self::Field => "field",
            Self::FieldHint => "keywords",
            Self::Value => "value",
            Self::ValueHint => "the text to write in all of them",
            Self::WriteToAll => "Write to all",
            Self::NoEntryTookName => "no entry took it: {name} is not a name a field can hold",
            Self::NoEntryTookValue => "no entry took the value for {name}",
            Self::OneValue => "1 value",
            Self::Values => "{count} values",
            Self::NoneHasIt => "none of them has {name} yet",
            Self::AllSay => "all of them say {value}",
            Self::TheySay => "they say {values} between them: {list}",
            Self::TheySaySomeMissing => "{values}, and {missing} have no {name} at all: {list}",
            Self::WroteTo => "Wrote {name} to {count} {entries}",
            Self::WroteToSome => "Wrote {name} to {count} {entries}; {refused} could not take it",
            Self::CouldNotTake => "{count} could not take it",
            Self::ListMore => "{shown} and {count} more",

            Self::ImportHeading => "Import",
            Self::Close => "Close",
            Self::ImportHint => "Paste a DOI, an arXiv id, or the link either one is shown at.",
            Self::ImportPlaceholder => "10.1038/nphys1170 or 2101.01234",
            Self::LookUp => "Look up",
            Self::AskingFor => "Asking for {id}",
            Self::AddToLibrary => "Add to the library",
            Self::ImportNote => {
                "Crossref answers for a DOI and arXiv for an id. A lookup that fails leaves the library untouched."
            }
            Self::Imported => "Imported {key}",

            Self::KeysFollow => "Keys follow {pattern}",
            Self::PatternRefused => "that pattern is not one: {why}",
            Self::GenerateHint => {
                "give every entry whose key the pattern makes the key it makes now"
            }
            Self::Renamed => "Renamed {was} to {now}",
            Self::RenamedCount => "Renamed {count} keys, not saved yet",
            Self::EveryKeyFollows => {
                "Every key already follows the pattern, or was written by hand"
            }

            Self::Saved => "Saved",
            Self::SaveFailed => "Save failed: {why}",
            Self::Undid => "Undid {what}",
            Self::NothingToUndo => "Nothing to undo",
            Self::Redid => "Redid {what}",
            Self::NothingToRedo => "Nothing to redo",
            Self::EditedNotSaved => "Edited, not saved yet",
            Self::Opened => "Opened {path}",
            Self::NoProgramOpens => "no program on this system opens files: {why}",

            Self::LookupStopped => "the lookup stopped without an answer",
            Self::LookupUnreadable => "the answer could not be read: {why}",
            Self::LookupNoRecord => "no such record",
            Self::LookupTimeout => "the service did not answer in time",
            Self::LookupFailed => "the lookup failed: {why}",
            Self::LookupNotARecord => "{service} sent something that is not a record",
            Self::RecordNotFiled => "the record could not be filed: {why}",
            Self::ArxivHasNo => "arXiv has no paper {id}",
        }
    }
}

/// The text for a key, filled with the arguments the key names. A name the caller
/// did not supply is left as `{name}` rather than panicking, since a window showing
/// a stray brace beats one that closes.
#[macro_export]
macro_rules! text {
    ($key:ident) => {
        $crate::catalogue::fill($crate::catalogue::Text::$key, &[])
    };
    ($key:ident, $($name:ident: $value:expr),+ $(,)?) => {
        $crate::catalogue::fill(
            $crate::catalogue::Text::$key,
            &[$((stringify!($name), format!("{}", $value))),+],
        )
    };
}

/// Fills a template with named arguments. See `text!` for the way in.
pub fn fill(key: Text, args: &[(&str, String)]) -> String {
    let template = key.english();
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("{{") {
            out.push('{');
            rest = after;
            continue;
        }
        if let Some(after) = tail.strip_prefix("}}") {
            out.push('}');
            rest = after;
            continue;
        }
        let Some(end) = tail.find('}') else {
            out.push_str(tail);
            rest = "";
            break;
        };
        let name = &tail[1..end];
        match args.iter().find(|(given, _)| *given == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_with_no_arguments_is_its_own_words() {
        assert_eq!(fill(Text::Save, &[]), "Save");
        assert_eq!(fill(Text::NothingToUndo, &[]), "Nothing to undo");
    }

    #[test]
    fn an_argument_is_filled_where_the_template_names_it() {
        let said = fill(Text::Entries, &[("count", String::from("4"))]);
        assert_eq!(said, "4 entries");
    }

    #[test]
    fn an_argument_the_caller_did_not_supply_stays_visible() {
        assert_eq!(fill(Text::Entries, &[]), "{count} entries");
    }

    #[test]
    fn an_argument_named_the_same_twice_is_filled_twice() {
        let said = fill(
            Text::WroteToSome,
            &[
                ("name", String::from("keywords")),
                ("count", String::from("2")),
                ("entries", String::from("entries")),
                ("refused", String::from("1")),
            ],
        );
        assert_eq!(said, "Wrote keywords to 2 entries; 1 could not take it");
    }

    #[test]
    fn a_doubled_brace_is_a_brace() {
        let said = fill(Text::PickedHint, &[]);
        assert!(said.starts_with("\u{b7} Shift-click"), "{said}");
    }

    #[test]
    fn every_key_has_words_of_its_own() {
        for key in [Text::Save, Text::Import, Text::ColumnYear, Text::Undid] {
            assert!(!key.english().is_empty(), "{key:?} has no words");
        }
    }

    #[test]
    fn the_macro_fills_from_the_callers_names() {
        let said = text!(Entries, count: 12);
        assert_eq!(said, "12 entries");
        assert_eq!(text!(Search), "Search");
    }
}
