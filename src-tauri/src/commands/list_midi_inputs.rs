use crate::midi::device;

/// The names of the currently connected hardware MIDI input devices.
#[tauri::command]
pub fn list_midi_inputs() -> Result<Vec<String>, String> {
    device::input_ports().map_err(|err| err.to_string())
}
