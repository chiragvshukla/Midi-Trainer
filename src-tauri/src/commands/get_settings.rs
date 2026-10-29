use std::sync::Mutex;

use tauri::State;

use crate::settings::Settings;

/// Returns the persisted settings.
#[tauri::command]
pub fn get_settings(settings: State<'_, Mutex<Settings>>) -> Result<Settings, String> {
    settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|_| "settings state is unavailable".to_string())
}
