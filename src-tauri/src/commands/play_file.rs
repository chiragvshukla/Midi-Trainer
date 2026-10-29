use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;
use crate::engine::matcher::LiveMatcher;
use crate::engine::{scheduler, timeline::Timeline};

/// One note of the loaded piece, as the falling-note view draws it.
#[derive(Serialize)]
pub struct NoteInfo {
    pub pitch: u8,
    pub start_ms: f64,
    pub end_ms: f64,
}

/// The piece the falling-note view renders: its notes and total length.
#[derive(Serialize)]
pub struct Piece {
    pub notes: Vec<NoteInfo>,
    pub duration_ms: f64,
}

/// Loads a MIDI or karaoke file, plays it from the beginning, and returns its notes.
#[tauri::command]
pub fn play_file(
    path: String,
    app: AppHandle,
    engine: State<'_, Mutex<AudioEngine>>,
    matcher: State<'_, Mutex<Option<LiveMatcher>>>,
) -> Result<Piece, String> {
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
    engine.set_gates(scheduler::gates(&timeline));
    engine.start();
    update_settings(&app, |settings| settings.record_recent_file(&path))?;
    log::info!("playing {path}");

    let notes = timeline
        .notes
        .iter()
        .map(|note| NoteInfo {
            pitch: note.pitch,
            start_ms: note.start_ms,
            end_ms: note.end_ms,
        })
        .collect();
    Ok(Piece {
        duration_ms: timeline.duration_ms,
        notes,
    })
}
