pub mod app_info;
pub mod cancel_soundfont_download;
pub mod connect_midi_input;
pub mod disconnect_midi_input;
pub mod download_soundfont;
pub mod get_settings;
pub mod key_down;
pub mod key_up;
pub mod list_library;
pub mod list_midi_inputs;
pub mod load_soundfont;
pub mod log_message;
pub mod play;
pub mod play_file;
pub mod play_test_note;
pub mod playback_position;
pub mod save_settings;
pub mod seek;
pub mod set_follow;
pub mod set_library_folder;
pub mod set_music_volume;
pub mod set_player_part;
pub mod set_player_volume;
pub mod set_tempo_percent;
pub mod set_track_volume;
pub mod set_transpose;
pub mod soundfont_status;
pub mod stop_playback;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::settings::Settings;

/// The per-user directory the app keeps its data in.
fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}

/// The settings file inside the app data directory.
pub fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("settings.json"))
}

/// Applies a change to the settings and persists it to disk.
pub fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let state = app.state::<Mutex<Settings>>();
    let mut settings = state
        .lock()
        .map_err(|_| "settings state is unavailable".to_string())?;
    change(&mut settings);
    settings.save(&settings_path(app)?)
}

/// Applies a change to the settings in memory only. Discrete events (a file opened, a device
/// chosen) save at once; continuously changing values are saved later by `save_settings`, so
/// dragging a slider does not rewrite the file on every tick.
pub fn change_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let state = app.state::<Mutex<Settings>>();
    let mut settings = state
        .lock()
        .map_err(|_| "settings state is unavailable".to_string())?;
    change(&mut settings);
    Ok(())
}
