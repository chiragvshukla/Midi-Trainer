use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::audio::AudioEngine;
use crate::commands::update_settings;
use crate::engine::matcher::LiveMatcher;
use crate::engine::notation;
use crate::engine::{
    midi, scheduler,
    timeline::{self, Timeline},
};
use crate::settings::Settings;

/// One note of the loaded piece, as the falling-note view draws it.
#[derive(Serialize)]
pub struct NoteInfo {
    pub pitch: u8,
    pub start_ms: f64,
    pub end_ms: f64,
}

/// The piece the views render: the falling notes, the total length, and the engraved score.
#[derive(Serialize)]
pub struct Piece {
    pub notes: Vec<NoteInfo>,
    pub duration_ms: f64,
    pub score: notation::Score,
    /// Every track in the file, so the picker can offer all of them; `notes` is already filtered
    /// down to the ones selected.
    pub tracks: Vec<midi::TrackInfo>,
    /// Karaoke lyric syllables, in time order; empty for files without lyrics.
    pub lyrics: Vec<timeline::LyricAt>,
}

/// Loads a MIDI or karaoke file at the beginning and returns its notes. Playback does not
/// start: like a stopped piece, the playhead sits at tick zero until the user presses play.
/// `tracks`, when given, keeps only those track indices; `None` keeps every track.
#[tauri::command]
pub fn play_file(
    path: String,
    tracks: Option<Vec<u16>>,
    app: AppHandle,
    settings: State<'_, Mutex<Settings>>,
    engine: State<'_, Mutex<AudioEngine>>,
    matcher: State<'_, Mutex<Option<LiveMatcher>>>,
) -> Result<Piece, String> {
    // Read once and parse once: the timeline and the engraved score come from the same file.
    let data = std::fs::read(&path).map_err(|err| err.to_string())?;
    let mut file = midi::parse(&data).map_err(|err| err.to_string())?;
    // Drop unchecked tracks before anything is built, so the views, the sheet, the matcher and
    // the playback schedule all see one consistent set of notes.
    if let Some(tracks) = &tracks {
        file.notes.retain(|note| tracks.contains(&note.track));
    }
    let timeline = Timeline::from_midi(&file);
    let (part, follow, app_play) = {
        let settings = settings
            .lock()
            .map_err(|_| "settings state is unavailable".to_string())?;
        (settings.part(), settings.follow(), settings.app_play())
    };
    let score = Arc::new(scheduler::schedule(&timeline, part, app_play));
    {
        let mut matcher = matcher
            .lock()
            .map_err(|_| "matcher state is unavailable".to_string())?;
        *matcher = Some(LiveMatcher::for_timeline(&timeline, part));
    }
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.set_score(score);
    engine.set_gates(scheduler::gates(&timeline, part));
    // A newly loaded piece starts in the saved skill's waiting mode.
    engine.set_follow(follow);
    // Loading must not start playing, and must silence a piece that is still going.
    engine.stop();
    update_settings(&app, |settings| settings.record_recent_file(&path))?;
    log::info!("loaded {path}");

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
        tracks: file.tracks.clone(),
        lyrics: timeline.lyrics.clone(),
        score: notation::engrave(&file, &timeline),
    })
}
