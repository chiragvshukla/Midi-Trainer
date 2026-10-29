pub mod audio;
mod commands;
pub mod engine;
pub mod midi;
pub mod soundfont;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use log::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .manage(Mutex::new(audio::AudioEngine::new()))
        // Shared flag the frontend flips to cancel an in-flight SoundFont download.
        .manage(Arc::new(AtomicBool::new(false)))
        // The open hardware MIDI input, if any.
        .manage(Mutex::new(None::<midi::device::MidiInputHandle>))
        .invoke_handler(tauri::generate_handler![
            commands::app_info::app_info,
            commands::log_message::log_message,
            commands::load_soundfont::load_soundfont,
            commands::play_file::play_file,
            commands::play_test_note::play_test_note,
            commands::stop_playback::stop_playback,
            commands::soundfont_status::soundfont_status,
            commands::download_soundfont::download_soundfont,
            commands::cancel_soundfont_download::cancel_soundfont_download,
            commands::key_down::key_down,
            commands::key_up::key_up,
            commands::list_midi_inputs::list_midi_inputs,
            commands::connect_midi_input::connect_midi_input,
            commands::disconnect_midi_input::disconnect_midi_input,
            commands::set_tempo_percent::set_tempo_percent,
            commands::set_transpose::set_transpose,
            commands::seek::seek,
            commands::playback_position::playback_position,
        ])
        .setup(|_app| {
            info!("MidiTrainer {} starting", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
