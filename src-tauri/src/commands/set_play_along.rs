use tauri::AppHandle;

use crate::commands::update_settings;

/// Remembers whether Follow mode also plays the other hand. The piece is rebuilt by the
/// frontend so the accompaniment follows the new choice.
#[tauri::command]
pub fn set_play_along(on: bool, app: AppHandle) -> Result<(), String> {
    update_settings(&app, |settings| settings.play_along = on)
}
