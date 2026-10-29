use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Shifts playback up (positive) or down (negative) by up to 12 semitones.
#[tauri::command]
pub fn set_transpose(semitones: i32, engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_transpose(semitones);
    Ok(())
}
