//! The user's practice settings, persisted as JSON in the app data directory.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// How many recent files are kept.
const MAX_RECENT_FILES: usize = 10;

/// Everything the app remembers between runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Most recently played files, newest first.
    pub recent_files: Vec<String>,
    /// The folder the user browses as their MIDI library, if one was chosen.
    pub library_folder: Option<String>,
    /// The hardware MIDI input to reconnect to, if one was chosen.
    pub midi_device: Option<String>,
    /// Output volume as a linear gain, 0.0 to 1.0.
    pub volume: f32,
    /// Playback tempo as a percentage of the original.
    pub tempo_percent: f32,
    /// Semitones the playback is shifted by.
    pub transpose: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            recent_files: Vec::new(),
            library_folder: None,
            midi_device: None,
            volume: 1.0,
            tempo_percent: 100.0,
            transpose: 0,
        }
    }
}

impl Settings {
    /// Reads the settings file, falling back to defaults when it is missing or unreadable.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => settings,
                Err(err) => {
                    log::warn!(
                        "settings file {} is unreadable ({err}); using defaults",
                        path.display()
                    );
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    /// Writes the settings file, creating the directory when needed. The write goes to a
    /// temporary file first, so a crash mid-write never leaves a half-written file behind.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        let temporary = path.with_extension("json.part");
        std::fs::write(&temporary, text).map_err(|err| err.to_string())?;
        // Windows refuses to rename over an existing file, and freshly written files can be
        // held briefly by antivirus scans, so the swap is retried a few times.
        for attempt in 0..3 {
            std::fs::remove_file(path).ok();
            match std::fs::rename(&temporary, path) {
                Ok(()) => return Ok(()),
                Err(err) if attempt == 2 => return Err(err.to_string()),
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        unreachable!("the loop returns on its last attempt")
    }

    /// Adds a file to the recent list, moving an existing entry to the front.
    pub fn record_recent_file(&mut self, path: &str) {
        self.recent_files.retain(|entry| entry != path);
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(MAX_RECENT_FILES);
    }
}

#[cfg(test)]
mod tests {
    use super::Settings;

    #[test]
    fn a_missing_file_loads_as_defaults() {
        assert_eq!(
            Settings::load(std::path::Path::new("Z:/definitely-missing/settings.json")),
            Settings::default()
        );
    }

    #[test]
    fn settings_round_trip_through_a_file() {
        // A directory of its own: the corrupt-file test below runs in parallel and must
        // not have this directory deleted out from under it.
        let dir = std::env::temp_dir().join("mt-settings-round-trip-test");
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        settings.record_recent_file("C:/music/sonata.mid");
        settings.volume = 0.5;
        settings.tempo_percent = 80.0;
        settings.transpose = -2;
        settings.midi_device = Some("Piano".to_string());
        settings.library_folder = Some("C:/music".to_string());
        settings.save(&path).expect("the file should write");
        // A second save replaces the first file, it does not fail on it.
        settings.recent_files.clear();
        settings.save(&path).expect("the overwrite should write");

        assert_eq!(Settings::load(&path), settings);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_file_loads_as_defaults() {
        let dir = std::env::temp_dir().join("mt-settings-corrupt-test");
        std::fs::create_dir_all(&dir).expect("the directory should create");
        let path = dir.join("corrupt.json");
        std::fs::write(&path, "{not json").expect("the file should write");

        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recent_files_deduplicate_and_are_capped() {
        let mut settings = Settings::default();
        settings.record_recent_file("a.mid");
        settings.record_recent_file("b.mid");
        settings.record_recent_file("a.mid");

        assert_eq!(settings.recent_files, vec!["a.mid", "b.mid"]);
    }
}
