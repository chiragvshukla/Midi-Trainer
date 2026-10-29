use std::sync::Mutex;

use tauri::State;

use crate::audio::{AudioCommand, AudioEngine, PLAYER_CHANNEL};
use crate::midi::keyboard::pitch_for;

/// Stops the note for a released PC key, if that key maps to one.
#[tauri::command]
pub fn key_up(code: String, engine: State<'_, Mutex<AudioEngine>>) -> Result<(), String> {
    let Some(pitch) = pitch_for(&code) else {
        return Ok(());
    };
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.send(AudioCommand::NoteOff {
        channel: PLAYER_CHANNEL,
        pitch,
    });
    Ok(())
}
