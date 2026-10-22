//! What a `file` field names: groups of `name:path:type` separated by semicolons.
//! Mendeley's `$\backslash$` is read as the backslash it stands for, and a group
//! that reads no known shape is dropped rather than guessed at.

/// One file a `file` field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// What the manager calls the file, empty when the field left the name out.
    pub name: String,
    /// The path as the field spells it, unescaped. Never empty, since a group
    /// with nothing to open is not an attachment.
    pub path: String,
    /// The type as the manager spelled it: `PDF`, `application/pdf`, `pdf`.
    pub kind: String,
}

/// The files a field names, in order. A group naming no path is dropped, and one
/// that cannot be read does not stop the groups after it.
pub fn parse_file_field(value: &str) -> Vec<Attachment> {
    groups(value).into_iter().filter_map(read).collect()
}

/// Reads one group, or nothing when it does not say which file it means.
fn read(group: &str) -> Option<Attachment> {
    // The parts are handed out by position rather than by pattern: a group is cut
    // wherever a colon was, and the drive of a Windows path adds one, so the count
    // is what says which form arrived.
    let parts: Vec<String> = split(group).into_iter().map(unescape).collect();
    let named = |name: &String, path: &String, kind: &String| Attachment {
        name: name.clone(),
        path: path.clone(),
        kind: kind.clone(),
    };
    let attachment = match parts.len() {
        // A path on its own, which some writers emit for a file with no name.
        1 if !parts[0].is_empty() && !looks_like_kind(&parts[0]) => Attachment {
            name: String::new(),
            path: parts[0].clone(),
            kind: String::new(),
        },
        // The form every manager agrees on.
        3 if !parts[0].is_empty() => named(&parts[0], &parts[1], &parts[2]),
        // The name left out in front of a path whose drive holds the other colon.
        3 => Attachment {
            name: String::new(),
            path: join_drive(&parts[1], &parts[2]),
            kind: String::new(),
        },
        // The name left out in front of a path with no drive.
        2 => named(&String::new(), &parts[0], &parts[1]),
        // The name left out, a path whose drive was cut in two, and a type.
        4 if parts[0].is_empty() => Attachment {
            name: String::new(),
            path: join_drive(&parts[1], &parts[2]),
            kind: parts[3].clone(),
        },
        _ => return None,
    };
    if attachment.path.trim().is_empty() {
        return None;
    }
    Some(attachment)
}

/// Whether a one-part group names a type rather than a file: a type is a word like
/// `PDF`, while a path holds a slash, a dot or a backslash.
fn looks_like_kind(part: &str) -> bool {
    !part.contains(['/', '.', '\\', ':'])
}

/// A path whose drive was cut at its colon, put back: the backslash Mendeley
/// leaves there is the colon the cut removed.
fn join_drive(drive: &str, rest: &str) -> String {
    // A drive is one letter, cut at its colon by the separator.
    if drive.len() == 1 {
        return format!("{drive}:{rest}");
    }
    // Otherwise the cut left Mendeley's backslash standing where the colon was,
    // so the colon goes back and the backslash comes off.
    match drive.strip_suffix('\\') {
        Some(letter) => format!("{letter}:{rest}"),
        None => format!("{drive}:{rest}"),
    }
}

/// The groups a field holds. JabRef and its relatives separate with `;`; a
/// separator escaped with a backslash belongs to the path, which is how a
/// Windows path that itself holds a semicolon survives the trip.
fn groups(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            b'\\' if escapes(bytes.get(at + 1)) => at += 2,
            b';' => {
                out.push(&value[start..at]);
                start = at + 1;
                at = start;
            }
            _ => at += 1,
        }
    }
    out.push(&value[start..]);
    out
}

/// One group cut at every unescaped colon.
fn split(group: &str) -> Vec<&str> {
    let bytes = group.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            // Only the escapes this field uses step over a character. A backslash
            // before anything else is part of a Windows path, and stepping over
            // the byte after it would hide the colon of `$\backslash$:`.
            b'\\' if escapes(bytes.get(at + 1)) => at += 2,
            b':' => {
                parts.push(&group[start..at]);
                start = at + 1;
                at = start;
            }
            _ => at += 1,
        }
    }
    parts.push(&group[start..]);
    parts
}

/// Whether a backslash in front of this byte is escaping it.
fn escapes(next: Option<&u8>) -> bool {
    matches!(next, Some(b'\\') | Some(b':') | Some(b';'))
}

/// The inverse of the field's escaping: two backslashes are one, one before a
/// separator is the separator, and `$\backslash$:` is a drive colon, not an escape.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['\\', '$']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some(after) = tail.strip_prefix("$\\backslash$:") {
            out.push(':');
            rest = after;
            continue;
        }
        if let Some(after) = tail.strip_prefix("$\\backslash$") {
            out.push('\\');
            rest = after;
            continue;
        }
        match tail.as_bytes().get(1) {
            Some(b'\\') => out.push('\\'),
            Some(b':') => out.push(':'),
            Some(b';') => out.push(';'),
            _ => {
                // A lone backslash before anything else is part of a Windows path
                // and stays. A lone `$` is part of nothing, and also stays.
                out.push_str(&tail[..1]);
                rest = &tail[1..];
                continue;
            }
        }
        rest = &tail[2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(name: &str, path: &str, kind: &str) -> Attachment {
        Attachment {
            name: name.into(),
            path: path.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn reads_what_jabref_writes() {
        assert_eq!(
            parse_file_field("smith2024.pdf:/home/me/papers/smith2024.pdf:PDF"),
            vec![one("smith2024.pdf", "/home/me/papers/smith2024.pdf", "PDF")]
        );
    }

    #[test]
    fn reads_what_zotero_writes() {
        assert_eq!(
            parse_file_field("smith2024.pdf:/home/me/papers/smith2024.pdf:application/pdf"),
            vec![one(
                "smith2024.pdf",
                "/home/me/papers/smith2024.pdf",
                "application/pdf"
            )]
        );
    }

    #[test]
    fn reads_the_form_mendeley_writes() {
        // As quoted in the JabRef forum: the escape, then the colon of the drive.
        assert_eq!(
            parse_file_field(
                r":C$\backslash$:/Users/me/AppData/Local/Mendeley/Brown - 2017.pdf:pdf"
            ),
            vec![one(
                "",
                "C:/Users/me/AppData/Local/Mendeley/Brown - 2017.pdf",
                "pdf"
            )],
            "the LaTeX escape in the path is undone, so the link opens"
        );
    }

    #[test]
    fn keeps_the_colon_of_a_drive() {
        assert_eq!(
            parse_file_field(r":C:\papers\a.pdf:PDF"),
            vec![one("", r"C:\papers\a.pdf", "PDF")],
            "the drive colon is not a separator, and the name is empty"
        );
        assert_eq!(
            parse_file_field(r"a.pdf:C\:\papers\a.pdf:PDF"),
            vec![one("a.pdf", r"C:\papers\a.pdf", "PDF")],
            "a named file on a drive, with the colon escaped as JabRef writes it"
        );
    }

    #[test]
    fn reads_a_path_and_a_type_with_no_name() {
        assert_eq!(
            parse_file_field("papers/smith2024.pdf:PDF"),
            vec![one("", "papers/smith2024.pdf", "PDF")],
            "a relative path, which is resolved beside the library"
        );
    }

    #[test]
    fn separates_files_with_a_semicolon() {
        let many = "a.pdf:/papers/a.pdf:PDF;b.pdf:/papers/b.pdf:PDF";
        assert_eq!(parse_file_field(many).len(), 2);
        assert_eq!(parse_file_field(many)[1].path, "/papers/b.pdf");
    }

    #[test]
    fn reads_back_a_separator_a_backslash_escaped() {
        assert_eq!(
            parse_file_field(r"odd.pdf:C\:\\notes\;draft\\odd.pdf:PDF"),
            vec![one("odd.pdf", r"C:\notes;draft\odd.pdf", "PDF")],
            "an escaped colon, semicolon and backslash all read as themselves"
        );
    }

    #[test]
    fn a_group_with_no_path_is_dropped() {
        assert!(parse_file_field("").is_empty());
        assert!(parse_file_field(";;").is_empty());
        assert!(parse_file_field("PDF").is_empty());
        assert!(parse_file_field("name::PDF").is_empty());
    }

    #[test]
    fn a_trailing_separator_does_not_lose_the_last_group() {
        assert_eq!(parse_file_field("a.pdf:/a.pdf:PDF;").len(), 1);
    }

    #[test]
    fn a_backslash_before_anything_else_stays() {
        assert_eq!(
            parse_file_field(r":C:\new\paper.pdf:PDF")[0].path,
            r"C:\new\paper.pdf",
            "`\\n` is not read as a newline"
        );
    }
}
