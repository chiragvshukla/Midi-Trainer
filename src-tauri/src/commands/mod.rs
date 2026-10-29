pub mod app_info;
pub mod cancel_soundfont_download;
pub mod connect_midi_input;
pub mod disconnect_midi_input;
pub mod download_soundfont;
pub mod key_down;
pub mod key_up;
pub mod list_midi_inputs;
pub mod load_soundfont;
pub mod log_message;
pub mod play_file;
pub mod play_test_note;
pub mod playback_position;
pub mod seek;
pub mod set_follow;
pub mod set_tempo_percent;
pub mod set_transpose;
pub mod soundfont_status;
pub mod stop_playback;

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// The per-user directory the app keeps its data in.
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}
