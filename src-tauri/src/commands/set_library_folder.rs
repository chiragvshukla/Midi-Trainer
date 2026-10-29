use tauri::AppHandle;

use crate::commands::update_settings;

/// Remembers the folder the user browses as their MIDI library, or clears it.
#[tauri::command]
pub fn set_library_folder(folder: Option<String>, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| settings.library_folder = folder)
}
