use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;
use crate::engine::matcher::LiveMatcher;
use crate::engine::{scheduler, timeline::Timeline};

/// Loads a MIDI or karaoke file and plays it from the beginning.
#[tauri::command]
pub fn play_file(
    path: String,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
    matcher: State<'_, Mutex<Option<LiveMatcher>>>,
) -> Result<(), String> {
    let file = Path::new(&path);
    let timeline = Timeline::load(file).map_err(|err| err.to_string())?;
    let score = Arc::new(scheduler::schedule(&timeline));
    {
        let mut matcher = matcher
            .lock()
            .map_err(|_| "matcher state is unavailable".to_string())?;
        *matcher = Some(LiveMatcher::for_timeline(&timeline));
    }
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_score(score);
    engine.set_gates(scheduler::gates(&timeline));
    engine.start();
    update_settings(&app, |settings| settings.record_recent_file(&path))?;
    log::info!("playing {path}");
    Ok(())
}
