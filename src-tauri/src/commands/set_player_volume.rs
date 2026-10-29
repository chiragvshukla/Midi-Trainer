use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::change_settings;

/// Sets the volume of the player's own notes, from 0.0 (silent) to 1.0 (full).
#[tauri::command]
pub fn set_player_volume(
    volume: f32,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let volume = volume.clamp(0.0, 1.0);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_player_volume(volume);
    change_settings(&app, |settings| settings.player_volume = volume)?;
    Ok(())
}
