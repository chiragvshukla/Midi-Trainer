use tauri::AppHandle;

use crate::soundfont::{self, SoundfontStatus};

use super::app_data_dir;

/// Reports whether the baseline SoundFont is installed and where it lives.
#[tauri::command]
pub fn soundfont_status(app: AppHandle) -> Result<SoundfontStatus, String> {
    Ok(soundfont::status(&app_data_dir(&app)?))
}
