use std::sync::Mutex;

use tauri::State;

use crate::audio::AudioEngine;
use crate::engine::matcher::LiveMatcher;

/// Moves the playhead to `position_ms` in the current piece. The matcher rewinds with it, so a
/// replayed section (a rewind or a loop repeat) is scored fresh instead of counting as missed.
#[tauri::command]
pub fn seek(
    position_ms: u64,
    engine: State<'_, Mutex<AudioEngine>>,
    matcher: State<'_, Mutex<Option<LiveMatcher>>>,
) -> Result<(), String> {
    let mut matcher = matcher
        .lock()
        .map_err(|_| "matcher state is unavailable".to_string())?;
    if let Some(matcher) = matcher.as_mut() {
        matcher.restart(position_ms as f64);
    }
    drop(matcher);
    let engine = engine
        .lock()
        .map_err(|_| "audio state is unavailable".to_string())?;
    engine.seek(position_ms);
    Ok(())
}
