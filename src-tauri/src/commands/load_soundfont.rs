use std::path::Path;
use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Loads a `.sf2` SoundFont and switches the audio engine to it.
#[tauri::command]
pub fn load_soundfont(path: String, engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let mut engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine
        .load_soundfont(Path::new(&path))
        .map_err(|err| err.to_string())?;
    log::info!("loaded SoundFont from {path}");
    Ok(())
}
