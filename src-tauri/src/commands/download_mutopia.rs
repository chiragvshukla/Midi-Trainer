use std::path::Path;

use crate::mutopia;

/// Downloads a Mutopia MIDI into `dest`, returning the saved file's path.
///
/// The download is blocking, so it runs off the main thread. `url` is validated in
/// [`mutopia::download_midi`] before anything is fetched.
#[tauri::command]
pub async fn download_mutopia(url: String, dest: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || mutopia::download_midi(&url, Path::new(&dest)))
        .await
        .map_err(|err| err.to_string())?
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|err| err.to_string())
}
