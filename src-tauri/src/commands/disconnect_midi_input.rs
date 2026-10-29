use std::sync::Mutex;

use tauri::State;

use crate::midi::device::MidiInputHandle;

/// Closes the current hardware MIDI input connection, if any.
#[tauri::command]
pub fn disconnect_midi_input(
    connection: State<'_, Mutex<Option<MidiInputHandle>>>,
) -> Result<(), String> {
    let mut connection = connection
        .lock()
        .map_err(|_| "MIDI state is unavailable".to_string())?;
    *connection = None;
    Ok(())
}
