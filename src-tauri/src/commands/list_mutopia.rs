use crate::mutopia::{self, MutopiaPage};

/// Lists one page of the Mutopia piano catalogue, optionally filtered by a search term.
///
/// The fetch is blocking, so it runs off the main thread. It is only ever called from an
/// explicit user action in the Download tab.
#[tauri::command]
pub async fn list_mutopia(start_at: usize, query: String) -> Result<MutopiaPage, String> {
    tauri::async_runtime::spawn_blocking(move || mutopia::list_piano(start_at, &query))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())
}
