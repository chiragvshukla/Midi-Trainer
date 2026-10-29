use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::settings::Settings;
use crate::soundfont::{self, SoundfontStatus};

use super::{app_data_dir, bundled_soundfont};

/// Reports which SoundFont is in effect and where the full bank would live.
#[tauri::command]
pub fn soundfont_status(
    app: AppHandle,
    settings: State<'_, Mutex<Settings>>,
) -> Result<SoundfontStatus, String> {
    let chosen = settings
        .lock()
        .map_err(|_| "settings state is unavailable".to_string())?
        .soundfont_path
        .clone();
    let bundled = bundled_soundfont(&app);
    Ok(soundfont::status(
        bundled.as_deref(),
        &app_data_dir(&app)?,
        chosen.as_deref(),
    ))
}
