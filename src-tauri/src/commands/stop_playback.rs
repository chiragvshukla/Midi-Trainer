use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Stops playback and silences every sounding note.
#[tauri::command]
pub fn stop_playback(engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.stop();
    Ok(())
}
