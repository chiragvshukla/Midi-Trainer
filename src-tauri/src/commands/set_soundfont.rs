use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::{app_data_dir, bundled_soundfont, update_settings};
use crate::soundfont::{self, SoundfontStatus};

/// Switches the SoundFont the engine plays: a specific `.sf2`, or `None` for the bundled
/// baseline. The choice is loaded and remembered, and the new status is returned.
#[tauri::command]
pub fn set_soundfont(
    path: Option<String>,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<SoundfontStatus, String> {
    let chosen = match path {
        Some(path) if Path::new(&path).is_file() => Some(path),
        Some(_) => return Err("the chosen SoundFont file does not exist".to_string()),
        None => None,
    };
    let bundled = bundled_soundfont(&app);
    let effective = match &chosen {
        Some(path) => PathBuf::from(path),
        None => bundled
            .clone()
            .ok_or_else(|| "the bundled SoundFont is missing".to_string())?,
    };
    {
        let mut engine = engine
            .lock()
            .map_err(|_| "audio state is unavailable".to_string())?;
        engine
            .load_soundfont(&effective)
            .map_err(|err| err.to_string())?;
    }
    update_settings(&app, |settings| settings.soundfont_path = chosen.clone())?;
    Ok(soundfont::status(
        bundled.as_deref(),
        &app_data_dir(&app)?,
        chosen.as_deref(),
    ))
}
