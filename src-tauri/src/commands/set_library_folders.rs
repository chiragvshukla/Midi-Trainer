use tauri::AppHandle;

use crate::commands::update_settings;

/// Remembers the folders the user browses as their MIDI library. The frontend owns the list
/// (adding and removing), so this just stores whatever it sends.
#[tauri::command]
pub fn set_library_folders(folders: Vec<String>, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| settings.library_folders = folders)
}
