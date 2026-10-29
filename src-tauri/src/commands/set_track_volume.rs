use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;

/// Sets one track's volume by its MIDI channel, from 0.0 (silent) to 1.0 (full). Session only:
/// the mixer is not persisted, since a channel means a different part in each piece.
#[tauri::command]
pub fn set_track_volume(
    channel: u8,
    gain: f32,
    engine: State<'_, Mutex<AudioEngine>>,
) -> Result<(), String> {
    let gain = gain.clamp(0.0, 1.0);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_track_volume(channel, gain);
    Ok(())
}
