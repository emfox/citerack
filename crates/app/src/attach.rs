//! Opening the file an entry names, through the platform's own opener so nothing
//! is linked. A path is resolved first, and a miss is a value for the pane to
//! write beside the field rather than a dialog over the window.

use std::path::{Path, PathBuf};

use citerack_bib::Attachment;

/// Where an attachment's path was looked for and what was there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A file, at the path this reader should hand to the desktop.
    File(PathBuf),
    /// A path that resolved to nothing, with the place it was looked for.
    Missing(PathBuf),
}

/// Resolves every path a `file` field names. A relative one is counted from
/// `library`, the folder a manager keeps its files beside.
pub fn resolve(library: &Path, attachments: &[Attachment]) -> Vec<Found> {
    let base = library.parent().unwrap_or(Path::new("."));
    attachments
        .iter()
        .map(|attachment| {
            let path = Path::new(&attachment.path);
            let full = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base.join(path)
            };
            if full.is_file() {
                Found::File(full)
            } else {
                Found::Missing(full)
            }
        })
        .collect()
}

/// Asks the desktop to open a file. The error is for a machine with no desktop to
/// ask, which has no default handler to call.
pub fn open(path: &Path) -> Result<(), String> {
    let spawned = opener(path);
    match spawned {
        Ok(mut child) => {
            // The opener is a short-lived launcher on every platform, and waiting
            // for it would freeze the window while a viewer starts.
            let _ = child.try_wait();
            Ok(())
        }
        Err(err) => Err(format!("no program on this system opens files: {err}")),
    }
}

#[cfg(target_os = "linux")]
fn opener(path: &Path) -> std::io::Result<std::process::Child> {
    // `xdg-open` is the desktop's own way in, and it is present wherever a
    // desktop is. It is spawned rather than linked, so the binary keeps its
    // three linked libraries.
    std::process::Command::new("xdg-open").arg(path).spawn()
}

#[cfg(target_os = "macos")]
fn opener(path: &Path) -> std::io::Result<std::process::Child> {
    std::process::Command::new("open").arg(path).spawn()
}

#[cfg(windows)]
fn opener(path: &Path) -> std::io::Result<std::process::Child> {
    // `cmd /c start` is the shell's own handler; the empty string is the window
    // title, without which `start` takes a quoted path for one.
    std::process::Command::new("cmd")
        .args(["/c", "start", ""])
        .arg(path)
        .spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(path: &str) -> Attachment {
        Attachment {
            name: String::new(),
            path: path.into(),
            kind: "PDF".into(),
        }
    }

    #[test]
    fn a_relative_path_is_resolved_beside_the_library() {
        let base = std::env::temp_dir().join("citerack-attach");
        std::fs::create_dir_all(base.join("papers")).expect("a folder");
        let paper = base.join("papers/a.pdf");
        std::fs::write(&paper, b"%PDF").expect("a file");
        let library = base.join("library.bib");

        let found = resolve(&library, &[attachment("papers/a.pdf")]);
        assert_eq!(found, vec![Found::File(paper)]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_absolute_path_is_left_as_it_is() {
        let base = std::env::temp_dir().join("citerack-abs");
        std::fs::create_dir_all(&base).expect("a folder");
        let paper = base.join("b.pdf");
        std::fs::write(&paper, b"%PDF").expect("a file");
        let found = resolve(
            Path::new("/nowhere/library.bib"),
            &[attachment(&paper.to_string_lossy())],
        );
        assert_eq!(found, vec![Found::File(paper)]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_missing_file_is_reported_with_the_path_it_was_looked_for() {
        let found = resolve(
            Path::new("/papers/library.bib"),
            &[attachment("papers/gone.pdf")],
        );
        assert_eq!(
            found,
            vec![Found::Missing(PathBuf::from("/papers/papers/gone.pdf"))]
        );
    }
}
