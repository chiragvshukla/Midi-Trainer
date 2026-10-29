use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

/// Asks an in-flight SoundFont download to stop.
#[tauri::command]
pub fn cancel_soundfont_download(cancelled: State<'_, Arc<AtomicBool>>) {
    cancelled.store(true, Ordering::Relaxed);
}
