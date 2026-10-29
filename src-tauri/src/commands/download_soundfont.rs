use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::soundfont;

use super::app_data_dir;

/// Event name the frontend listens on for download progress.
pub const PROGRESS_EVENT: &str = "soundfont-progress";

/// The running and total byte counts sent with each progress event.
#[derive(Clone, Serialize)]
struct DownloadProgress {
    received: u64,
    total: u64,
}

/// Downloads the baseline SoundFont into app data, reporting progress, and returns its path.
///
/// The blocking download runs off the main thread; the frontend tracks it through
/// [`PROGRESS_EVENT`] and can abort it with `cancel_soundfont_download`.
#[tauri::command]
pub async fn download_soundfont(
    app: AppHandle,
    cancelled: State<'_, Arc<AtomicBool>>,
) -> Result<String, String> {
    let directory = app_data_dir(&app)?;
    let cancelled = Arc::clone(cancelled.inner());
    cancelled.store(false, Ordering::Relaxed);
    let emitter = app.clone();

    let path = tauri::async_runtime::spawn_blocking(move || {
        soundfont::download(&directory, &cancelled, &mut |received, total| {
            let _ = emitter.emit(PROGRESS_EVENT, DownloadProgress { received, total });
        })
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())?;

    Ok(path.to_string_lossy().into_owned())
}
