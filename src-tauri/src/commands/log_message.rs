/// Receives frontend log lines so UI code never writes to the console directly.
#[tauri::command]
pub fn log_message(level: &str, message: &str) {
    match level {
        "error" => log::error!(target: "frontend", "{}", message),
        "warn" => log::warn!(target: "frontend", "{}", message),
        "info" => log::info!(target: "frontend", "{}", message),
        "debug" => log::debug!(target: "frontend", "{}", message),
        "trace" => log::trace!(target: "frontend", "{}", message),
        other => log::warn!(target: "frontend", "unknown log level {:?}: {}", other, message),
    }
}
