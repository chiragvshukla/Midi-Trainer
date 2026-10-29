use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Moves the playhead to `position_ms` in the current piece.
#[tauri::command]
pub fn seek(position_ms: u64, engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.seek(position_ms);
    Ok(())
}
