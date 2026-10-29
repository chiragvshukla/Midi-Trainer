use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Starts or resumes playback of the loaded piece.
#[tauri::command]
pub fn play(engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.start();
    Ok(())
}
