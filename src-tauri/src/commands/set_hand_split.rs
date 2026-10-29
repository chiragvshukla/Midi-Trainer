use tauri::AppHandle;

use crate::commands::update_settings;

/// Remembers whether a one-track piece offers hand practice, split at middle C. The piece is
/// rebuilt by the frontend so the gates and matcher follow the new part.
#[tauri::command]
pub fn set_hand_split(on: bool, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| settings.hand_split = on)
}
