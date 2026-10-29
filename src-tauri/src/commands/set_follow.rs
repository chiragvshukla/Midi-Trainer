use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Turns follow-you mode on or off: the accompaniment waits for the player at each note.
#[tauri::command]
pub fn set_follow(follow: bool, engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_follow(follow);
    Ok(())
}
