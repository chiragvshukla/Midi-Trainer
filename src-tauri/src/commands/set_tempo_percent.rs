use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::{AudioEngine, MAX_TEMPO_PERCENT, MIN_TEMPO_PERCENT};
use crate::commands::change_settings;

/// Sets the playback tempo as a percentage of the original (100 is normal speed).
#[tauri::command]
pub fn set_tempo_percent(
    percent: f32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let percent = percent.clamp(MIN_TEMPO_PERCENT, MAX_TEMPO_PERCENT);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_tempo_percent(percent);
    change_settings(&app, |settings| settings.tempo_percent = percent)?;
    Ok(())
}
