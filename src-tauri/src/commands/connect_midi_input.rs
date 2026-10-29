use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;
use crate::midi::device::{self, MidiInputHandle};

/// Opens a hardware MIDI input device, replacing any current connection.
#[tauri::command]
pub fn connect_midi_input(
    name: String,
    engine: State<'_, Mutex<AudioEngine>>,
    connection: State<'_, Mutex<Option<MidiInputHandle>>>,
) -> Result<(), String> {
    let sink = {
        let engine = engine
            .lock()
            .map_err(|_| "audio state is unavailable".to_string())?;
        engine.input_sink()
    };
    let handle = device::connect(&name, sink).map_err(|err| err.to_string())?;
    let mut connection = connection
        .lock()
        .map_err(|_| "MIDI state is unavailable".to_string())?;
    *connection = Some(handle);
    log::info!("connected MIDI input {name}");
    Ok(())
}
