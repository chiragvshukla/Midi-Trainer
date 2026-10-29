use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use super::EngineError;

/// Tempo assumed when a file has no tempo event: 500,000 microseconds per quarter note (120 bpm).
pub const DEFAULT_MICROS_PER_QUARTER: u32 = 500_000;

/// How tick positions map to time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Division {
    /// Musical time: ticks per quarter note.
    Metrical(u16),
    /// Real time: frames per second and ticks per frame.
    Timecode { fps: f32, ticks_per_frame: u8 },
}

/// A tempo change, in microseconds per quarter note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempoChange {
    pub tick: u64,
    pub micros_per_quarter: u32,
}

/// A time signature change. The denominator is a real value (4, 8, ...), not the file's power-of-two encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSignature {
    pub tick: u64,
    pub numerator: u8,
    pub denominator: u8,
}

/// A karaoke lyric syllable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricEvent {
    pub tick: u64,
    pub text: String,
}

/// One track of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackInfo {
    pub index: u16,
    pub name: Option<String>,
    pub program: Option<u8>,
    pub note_count: usize,
}

/// A note with its tick range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteEvent {
    pub track: u16,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub end_tick: u64,
}

/// A parsed Standard MIDI File or KAR karaoke file.
#[derive(Debug, Clone, PartialEq)]
pub struct MidiFile {
    /// 0 = single track, 1 = parallel tracks (the common case), 2 = independent patterns.
    pub format: u16,
    pub division: Division,
    /// Always starts at tick 0, with exactly one entry per tick.
    pub tempo_map: Vec<TempoChange>,
    pub time_signatures: Vec<TimeSignature>,
    pub lyrics: Vec<LyricEvent>,
    pub tracks: Vec<TrackInfo>,
    /// Sorted by start tick, then pitch, then track.
    pub notes: Vec<NoteEvent>,
}

/// A note that has started but has not ended yet.
struct OpenNote {
    channel: u8,
    pitch: u8,
    velocity: u8,
    start_tick: u64,
}

/// Parses `.mid` and `.kar` data.
pub fn parse(bytes: &[u8]) -> Result<MidiFile, EngineError> {
    let smf = Smf::parse(bytes)?;

    let mut tempo_map = Vec::new();
    let mut time_signatures = Vec::new();
    let mut lyrics = Vec::new();
    let mut tracks = Vec::new();
    let mut notes = Vec::new();

    for (index, track) in smf.tracks.iter().enumerate() {
        let track_index = index as u16;
        let mut tick = 0u64;
        let mut name = None;
        let mut program = None;
        let mut open: Vec<OpenNote> = Vec::new();
        let mut track_notes: Vec<NoteEvent> = Vec::new();

        for event in track {
            tick += u64::from(event.delta.as_int());

            match &event.kind {
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } => {
                            let pitch = key.as_int();
                            let velocity = vel.as_int();
                            if velocity == 0 {
                                // A note-on with velocity 0 is a note-off.
                                close_note(
                                    &mut open,
                                    &mut track_notes,
                                    track_index,
                                    channel,
                                    pitch,
                                    tick,
                                );
                            } else {
                                open.push(OpenNote {
                                    channel,
                                    pitch,
                                    velocity,
                                    start_tick: tick,
                                });
                            }
                        }
                        MidiMessage::NoteOff { key, .. } => {
                            close_note(
                                &mut open,
                                &mut track_notes,
                                track_index,
                                channel,
                                key.as_int(),
                                tick,
                            );
                        }
                        MidiMessage::ProgramChange { program: value } => {
                            program = Some(value.as_int())
                        }
                        _ => {}
                    }
                }
                TrackEventKind::Meta(message) => match message {
                    MetaMessage::Tempo(micros) => tempo_map.push(TempoChange {
                        tick,
                        micros_per_quarter: micros.as_int(),
                    }),
                    MetaMessage::TimeSignature(numerator, denominator, _, _) => time_signatures
                        .push(TimeSignature {
                            tick,
                            numerator: *numerator,
                            denominator: 2u8.saturating_pow(u32::from(*denominator)),
                        }),
                    MetaMessage::TrackName(bytes) => name = Some(text_of(bytes)),
                    MetaMessage::Lyric(bytes) => lyrics.push(LyricEvent {
                        tick,
                        text: text_of(bytes),
                    }),
                    _ => {}
                },
                _ => {}
            }
        }

        // A note that never ends still occupies its track up to the last tick seen.
        for note in open.drain(..) {
            track_notes.push(NoteEvent {
                track: track_index,
                channel: note.channel,
                pitch: note.pitch,
                velocity: note.velocity,
                start_tick: note.start_tick,
                end_tick: tick,
            });
        }

        tracks.push(TrackInfo {
            index: track_index,
            name,
            program,
            note_count: track_notes.len(),
        });
        notes.extend(track_notes);
    }

    notes.sort_by_key(|note| (note.start_tick, note.pitch, note.track));

    Ok(MidiFile {
        format: format_number(smf.header.format),
        division: division_of(smf.header.timing),
        tempo_map: tidy_tempo_map(tempo_map),
        time_signatures,
        lyrics,
        tracks,
        notes,
    })
}

/// Ends the earliest open note with this channel and pitch. Stray note-offs are ignored.
fn close_note(
    open: &mut Vec<OpenNote>,
    notes: &mut Vec<NoteEvent>,
    track: u16,
    channel: u8,
    pitch: u8,
    tick: u64,
) {
    if let Some(index) = open
        .iter()
        .position(|note| note.channel == channel && note.pitch == pitch)
    {
        let note = open.remove(index);
        notes.push(NoteEvent {
            track,
            channel: note.channel,
            pitch: note.pitch,
            velocity: note.velocity,
            start_tick: note.start_tick,
            end_tick: tick,
        });
    }
}

fn format_number(format: Format) -> u16 {
    match format {
        Format::SingleTrack => 0,
        Format::Parallel => 1,
        Format::Sequential => 2,
    }
}

fn division_of(timing: Timing) -> Division {
    match timing {
        Timing::Metrical(ticks) => Division::Metrical(ticks.as_int()),
        Timing::Timecode(fps, ticks_per_frame) => Division::Timecode {
            fps: fps.as_f32(),
            ticks_per_frame,
        },
    }
}

/// Byte strings in meta events are not guaranteed to be UTF-8.
fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Sorts by tick, keeps the last tempo per tick, and guarantees an entry at tick 0.
fn tidy_tempo_map(changes: Vec<TempoChange>) -> Vec<TempoChange> {
    let mut sorted = changes;
    sorted.sort_by_key(|change| change.tick);

    let mut tidied: Vec<TempoChange> = Vec::with_capacity(sorted.len() + 1);
    if sorted.first().is_none_or(|change| change.tick != 0) {
        tidied.push(TempoChange {
            tick: 0,
            micros_per_quarter: DEFAULT_MICROS_PER_QUARTER,
        });
    }
    for change in sorted {
        if let Some(last) = tidied.last_mut()
            && last.tick == change.tick
        {
            *last = change;
            continue;
        }
        tidied.push(change);
    }
    tidied
}

#[cfg(test)]
mod tests {
    use super::{Division, EngineError, LyricEvent, NoteEvent, TempoChange, TimeSignature, parse};

    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");

    #[test]
    fn parses_the_two_track_fixture() {
        let file = parse(&read_fixture("two-track-scale.mid")).unwrap();

        assert_eq!(file.format, 1);
        assert_eq!(file.division, Division::Metrical(480));
        assert_eq!(
            file.tempo_map,
            vec![
                TempoChange {
                    tick: 0,
                    micros_per_quarter: 500_000
                },
                TempoChange {
                    tick: 960,
                    micros_per_quarter: 250_000
                },
            ]
        );
        assert_eq!(
            file.time_signatures,
            vec![TimeSignature {
                tick: 0,
                numerator: 4,
                denominator: 4
            }]
        );

        assert_eq!(file.tracks.len(), 2);
        assert_eq!(file.tracks[0].name.as_deref(), Some("Tempo"));
        assert_eq!(file.tracks[0].note_count, 0);
        assert_eq!(file.tracks[1].name.as_deref(), Some("Piano"));
        assert_eq!(file.tracks[1].program, Some(0));
        assert_eq!(file.tracks[1].note_count, 4);

        let pitches: Vec<u8> = file.notes.iter().map(|note| note.pitch).collect();
        assert_eq!(pitches, vec![60, 62, 64, 65]);
        assert_eq!(file.notes[0].start_tick, 0);
        assert_eq!(file.notes[0].end_tick, 480);
        assert_eq!(file.notes[0].velocity, 100);
        assert_eq!(file.notes[0].track, 1);
        assert_eq!(file.notes[3].start_tick, 1440);
        assert_eq!(file.notes[3].end_tick, 1920);
    }

    #[test]
    fn parses_karaoke_lyrics() {
        let file = parse(&read_fixture("karaoke.kar")).unwrap();

        assert_eq!(file.notes.len(), 2);
        assert_eq!(file.tempo_map[0].micros_per_quarter, 600_000);
        assert_eq!(
            file.lyrics,
            vec![
                LyricEvent {
                    tick: 0,
                    text: "Hel-".to_string()
                },
                LyricEvent {
                    tick: 480,
                    text: "lo ".to_string()
                },
            ]
        );
    }

    #[test]
    fn keeps_the_channel_of_each_note() {
        let bytes = midi_file(
            0,
            480,
            vec![track(&[
                event(0, &[0x93, 60, 100]),
                event(480, &[0x83, 60, 64]),
            ])],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(file.notes[0].channel, 3);
    }

    #[test]
    fn note_on_with_velocity_zero_ends_the_note() {
        let bytes = midi_file(
            0,
            480,
            vec![track(&[
                event(0, &[0x90, 60, 100]),
                event(480, &[0x90, 60, 0]),
            ])],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(file.notes, vec![note(0, 60, 100, 0, 480)]);
    }

    #[test]
    fn overlapping_same_pitch_notes_pair_in_order() {
        let bytes = midi_file(
            0,
            480,
            vec![track(&[
                event(0, &[0x90, 60, 100]),
                event(10, &[0x90, 60, 100]),
                event(10, &[0x80, 60, 64]),
                event(10, &[0x80, 60, 64]),
            ])],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(
            file.notes,
            vec![note(0, 60, 100, 0, 20), note(0, 60, 100, 10, 30)]
        );
    }

    #[test]
    fn ignores_stray_note_off() {
        let bytes = midi_file(0, 480, vec![track(&[event(0, &[0x80, 60, 64])])]);

        let file = parse(&bytes).unwrap();
        assert!(file.notes.is_empty());
        assert_eq!(file.tracks[0].note_count, 0);
    }

    #[test]
    fn unterminated_note_ends_at_the_last_tick() {
        let bytes = midi_file(
            0,
            480,
            vec![track(&[
                event(0, &[0x90, 60, 100]),
                event(120, &[0x80, 62, 64]),
            ])],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(file.notes, vec![note(0, 60, 100, 0, 120)]);
    }

    #[test]
    fn notes_are_sorted_by_start_tick() {
        let bytes = midi_file(
            1,
            480,
            vec![
                track(&[event(480, &[0x90, 60, 100])]),
                track(&[event(0, &[0x90, 62, 100]), event(480, &[0x80, 62, 64])]),
            ],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(
            file.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
            vec![62, 60]
        );
        assert_eq!(file.notes[0].track, 1);
        assert_eq!(file.notes[1].track, 0);
    }

    #[test]
    fn missing_tempo_defaults_to_120_bpm() {
        let bytes = midi_file(0, 480, vec![track(&[event(0, &[0x90, 60, 100])])]);

        let file = parse(&bytes).unwrap();
        assert_eq!(
            file.tempo_map,
            vec![TempoChange {
                tick: 0,
                micros_per_quarter: 500_000
            }]
        );
    }

    #[test]
    fn the_last_tempo_at_a_tick_wins() {
        let bytes = midi_file(
            0,
            480,
            vec![track(&[
                event(0, &[0xff, 0x51, 0x03, 0x07, 0xa1, 0x20]),
                event(0, &[0xff, 0x51, 0x03, 0x03, 0xd0, 0x90]),
            ])],
        );

        let file = parse(&bytes).unwrap();
        assert_eq!(file.tempo_map.len(), 1);
        assert_eq!(file.tempo_map[0].micros_per_quarter, 250_000);
    }

    #[test]
    fn reads_smpte_timing() {
        // 0xE728 = 25 frames per second with 40 ticks per frame.
        let bytes = midi_file(0, 0xE728, vec![track(&[event(0, &[0x90, 60, 100])])]);

        let file = parse(&bytes).unwrap();
        assert_eq!(
            file.division,
            Division::Timecode {
                fps: 25.0,
                ticks_per_frame: 40
            }
        );
    }

    #[test]
    fn rejects_data_that_is_not_midi() {
        assert!(matches!(
            parse(b"this is not a midi file"),
            Err(EngineError::InvalidMidi(_))
        ));
    }

    fn read_fixture(name: &str) -> Vec<u8> {
        let path = std::path::Path::new(FIXTURE_DIR).join(name);
        std::fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
    }

    fn note(track: u16, pitch: u8, velocity: u8, start_tick: u64, end_tick: u64) -> NoteEvent {
        NoteEvent {
            track,
            channel: 0,
            pitch,
            velocity,
            start_tick,
            end_tick,
        }
    }

    fn midi_file(format: u16, division: u16, tracks: Vec<Vec<u8>>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MThd");
        bytes.extend_from_slice(&6u32.to_be_bytes());
        bytes.extend_from_slice(&format.to_be_bytes());
        bytes.extend_from_slice(&(tracks.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&division.to_be_bytes());
        for track in tracks {
            bytes.extend(chunk(b"MTrk", &track));
        }
        bytes
    }

    /// Concatenates raw event bytes and adds the end-of-track event.
    fn track(events: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        for event in events {
            body.extend_from_slice(event);
        }
        body.extend_from_slice(&[0x00, 0xff, 0x2f, 0x00]);
        body
    }

    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(id);
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        bytes
    }

    fn event(delta: u32, bytes: &[u8]) -> Vec<u8> {
        let mut out = variable_length(delta);
        out.extend_from_slice(bytes);
        out
    }

    fn variable_length(mut value: u32) -> Vec<u8> {
        let mut bytes = vec![(value & 0x7f) as u8];
        value >>= 7;
        while value > 0 {
            bytes.insert(0, ((value & 0x7f) | 0x80) as u8);
            value >>= 7;
        }
        bytes
    }
}
