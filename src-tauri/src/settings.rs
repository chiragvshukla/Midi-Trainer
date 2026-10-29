//! The user's practice settings, persisted as JSON in the app data directory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::scheduler::{AppPlay, Hand, Part};

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

/// Reads `library_folders` from the new array, or the old singular string or null, so a
/// settings file written before folders became a list still loads.
fn folders_from_json<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Stored {
        Many(Vec<String>),
        One(String),
        None,
    }
    Ok(match Stored::deserialize(deserializer)? {
        Stored::Many(folders) => folders,
        Stored::One(folder) => vec![folder],
        Stored::None => Vec::new(),
    })
}

/// Everything the app remembers between runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Most recently played files, newest first.
    pub recent_files: Vec<String>,
    /// The folders the user browses as their MIDI library.
    #[serde(alias = "library_folder", deserialize_with = "folders_from_json")]
    pub library_folders: Vec<String>,
    /// The hardware MIDI input to reconnect to, if one was chosen.
    pub midi_device: Option<String>,
    /// Volume of the piece's own parts, as a linear gain, 0.0 to 1.0.
    pub music_volume: f32,
    /// Volume of the player's own notes, as a linear gain, 0.0 to 1.0.
    pub player_volume: f32,
    /// Volume of the wrong-note cue, as a linear gain, 0.0 to 1.0.
    pub cue_volume: f32,
    /// Master output volume, as a linear gain, 0.0 to 1.0, applied to the whole mix.
    pub output_volume: f32,
    /// Playback tempo as a percentage of the original.
    pub tempo_percent: f32,
    /// Semitones the playback is shifted by.
    pub transpose: i32,
    /// The hand the player takes in one-hand practice; the app plays the other.
    pub player_hand: Hand,
    /// A track the player takes over for play-along, overriding the hand split. `None` uses
    /// `player_hand`.
    pub player_track: Option<u16>,
    /// Whether a one-track piece offers hand practice, split at middle C.
    pub hand_split: bool,
    /// In Follow mode with one hand chosen, whether the app also plays the other hand. Off
    /// leaves the player alone with the waiting.
    pub play_along: bool,
    /// How strictly the player's notes are judged.
    pub skill: Skill,
    /// A `.sf2` the user chose, or the downloaded full bank; `None` uses the bundled baseline.
    pub soundfont_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            recent_files: Vec::new(),
            library_folders: Vec::new(),
            midi_device: None,
            music_volume: 0.8,
            player_volume: 0.5,
            cue_volume: 1.0,
            output_volume: 1.0,
            tempo_percent: 100.0,
            transpose: 0,
            player_hand: Hand::Both,
            player_track: None,
            hand_split: false,
            play_along: false,
            skill: Skill::Strict,
            soundfont_path: None,
        }
    }
}

impl Settings {
    /// The part the player takes: a chosen track, or the hand split. Without hand practice, the
    /// player plays everything.
    pub fn part(&self) -> Part {
        match self.player_track {
            Some(track) => Part::Track(track),
            None if self.hand_split => Part::Hands(self.player_hand),
            None => Part::Hands(Hand::Both),
        }
    }

    /// Whether the accompaniment waits for the player at each note.
    pub fn follow(&self) -> bool {
        self.skill == Skill::Follow
    }

    /// What the app sounds under the player. With one hand taken, Listen and Strict play that
    /// hand back as a guide; Follow goes silent and waits, unless play-along adds the other
    /// hand. Both hands and a chosen track keep the ordinary accompaniment.
    pub fn app_play(&self) -> AppPlay {
        match self.part() {
            Part::Hands(hand) if hand != Hand::Both => match self.skill {
                Skill::Follow if !self.play_along => AppPlay::Silent,
                Skill::Follow => AppPlay::Complement,
                _ => AppPlay::PlayerHand,
            },
            _ => AppPlay::Complement,
        }
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
        // Rename replaces the destination atomically on every supported platform, so the old
        // file survives a crash right up to the swap. Windows can still refuse while a scanner
        // holds the file, so a failure removes it and retries instead of losing the write.
        for attempt in 0..3 {
            match std::fs::rename(&temporary, path) {
                Ok(()) => return Ok(()),
                Err(err) if attempt == 2 => {
                    let _ = std::fs::remove_file(&temporary);
                    return Err(err.to_string());
                }
                Err(_) => {
                    std::fs::remove_file(path).ok();
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
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
        settings.cue_volume = 0.75;
        settings.tempo_percent = 80.0;
        settings.transpose = -2;
        settings.midi_device = Some("Piano".to_string());
        settings.library_folders = vec!["C:/music".to_string(), "D:/midi".to_string()];
        settings.player_hand = super::Hand::Left;
        settings.player_track = Some(2);
        settings.hand_split = true;
        settings.skill = super::Skill::Follow;
        settings.soundfont_path = Some("C:/fonts/custom.sf2".to_string());
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
    fn a_legacy_single_folder_migrates_into_the_list() {
        let dir = std::env::temp_dir().join("mt-settings-migrate-test");
        std::fs::create_dir_all(&dir).expect("the directory should create");

        // An old file stored one folder under `library_folder`; a chosen-none file stored null.
        let path = dir.join("old.json");
        std::fs::write(&path, r#"{"library_folder":"C:/music"}"#).unwrap();
        assert_eq!(Settings::load(&path).library_folders, vec!["C:/music"]);

        std::fs::write(&path, r#"{"library_folder":null}"#).unwrap();
        assert!(Settings::load(&path).library_folders.is_empty());

        // The new shape loads as-is.
        std::fs::write(&path, r#"{"library_folders":["A","B"]}"#).unwrap();
        assert_eq!(Settings::load(&path).library_folders, vec!["A", "B"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_part_needs_hand_practice_and_prefers_a_track() {
        use crate::engine::scheduler::{Hand, Part};

        let mut settings = Settings {
            player_hand: Hand::Left,
            ..Settings::default()
        };
        // Without hand practice, the player plays everything, whatever hand was last chosen.
        assert_eq!(settings.part(), Part::Hands(Hand::Both));

        settings.hand_split = true;
        assert_eq!(settings.part(), Part::Hands(Hand::Left));

        // A chosen track overrides the hand split.
        settings.player_track = Some(3);
        assert_eq!(settings.part(), Part::Track(3));
    }

    #[test]
    fn app_play_follows_the_hand_and_mode() {
        use crate::engine::scheduler::{AppPlay, Hand, Part};

        let mut settings = Settings::default();
        // Both hands and a chosen track keep the ordinary accompaniment.
        assert_eq!(settings.app_play(), AppPlay::Complement);
        settings.player_track = Some(1);
        assert_eq!(settings.app_play(), AppPlay::Complement);

        // One hand taken: Listen and Strict play that hand back as a guide.
        settings.player_track = None;
        settings.hand_split = true;
        settings.player_hand = Hand::Left;
        settings.skill = super::Skill::Listen;
        assert_eq!(settings.app_play(), AppPlay::PlayerHand);
        settings.skill = super::Skill::Strict;
        assert_eq!(settings.app_play(), AppPlay::PlayerHand);

        // Follow goes silent, unless play-along adds the other hand.
        settings.skill = super::Skill::Follow;
        assert_eq!(settings.app_play(), AppPlay::Silent);
        settings.play_along = true;
        assert_eq!(settings.app_play(), AppPlay::Complement);

        // Without hand practice the player plays everything, so it is the ordinary accompaniment.
        settings.hand_split = false;
        assert_eq!(settings.part(), Part::Hands(Hand::Both));
        assert_eq!(settings.app_play(), AppPlay::Complement);
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
