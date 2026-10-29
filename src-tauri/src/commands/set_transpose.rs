use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;

/// Shifts playback up (positive) or down (negative) by up to 12 semitones.
#[tauri::command]
pub fn set_transpose(
    semitones: i32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let semitones = semitones.clamp(-12, 12);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_transpose(semitones);
    update_settings(&app, |settings| settings.transpose = semitones)?;
    Ok(())
}
