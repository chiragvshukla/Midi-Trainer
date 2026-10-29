pub mod audio;
mod commands;
pub mod engine;

use std::sync::Mutex;

use log::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .manage(Mutex::new(audio::AudioEngine::new()))
        .invoke_handler(tauri::generate_handler![
            commands::app_info::app_info,
            commands::log_message::log_message,
            commands::load_soundfont::load_soundfont,
            commands::play_file::play_file,
            commands::play_test_note::play_test_note,
            commands::stop_playback::stop_playback,
        ])
        .setup(|_app| {
            info!("MidiTrainer {} starting", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
