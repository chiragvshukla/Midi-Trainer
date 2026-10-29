use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::State;

use crate::audio::AudioEngine;
use crate::engine::matcher::LiveMatcher;
use crate::engine::{scheduler, timeline::Timeline};

/// Loads a MIDI or karaoke file and plays it from the beginning.
#[tauri::command]
pub fn play_file(
    path: String,
    engine: State<'_, Mutex<AudioEngine>>,
    matcher: State<'_, Mutex<Option<LiveMatcher>>>,
) -> Result<(), String> {
    let timeline = Timeline::load(Path::new(&path)).map_err(|err| err.to_string())?;
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
    engine.start();
    log::info!("playing {path}");
    Ok(())
}
