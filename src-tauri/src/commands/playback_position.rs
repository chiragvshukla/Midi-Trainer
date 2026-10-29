use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// The current playhead position in the piece, in milliseconds.
#[tauri::command]
pub fn playback_position(engine: State<'_, Mutex<AudioEngine>>) -> Result<u64, String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    Ok(engine.position_ms())
}
