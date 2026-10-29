use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::settings::Settings;

use super::settings_path;

/// Persists the current settings, for values that change continuously (sliders).
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: State<'_, Mutex<Settings>>) -> Result<(), String> {
    let settings = settings
        .lock()
        .map_err(|_| "settings state is unavailable".to_string())?
        .clone();
    settings.save(&settings_path(&app)?)
}
