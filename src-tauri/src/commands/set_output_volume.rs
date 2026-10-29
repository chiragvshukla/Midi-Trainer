use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::{AudioEngine, MAX_OUTPUT_VOLUME};
use crate::commands::change_settings;

/// Sets the master output volume, from 0.0 (silent) to [`MAX_OUTPUT_VOLUME`]. Applied to the whole
/// mix on top of the Music and My notes gains.
#[tauri::command]
pub fn set_output_volume(
    volume: f32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let volume = volume.clamp(0.0, MAX_OUTPUT_VOLUME);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_output_volume(volume);
    change_settings(&app, |settings| settings.output_volume = volume)?;
    Ok(())
}
