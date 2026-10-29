pub mod audio;
mod commands;
pub mod engine;
pub mod midi;
pub mod soundfont;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use log::info;
use tauri::{AppHandle, Emitter, Manager};

use audio::AudioEngine;
use engine::matcher::LiveMatcher;
use engine::scoring::Verdict;

/// How often the judgement thread looks for notes the player has pressed.
const JUDGEMENT_POLL: Duration = Duration::from_millis(33);

/// Event carrying the judgements of the player's notes to the frontend.
pub const NOTE_JUDGEMENTS_EVENT: &str = "note-judgements";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .manage(Mutex::new(AudioEngine::new()))
        // Shared flag the frontend flips to cancel an in-flight SoundFont download.
        .manage(Arc::new(AtomicBool::new(false)))
        // The open hardware MIDI input, if any.
        .manage(Mutex::new(None::<midi::device::MidiInputHandle>))
        // The live matcher for the piece being practised, if one is loaded.
        .manage(Mutex::new(None::<LiveMatcher>))
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
        .setup(|app| {
            info!("MidiTrainer {} starting", env!("CARGO_PKG_VERSION"));
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("judgements".to_string())
                .spawn(move || judgement_pump(handle))
                .expect("the judgement thread should start");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Emits the notes the player pressed as judgement events for the frontend.
fn judgement_pump(app: AppHandle) {
    loop {
        std::thread::sleep(JUDGEMENT_POLL);
        let played = app
            .state::<Mutex<AudioEngine>>()
            .lock()
            .map(|engine| engine.take_played())
            .unwrap_or_default();
        if played.is_empty() {
            continue;
        }

        let mut judgements = Vec::new();
        let mut wrong = false;
        {
            let state = app.state::<Mutex<Option<LiveMatcher>>>();
            let Ok(mut guard) = state.lock() else {
                continue;
            };
            let Some(matcher) = guard.as_mut() else {
                continue;
            };
            for note in played {
                if let Some(judgement) = matcher.evaluate(note.position_ms, note.pitch) {
                    wrong |= judgement.verdict == Verdict::Wrong;
                    judgements.push(judgement);
                }
            }
        }
        if wrong && let Ok(engine) = app.state::<Mutex<AudioEngine>>().lock() {
            engine.play_wrong_note();
        }
        if !judgements.is_empty() {
            let _ = app.emit(NOTE_JUDGEMENTS_EVENT, judgements);
        }
    }
}
