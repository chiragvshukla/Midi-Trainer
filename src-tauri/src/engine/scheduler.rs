use std::collections::HashMap;
use std::sync::Arc;

use crate::audio::{AudioCommand, Gate, PERCUSSION_CHANNEL, ScheduledCommand};

use super::timeline::Timeline;

/// Flattens a timeline into one time-sorted command stream: a program change per track, then
/// the note-ons and note-offs of every note. The playback scheduler applies these in order.
pub fn schedule(timeline: &Timeline) -> Vec<ScheduledCommand> {
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

/// The notes the player must play, in time order, for follow-you mode. The app plays the
/// percussion itself, so drums never hold the playhead.
pub fn gates(timeline: &Timeline) -> Arc<Vec<Gate>> {
    Arc::new(
        timeline
            .notes
            .iter()
            .filter(|note| note.channel != PERCUSSION_CHANNEL)
            .map(|note| Gate {
                pitch: note.pitch,
                start_ms: note.start_ms,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{gates, schedule};
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
            lyrics: Vec::new(),
            tracks,
            notes,
        });

        let gates = gates(&timeline);

        assert_eq!(gates.len(), 2);
        assert_eq!(gates[0].pitch, 60);
        assert!((gates[0].start_ms - 0.0).abs() < 1e-6);
        assert_eq!(gates[1].pitch, 64);
        assert!((gates[1].start_ms - 500.0).abs() < 1e-6);
    }

    #[test]
    fn sets_each_tracks_instrument_then_plays_its_notes() {
        let timeline = fixture_timeline();

        let events = schedule(&timeline);

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
