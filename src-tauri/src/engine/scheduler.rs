use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::audio::{AudioCommand, Gate, PERCUSSION_CHANNEL, ScheduledCommand};

use super::timeline::Timeline;
use super::voices::{Staff, staff_of};

/// Which hand the player takes in one-hand practice; the app plays the other. `Both` is the
/// default: the app plays the whole piece as a guide and the player may play along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hand {
    Both,
    Left,
    Right,
}

/// Whether the player is expected to play a non-percussion note.
fn player_plays(pitch: u8, hand: Hand) -> bool {
    match hand {
        Hand::Both => true,
        Hand::Right => staff_of(pitch) == Staff::Treble,
        Hand::Left => staff_of(pitch) == Staff::Bass,
    }
}

/// Whether the app should sound a non-percussion note. In `Both` the app plays everything;
/// taking one hand silences that hand so only the player's part is missing.
fn app_plays(pitch: u8, hand: Hand) -> bool {
    match hand {
        Hand::Both => true,
        Hand::Right => staff_of(pitch) == Staff::Bass,
        Hand::Left => staff_of(pitch) == Staff::Treble,
    }
}

/// Flattens a timeline into one time-sorted command stream: a program change per track, then
/// the note-ons and note-offs of every note the app plays. The playback scheduler applies
/// these in order. Drums are always played by the app.
pub fn schedule(timeline: &Timeline, hand: Hand) -> Vec<ScheduledCommand> {
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
        if note.channel != PERCUSSION_CHANNEL && !app_plays(note.pitch, hand) {
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

/// The notes the player must play, in time order, for follow-you mode: the player's hand in
/// one-hand practice, otherwise everything but the drums. The app plays the percussion
/// itself, so drums never hold the playhead.
pub fn gates(timeline: &Timeline, hand: Hand) -> Arc<Vec<Gate>> {
    Arc::new(
        timeline
            .notes
            .iter()
            .filter(|note| note.channel != PERCUSSION_CHANNEL && player_plays(note.pitch, hand))
            .map(|note| Gate {
                pitch: note.pitch,
                start_ms: note.start_ms,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{Hand, gates, schedule};
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

        let gates = gates(&timeline, Hand::Both);

        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0].pitch, 60);
        assert!((gates[0].start_ms - 0.0).abs() < 1e-6);
        assert_eq!(gates[1].pitch, 64);
        assert!((gates[1].start_ms - 500.0).abs() < 1e-6);
    }

    #[test]
    fn sets_each_tracks_instrument_then_plays_its_notes() {
        let timeline = fixture_timeline();

        let events = schedule(&timeline, Hand::Both);

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

        let played = |hand| {
            schedule(&timeline, hand)
                .into_iter()
                .filter_map(|event| match event.command {
                    AudioCommand::NoteOn { pitch, .. } => Some(pitch),
                    _ => None,
                })
                .collect::<Vec<u8>>()
        };
        let gated = |hand| {
            gates(&timeline, hand)
                .iter()
                .map(|gate| gate.pitch)
                .collect::<Vec<u8>>()
        };

        // Taking the left hand: the player plays the bass note, the app sounds the treble note.
        assert_eq!(gated(Hand::Left), vec![48]);
        assert_eq!(played(Hand::Left), vec![72]);
        // Taking the right hand: the player plays the treble note, the app sounds the bass note.
        assert_eq!(gated(Hand::Right), vec![72]);
        assert_eq!(played(Hand::Right), vec![48]);
        // Both: the app plays everything and both notes are gates.
        assert_eq!(gated(Hand::Both), vec![48, 72]);
        assert_eq!(played(Hand::Both), vec![48, 72]);
    }

    fn fixture_timeline() -> Timeline {
        let notes = vec![note(0, 0, 60, 0, 480), note(9, 9, 38, 0, 240)];
        let tracks = vec![
            TrackInfo {
                index: 0,
                name: None,
                program: Some(40),
                note_count: 1,
            },
            TrackInfo {
                index: 9,
                name: None,
                // A percussion track carries no program change; its channel decides the kit.
                program: None,
                note_count: 1,
            },
        ];
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
