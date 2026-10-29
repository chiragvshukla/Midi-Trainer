use std::sync::Mutex;

use tauri::State;

use crate::audio::{AudioCommand, AudioEngine, PLAYER_CHANNEL};
use crate::midi::keyboard::{KEY_VELOCITY, pitch_for};

/// Plays a note for a pressed PC key and returns its pitch, if that key maps to one.
#[tauri::command]
pub fn key_down(code: String, engine: State<'_, Mutex<AudioEngine>>) -> Result<Option<u8>, String> {
    let Some(pitch) = pitch_for(&code) else {
        return Ok(None);
    };
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.send(AudioCommand::NoteOn {
        channel: PLAYER_CHANNEL,
        pitch,
        velocity: KEY_VELOCITY,
    });
    Ok(Some(pitch))
}
