use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;

/// Sets the output volume, from 0.0 (silent) to 1.0 (full).
#[tauri::command]
pub fn set_volume(
    volume: f32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_volume(volume);
    update_settings(&app, |settings| settings.volume = volume)?;
    Ok(())
}
