use std::sync::Mutex;

use tauri::{AppHandle, State};

use crate::commands::update_settings;
use crate::midi::device::MidiInputHandle;

/// Closes the current hardware MIDI input connection, if any.
#[tauri::command]
pub fn disconnect_midi_input(
    app: AppHandle,
    connection: State<'_, Mutex<Option<MidiInputHandle>>>,
) -> Result<(), String> {
    let mut connection = connection
        .lock()
        .map_err(|_| "MIDI state is unavailable".to_string())?;
    *connection = None;
    update_settings(&app, |settings| settings.midi_device = None)?;
    Ok(())
}
