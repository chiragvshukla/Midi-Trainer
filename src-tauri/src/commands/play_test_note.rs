use std::sync::Mutex;

use tauri::State;

use crate::audio::{AudioCommand, AudioEngine};

/// Plays or stops a note on the default channel, to hear that audio output works.
#[tauri::command]
pub fn play_test_note(
    pitch: u8,
    on: bool,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    let command = if on {
        AudioCommand::NoteOn {
            channel: 0,
            pitch,
            velocity: 100,
        }
    } else {
        AudioCommand::NoteOff { channel: 0, pitch }
    };
    engine.send(command);
    Ok(())
}
