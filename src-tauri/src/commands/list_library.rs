//! Listing the user's MIDI library folder.
//!
//! The folder is user-chosen, so it is a trust boundary: a missing root is reported, while
//! unreadable files and subfolders are skipped rather than failing the whole listing.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The most entries a library will list, so a huge folder cannot stall the UI.
const MAX_ENTRIES: usize = 5000;

/// How deep the scan descends, a guard against accidental symlink loops.
const MAX_DEPTH: usize = 16;

/// One file the library can open.
#[derive(Serialize)]
pub struct LibraryEntry {
    pub name: String,
    /// Path relative to the library folder, with forward slashes.
    pub relative_path: String,
    /// File size in bytes, or 0 when it could not be read.
    pub size_bytes: u64,
    /// Last-modified time as Unix milliseconds, or 0 when it could not be read.
    pub modified_ms: u64,
}

/// Lists the MIDI and karaoke files under a folder, recursing into subfolders. The entries
/// are sorted by relative path so the library reads in a stable order.
#[tauri::command]
pub fn list_library(folder: String) -> Result<Vec<LibraryEntry>, String> {
    let root = PathBuf::from(&folder);
    let metadata =
        std::fs::metadata(&root).map_err(|err| format!("cannot read {folder}: {err}"))?;
    if !metadata.is_dir() {
        return Err(format!("{folder} is not a folder"));
    }

    let mut entries = Vec::new();
    scan(&root, &root, 0, &mut entries);
    entries.sort_by_key(|entry| entry.relative_path.to_lowercase());
    Ok(entries)
}

/// Collects MIDI files under `dir`, skipping unreadable entries. `root` anchors the paths.
fn scan(root: &Path, dir: &Path, depth: usize, entries: &mut Vec<LibraryEntry>) {
    if depth > MAX_DEPTH || entries.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for item in read_dir.flatten() {
        if entries.len() >= MAX_ENTRIES {
            return;
        }
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            scan(root, &path, depth + 1, entries);
        } else if is_midi(&path)
            && let Ok(relative) = path.strip_prefix(root)
        {
            let metadata = item.metadata().ok();
            entries.push(LibraryEntry {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                relative_path: relative.to_string_lossy().replace('\\', "/"),
                size_bytes: metadata.as_ref().map_or(0, |file| file.len()),
                modified_ms: metadata
                    .and_then(|file| file.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |since| since.as_millis() as u64),
            });
        }
    }
}

/// Whether the file's extension marks it as a MIDI or karaoke file.
fn is_midi(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mid" | "midi" | "kar"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{is_midi, list_library};
    use std::path::Path;

    #[test]
    fn recognizes_midi_extensions_case_insensitively() {
        assert!(is_midi(Path::new("a.mid")));
        assert!(is_midi(Path::new("A.MID")));
        assert!(is_midi(Path::new("b.kar")));
        assert!(!is_midi(Path::new("c.txt")));
        assert!(!is_midi(Path::new("d")));
    }

    #[test]
    fn lists_midi_files_recursively_and_ignores_others() {
        let dir = std::env::temp_dir().join("mt-library-listing-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("sub")).expect("the directory should create");
        std::fs::write(dir.join("b.mid"), b"").unwrap();
        std::fs::write(dir.join("a.mid"), b"").unwrap();
        std::fs::write(dir.join("notes.txt"), b"").unwrap();
        std::fs::write(dir.join("sub/c.kar"), b"").unwrap();

        let entries = list_library(dir.to_string_lossy().into_owned()).unwrap();
        let paths: Vec<String> = entries
            .into_iter()
            .map(|entry| entry.relative_path)
            .collect();

        assert_eq!(paths, vec!["a.mid", "b.mid", "sub/c.kar"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_each_files_size_and_modified_time() {
        let dir = std::env::temp_dir().join("mt-library-metadata-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the directory should create");
        std::fs::write(dir.join("a.mid"), b"12345").unwrap();

        let entries = list_library(dir.to_string_lossy().into_owned()).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].size_bytes, 5);
        assert!(entries[0].modified_ms > 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        assert!(list_library("Z:/definitely-missing-library".to_string()).is_err());
    }
}
