use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Sets the playback tempo as a percentage of the original (100 is normal speed).
#[tauri::command]
pub fn set_tempo_percent(
    percent: f32,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_tempo_percent(percent);
    Ok(())
}
