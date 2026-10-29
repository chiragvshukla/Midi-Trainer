pub mod app_info;
pub mod cancel_soundfont_download;
pub mod download_soundfont;
pub mod load_soundfont;
pub mod log_message;
pub mod play_file;
pub mod play_test_note;
pub mod soundfont_status;
pub mod stop_playback;

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// The per-user directory the app keeps its data in.
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}
