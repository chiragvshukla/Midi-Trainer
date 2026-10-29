use std::path::Path;

use crate::mutopia;

/// Downloads a Mutopia MIDI into `dest` under `name`, returning the saved file's path.
///
/// The download is blocking, so it runs off the main thread. `url` and `name` are validated
/// in [`mutopia::download_midi`] before anything is fetched or written.
#[tauri::command]
pub async fn download_mutopia(
    url: String,
    dest: String,
    name: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mutopia::download_midi(&url, Path::new(&dest), name.as_deref())
    })
    .await
    .map_err(|err| err.to_string())?
    .map(|path| path.to_string_lossy().into_owned())
    .map_err(|err| err.to_string())
}
