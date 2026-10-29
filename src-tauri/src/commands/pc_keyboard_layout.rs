use serde::Serialize;

use crate::midi::keyboard::LAYOUT;

/// One key of the PC-keyboard fallback: the browser key code and the MIDI pitch it plays.
#[derive(Serialize)]
pub struct PcKey {
    pub code: String,
    pub pitch: u8,
}

/// The PC-keyboard note layout, so the UI can show it without duplicating the table.
#[tauri::command]
pub fn pc_keyboard_layout() -> Vec<PcKey> {
    LAYOUT
        .iter()
        .map(|(code, pitch)| PcKey {
            code: (*code).to_string(),
            pitch: *pitch,
        })
        .collect()
}
