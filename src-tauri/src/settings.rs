//! The user's practice settings, persisted as JSON in the app data directory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::scheduler::{Hand, Part};

/// How strictly the app judges the player's notes, from a plain play-along to a scored test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Skill {
    /// Playback only: the player's notes are neither judged nor cued.
    Listen,
    /// The accompaniment waits when the player falls behind; every note is judged.
    Follow,
    /// Playback never waits; every note is judged.
    Strict,
}

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
    /// Volume of the piece's own parts, as a linear gain, 0.0 to 1.0.
    pub music_volume: f32,
    /// Volume of the player's own notes, as a linear gain, 0.0 to 1.0.
    pub player_volume: f32,
    /// Playback tempo as a percentage of the original.
    pub tempo_percent: f32,
    /// Semitones the playback is shifted by.
    pub transpose: i32,
    /// The hand the player takes in one-hand practice; the app plays the other.
    pub player_hand: Hand,
    /// A track the player takes over for play-along, overriding the hand split. `None` uses
    /// `player_hand`.
    pub player_track: Option<u16>,
    /// How strictly the player's notes are judged.
    pub skill: Skill,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            recent_files: Vec::new(),
            library_folder: None,
            midi_device: None,
            music_volume: 1.0,
            player_volume: 1.0,
            tempo_percent: 100.0,
            transpose: 0,
            player_hand: Hand::Both,
            player_track: None,
            skill: Skill::Strict,
        }
    }
}

impl Settings {
    /// The part the player takes: a chosen track, or the hand split.
    pub fn part(&self) -> Part {
        self.player_track
            .map_or(Part::Hands(self.player_hand), Part::Track)
    }

    /// Whether the accompaniment waits for the player at each note.
    pub fn follow(&self) -> bool {
        self.skill == Skill::Follow
    }

    /// Whether the player's notes are judged and cued.
    pub fn scored(&self) -> bool {
        self.skill != Skill::Listen
    }

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
        settings.music_volume = 0.5;
        settings.player_volume = 0.25;
        settings.tempo_percent = 80.0;
        settings.transpose = -2;
        settings.midi_device = Some("Piano".to_string());
        settings.library_folder = Some("C:/music".to_string());
        settings.player_hand = super::Hand::Left;
        settings.player_track = Some(2);
        settings.skill = super::Skill::Follow;
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
    fn skill_selects_waiting_and_scoring() {
        let mut settings = Settings::default();
        // Strict is the default: judged, without waiting.
        assert!(settings.scored());
        assert!(!settings.follow());

        settings.skill = super::Skill::Follow;
        assert!(settings.scored());
        assert!(settings.follow());

        settings.skill = super::Skill::Listen;
        assert!(!settings.scored());
        assert!(!settings.follow());
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
