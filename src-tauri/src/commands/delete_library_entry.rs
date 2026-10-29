use std::path::Path;

use tauri::AppHandle;

use crate::commands::list_library::is_midi;
use crate::commands::update_settings;

/// Removes a file from the library: records it as hidden, drops it from the Recent list, and
/// remembers the user's storage choice when they asked not to be prompted again. With
/// `delete_storage`, the file itself is deleted too.
///
/// Trust boundary: only an existing MIDI or karaoke file is ever removed from disk.
#[tauri::command]
pub fn delete_library_entry(
    path: String,
    delete_storage: bool,
    remember: Option<bool>,
    app: AppHandle,
) -> Result<(), String> {
    if delete_storage {
        delete_midi_file(&path)?;
    }
    update_settings(&app, |settings| {
        if let Some(choice) = remember {
            settings.delete_from_storage = Some(choice);
        }
        settings.hide_file(&path);
    })
}

/// Deletes a MIDI or karaoke file, refusing any other path.
fn delete_midi_file(path: &str) -> Result<(), String> {
    let file = Path::new(path);
    if !is_midi(file) {
        return Err(format!("{path} is not a MIDI file"));
    }
    if !file.is_file() {
        return Err(format!("{path} could not be found"));
    }
    std::fs::remove_file(file).map_err(|err| format!("could not delete {path}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::delete_midi_file;

    #[test]
    fn deletes_a_midi_file() {
        let dir = std::env::temp_dir().join("mt-delete-entry-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the directory should create");
        let path = dir.join("a.mid");
        std::fs::write(&path, b"MThd").unwrap();

        delete_midi_file(&path.to_string_lossy()).expect("the MIDI should delete");
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refuses_anything_but_a_midi_file() {
        let dir = std::env::temp_dir().join("mt-delete-refuse-test");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the directory should create");
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"keep me").unwrap();

        assert!(delete_midi_file(&text.to_string_lossy()).is_err());
        assert!(text.exists(), "a non-MIDI file must be left alone");
        assert!(delete_midi_file(&dir.join("missing.mid").to_string_lossy()).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
