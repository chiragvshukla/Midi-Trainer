use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;

/// Sets the playback tempo as a percentage of the original (100 is normal speed).
#[tauri::command]
pub fn set_tempo_percent(
    percent: f32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let percent = percent.clamp(10.0, 400.0);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_tempo_percent(percent);
    update_settings(&app, |settings| settings.tempo_percent = percent)?;
    Ok(())
}
