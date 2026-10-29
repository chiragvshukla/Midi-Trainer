use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::audio::{AudioCommand, Gate, PERCUSSION_CHANNEL, ScheduledCommand};

use super::timeline::{Note, Timeline};
use super::voices::{Staff, staff_of};

/// Which hand the player takes in one-hand practice; the app plays the other. `Both` is the
/// default: the app plays the whole piece as a guide and the player may play along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hand {
    Both,
    Left,
    Right,
}

/// The part the player takes over: one hand, or a whole track for play-along on any instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Hands(Hand),
    Track(u16),
}

/// What the app sounds under the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPlay {
    /// Everything the player is not taking; the ordinary accompaniment.
    Complement,
    /// Exactly the player's hand, as a guide they can play along with.
    PlayerHand,
    /// Nothing; the app only waits for the player.
    Silent,
}

/// Whether the player is expected to play this note. Drums are always the app's.
pub fn player_plays(note: &Note, part: Part) -> bool {
    if note.channel == PERCUSSION_CHANNEL {
        return false;
    }
    match part {
        Part::Hands(hand) => hand_plays(note.pitch, hand),
        Part::Track(index) => note.track == index,
    }
}

/// Whether the app should sound this note. Drums are never the player's, so they always sound.
fn app_plays(note: &Note, part: Part, app: AppPlay) -> bool {
    if note.channel == PERCUSSION_CHANNEL {
        return true;
    }
    match app {
        AppPlay::Silent => false,
        AppPlay::PlayerHand => player_plays(note, part),
        AppPlay::Complement => match part {
            Part::Hands(hand) => hand_app_plays(note.pitch, hand),
            Part::Track(index) => note.track != index,
        },
    }
}

/// Whether one hand's worth of notes covers this pitch.
fn hand_plays(pitch: u8, hand: Hand) -> bool {
    match hand {
        Hand::Both => true,
        Hand::Right => staff_of(pitch) == Staff::Treble,
        Hand::Left => staff_of(pitch) == Staff::Bass,
    }
}

/// Whether the app plays this pitch when the player takes one hand. In `Both` the app plays
/// everything; taking one hand silences that hand so only the player's part is missing.
fn hand_app_plays(pitch: u8, hand: Hand) -> bool {
    match hand {
        Hand::Both => true,
        Hand::Right => staff_of(pitch) == Staff::Bass,
        Hand::Left => staff_of(pitch) == Staff::Treble,
    }
}

/// Flattens a timeline into one time-sorted command stream: a program change per track, then
/// the note-ons and note-offs of every note the app plays. The playback scheduler applies
/// these in order. Drums are always played by the app.
pub fn schedule(timeline: &Timeline, part: Part, app: AppPlay) -> Vec<ScheduledCommand> {
    let mut events = Vec::with_capacity(timeline.notes.len() * 2 + timeline.tracks.len());

    // Set each track's instrument on the channel its notes use, before the first note.
    // A track's notes almost always share one channel; the first note decides which.
    let mut channel_of_track: HashMap<u16, u8> = HashMap::new();
    for note in &timeline.notes {
        channel_of_track.entry(note.track).or_insert(note.channel);
    }
    for track in &timeline.tracks {
        let (Some(program), Some(&channel)) = (track.program, channel_of_track.get(&track.index))
        else {
            continue;
        };
        events.push(ScheduledCommand {
            at_ms: 0.0,
            command: AudioCommand::ProgramChange { channel, program },
        });
    }

    for note in &timeline.notes {
        if !app_plays(note, part, app) {
            continue;
        }
        events.push(ScheduledCommand {
            at_ms: note.start_ms,
            command: AudioCommand::NoteOn {
                channel: note.channel,
                pitch: note.pitch,
                velocity: note.velocity,
            },
        });
        events.push(ScheduledCommand {
            at_ms: note.end_ms,
            command: AudioCommand::NoteOff {
                channel: note.channel,
                pitch: note.pitch,
            },
        });
    }

    events.sort_by(|a, b| a.at_ms.total_cmp(&b.at_ms));
    events
}

/// The notes the player must play, in time order, for follow-you mode: the part the player
/// took, everything but the drums otherwise. The app plays the percussion itself, so drums
/// never hold the playhead.
pub fn gates(timeline: &Timeline, part: Part) -> Arc<Vec<Gate>> {
    Arc::new(
        timeline
            .notes
            .iter()
            .filter(|note| player_plays(note, part))
            .map(|note| Gate {
                pitch: note.pitch,
                start_ms: note.start_ms,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{AppPlay, Hand, Part, gates, schedule};
    use crate::audio::AudioCommand;
    use crate::engine::midi::{Division, MidiFile, NoteEvent, TempoChange, TrackInfo};
    use crate::engine::timeline::Timeline;

    #[test]
    fn gates_list_every_played_note_in_time_order() {
        // Notes given out of order, at distinct times. The percussion note is played by the app
        // and must not become a gate.
        let notes = vec![
            note(9, 9, 38, 720, 960),
            note(0, 0, 64, 480, 960),
            note(0, 0, 60, 0, 480),
        ];
        let tracks = vec![TrackInfo {
            index: 0,
            name: None,
            program: None,
            channel: None,
            note_count: 2,
        }];
        let timeline = Timeline::from_midi(&MidiFile {
            format: 1,
            division: Division::Metrical(480),
            tempo_map: vec![TempoChange {
                tick: 0,
                micros_per_quarter: 500_000,
            }],
            time_signatures: Vec::new(),
            key_signatures: Vec::new(),
            lyrics: Vec::new(),
            tracks,
            notes,
        });

        let gates = gates(&timeline, Part::Hands(Hand::Both));

        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0].pitch, 60);
        assert!((gates[0].start_ms - 0.0).abs() < 1e-6);
        assert_eq!(gates[1].pitch, 64);
        assert!((gates[1].start_ms - 500.0).abs() < 1e-6);
    }

    #[test]
    fn sets_each_tracks_instrument_then_plays_its_notes() {
        let timeline = fixture_timeline();

        let events = schedule(&timeline, Part::Hands(Hand::Both), AppPlay::Complement);

        let commands: Vec<AudioCommand> = events.iter().map(|event| event.command).collect();
        assert_eq!(
            commands,
            vec![
                AudioCommand::ProgramChange {
                    channel: 0,
                    program: 40
                },
                AudioCommand::NoteOn {
                    channel: 9,
                    pitch: 38,
                    velocity: 100
                },
                AudioCommand::NoteOn {
                    channel: 0,
                    pitch: 60,
                    velocity: 100
                },
                AudioCommand::NoteOff {
                    channel: 9,
                    pitch: 38
                },
                AudioCommand::NoteOff {
                    channel: 0,
                    pitch: 60
                },
            ]
        );

        let times: Vec<f64> = events.iter().map(|event| event.at_ms).collect();
        for (actual, expected) in times.iter().zip([0.0, 0.0, 0.0, 250.0, 500.0]) {
            assert!(
                (actual - expected).abs() < 1e-6,
                "expected {expected} ms, got {actual} ms"
            );
        }
    }

    #[test]
    fn playing_one_hand_gates_that_hand_and_sounds_the_other() {
        // One left-hand note (below middle C) and one right-hand note (middle C and up).
        let notes = vec![note(0, 0, 48, 0, 480), note(0, 0, 72, 480, 960)];
        let tracks = vec![TrackInfo {
            index: 0,
            name: None,
            program: Some(0),
            channel: Some(0),
            note_count: 2,
        }];
        let timeline = Timeline::from_midi(&MidiFile {
            format: 1,
            division: Division::Metrical(480),
            tempo_map: vec![TempoChange {
                tick: 0,
                micros_per_quarter: 500_000,
            }],
            time_signatures: Vec::new(),
            key_signatures: Vec::new(),
            lyrics: Vec::new(),
            tracks,
            notes,
        });

        let played = |hand, app| {
            schedule(&timeline, Part::Hands(hand), app)
                .into_iter()
                .filter_map(|event| match event.command {
                    AudioCommand::NoteOn { pitch, .. } => Some(pitch),
                    _ => None,
                })
                .collect::<Vec<u8>>()
        };
        let gated = |hand| {
            gates(&timeline, Part::Hands(hand))
                .iter()
                .map(|gate| gate.pitch)
                .collect::<Vec<u8>>()
        };

        // The player always gates their own hand, whichever way the app sounds.
        assert_eq!(gated(Hand::Left), vec![48]);
        assert_eq!(gated(Hand::Right), vec![72]);
        assert_eq!(gated(Hand::Both), vec![48, 72]);

        // Complement: the app sounds the other hand, as with play-along on a track.
        assert_eq!(played(Hand::Left, AppPlay::Complement), vec![72]);
        assert_eq!(played(Hand::Right, AppPlay::Complement), vec![48]);
        assert_eq!(played(Hand::Both, AppPlay::Complement), vec![48, 72]);

        // PlayerHand: the app sounds the player's own hand as a guide, as in Listen/Strict.
        assert_eq!(played(Hand::Left, AppPlay::PlayerHand), vec![48]);
        assert_eq!(played(Hand::Right, AppPlay::PlayerHand), vec![72]);

        // Silent: the app sounds nothing, as in Follow without play-along.
        assert!(played(Hand::Left, AppPlay::Silent).is_empty());
    }

    #[test]
    fn playing_a_track_gates_it_and_sounds_the_rest() {
        // Two single-note tracks: the player takes track 0, the app keeps track 1.
        let notes = vec![note(0, 0, 60, 0, 480), note(1, 1, 72, 480, 960)];
        let tracks = vec![
            TrackInfo {
                index: 0,
                name: Some("Piano".to_string()),
                program: Some(0),
                channel: Some(0),
                note_count: 1,
            },
            TrackInfo {
                index: 1,
                name: Some("Strings".to_string()),
                program: Some(40),
                channel: Some(1),
                note_count: 1,
            },
        ];
        let timeline = timeline_with(tracks, notes);

        let gated: Vec<u8> = gates(&timeline, Part::Track(0))
            .iter()
            .map(|gate| gate.pitch)
            .collect();
        let played: Vec<u8> = schedule(&timeline, Part::Track(0), AppPlay::Complement)
            .into_iter()
            .filter_map(|event| match event.command {
                AudioCommand::NoteOn { pitch, .. } => Some(pitch),
                _ => None,
            })
            .collect();

        // The player takes the piano line; the app keeps the strings.
        assert_eq!(gated, vec![60]);
        assert_eq!(played, vec![72]);
    }

    fn fixture_timeline() -> Timeline {
        let notes = vec![note(0, 0, 60, 0, 480), note(9, 9, 38, 0, 240)];
        let tracks = vec![
            TrackInfo {
                index: 0,
                name: None,
                program: Some(40),
                channel: Some(0),
                note_count: 1,
            },
            TrackInfo {
                index: 9,
                name: None,
                // A percussion track carries no program change; its channel decides the kit.
                program: None,
                channel: Some(9),
                note_count: 1,
            },
        ];
        timeline_with(tracks, notes)
    }

    fn timeline_with(tracks: Vec<TrackInfo>, notes: Vec<NoteEvent>) -> Timeline {
        Timeline::from_midi(&MidiFile {
            format: 1,
            division: Division::Metrical(480),
            tempo_map: vec![TempoChange {
                tick: 0,
                micros_per_quarter: 500_000,
            }],
            time_signatures: Vec::new(),
            key_signatures: Vec::new(),
            lyrics: Vec::new(),
            tracks,
            notes,
        })
    }

    fn note(track: u16, channel: u8, pitch: u8, start_tick: u64, end_tick: u64) -> NoteEvent {
        NoteEvent {
            track,
            channel,
            pitch,
            velocity: 100,
            start_tick,
            end_tick,
        }
    }
}
