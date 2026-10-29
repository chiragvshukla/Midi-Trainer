//! The engraved notation model: measures, staves, voices.
//!
//! This is the one source of truth the sheet view renders and the cursor reads. It is
//! built from the same parsed file as the playback timeline, so the two never disagree.

use serde::Serialize;

use super::midi::MidiFile;
use super::quantizer::{self, Duration, QuantizedNote};
use super::timeline::Timeline;
use super::voices::{self, Cell, MeasureSpan, Piece, Staff};
use crate::audio::PERCUSSION_CHANNEL;

/// The natural pitch class of each written letter, C..B.
const LETTER_PITCH: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

/// The letters each sharp raises, in key-signature order (F C G D A E B).
const SHARP_LETTERS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];

/// The letters each flat lowers, in key-signature order (B E A D G C F).
const FLAT_LETTERS: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];

/// The engraved piece: measures in order, each holding both staves.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Score {
    pub measures: Vec<Measure>,
    /// The piece's key, when the file declares one. Display only: the renderer draws the
    /// sharps or flats but does not suppress in-key accidentals.
    pub key_signature: Option<KeySignature>,
}

/// A key signature: the number of sharps (negative for flats) and whether the key is minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KeySignature {
    pub fifths: i8,
    pub minor: bool,
}

/// One measure of music: its timing on the playback timeline, its signature, and the
/// voices of each staff. Empty staves carry a whole-measure rest.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measure {
    /// Playback time of the measure's first beat, in milliseconds.
    pub start_ms: f64,
    /// 1-based; the pickup measure, if any, is 0.
    pub number: usize,
    pub time_signature: TimeSignature,
    pub treble: Vec<Voice>,
    pub bass: Vec<Voice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TimeSignature {
    pub numerator: u8,
    pub denominator: u8,
}

/// One staff voice: chords and rests in order, their durations filling the measure.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Voice {
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum Event {
    Chord(Chord),
    Rest { duration: Duration },
}

/// Notes sounding together, written as one chord with one duration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chord {
    pub duration: Duration,
    pub notes: Vec<ChordNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChordNote {
    pub pitch: u8,
    /// The written letter, 0 = C .. 6 = B, in the piece's key.
    pub letter: u8,
    /// That letter's chromatic alteration: -1 flat, 0 natural, +1 sharp.
    pub alter: i8,
    /// Whether the engraver must print an accidental, given the key. The sign follows `alter`
    /// (a natural in an altered key needs a natural sign). False when the key already covers it.
    pub accidental: bool,
    /// The original note's onset on the timeline, for matching playback verdicts.
    pub onset_ms: f64,
    pub tied_from_previous: bool,
    pub tied_to_next: bool,
}

/// Engraves a parsed file into the notation model. The drums are left out: the app plays
/// them itself, so they are not part of the written music.
pub fn engrave(file: &MidiFile, timeline: &Timeline) -> Score {
    let quantization = quantizer::quantize(file);
    let ticks_per_quarter = quantizer::ticks_per_quarter(file);
    let notes: Vec<QuantizedNote> = quantization
        .notes
        .iter()
        .filter(|note| note.channel != PERCUSSION_CHANNEL)
        .cloned()
        .collect();
    if notes.is_empty() {
        return Score {
            measures: Vec::new(),
            key_signature: None,
        };
    }

    let last_tick = notes.iter().map(|note| note.end_tick).max().unwrap_or(0);
    let first = voices::signature_at(&file.time_signatures, 0);
    let pickup = voices::pickup_offset(&notes, first, ticks_per_quarter);
    let spans = voices::measure_spans(&file.time_signatures, ticks_per_quarter, last_tick, pickup);
    let pieces = voices::pieces_of(
        &notes,
        &spans,
        ticks_per_quarter,
        quantization.min_duration_ticks,
    );

    let opens_with_pickup = spans.first().is_some_and(|span| span.pickup);
    let measures = spans
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let number = if span.pickup {
                0
            } else if opens_with_pickup {
                index
            } else {
                index + 1
            };
            let in_measure: Vec<Piece> = pieces
                .iter()
                .copied()
                .filter(|piece| piece.measure == index)
                .collect();
            build_measure(
                number,
                span,
                &in_measure,
                file,
                timeline,
                ticks_per_quarter,
                quantization.min_duration_ticks,
            )
        })
        .collect();
    let key_signature = file.key_signatures.first().map(|key| KeySignature {
        fifths: key.fifths,
        minor: key.minor,
    });
    Score {
        measures,
        key_signature,
    }
}

fn build_measure(
    number: usize,
    span: &MeasureSpan,
    pieces: &[Piece],
    file: &MidiFile,
    timeline: &Timeline,
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) -> Measure {
    let staff_voices = |staff| -> Vec<Voice> {
        let staff_pieces: Vec<Piece> = pieces
            .iter()
            .copied()
            .filter(|piece| voices::staff_of(piece.pitch) == staff)
            .collect();
        let cells = voices::cells_of(&staff_pieces);
        voices_of(
            &cells,
            span,
            file,
            timeline,
            ticks_per_quarter,
            min_duration_ticks,
        )
    };

    Measure {
        start_ms: timeline.tick_to_ms(span.start_tick),
        number,
        time_signature: TimeSignature {
            numerator: span.signature.numerator,
            denominator: span.signature.denominator,
        },
        treble: staff_voices(Staff::Treble),
        bass: staff_voices(Staff::Bass),
    }
}

/// Builds the voices of one staff: voice one always exists (rest-filled when empty),
/// voice two only when it carries notes.
fn voices_of(
    cells: &[Cell],
    span: &MeasureSpan,
    file: &MidiFile,
    timeline: &Timeline,
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) -> Vec<Voice> {
    let (one, two) = voices::two_voices(cells, ticks_per_quarter, min_duration_ticks);
    let one = voices::fill_short_gaps(one, span.end_tick, min_duration_ticks, ticks_per_quarter);
    let two = voices::fill_short_gaps(two, span.end_tick, min_duration_ticks, ticks_per_quarter);
    let mut staff = vec![filled_voice(
        &one,
        span,
        file,
        timeline,
        ticks_per_quarter,
        min_duration_ticks,
    )];
    if !two.is_empty() {
        staff.push(filled_voice(
            &two,
            span,
            file,
            timeline,
            ticks_per_quarter,
            min_duration_ticks,
        ));
    }
    staff
}

/// Lays one voice out across the measure, filling every gap with rests.
fn filled_voice(
    voice: &[Cell],
    span: &MeasureSpan,
    file: &MidiFile,
    timeline: &Timeline,
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) -> Voice {
    let mut events = Vec::new();
    let mut tick = span.start_tick;
    // The piece's key decides how notes are spelled and which accidentals the key already shows.
    let fifths = file.key_signatures.first().map_or(0, |key| key.fifths);
    for cell in voice {
        if cell.start_tick > tick {
            push_rests(
                &mut events,
                cell.start_tick - tick,
                ticks_per_quarter,
                min_duration_ticks,
            );
        }
        events.push(Event::Chord(Chord {
            duration: cell.duration,
            notes: cell
                .notes
                .iter()
                .map(|piece| {
                    let (letter, alter) = spell(piece.pitch, fifths);
                    ChordNote {
                        pitch: piece.pitch,
                        letter: letter as u8,
                        alter,
                        accidental: accidental_needed(letter, alter, fifths),
                        onset_ms: timeline.tick_to_ms(file.notes[piece.source].start_tick),
                        tied_from_previous: piece.tied_from_previous,
                        tied_to_next: piece.tied_to_next,
                    }
                })
                .collect(),
        }));
        // Voices are non-overlapping by construction; never walk time backwards.
        tick = tick.max(cell.end_tick);
    }
    if tick < span.end_tick {
        push_rests(
            &mut events,
            span.end_tick - tick,
            ticks_per_quarter,
            min_duration_ticks,
        );
    }
    Voice { events }
}

/// The written letter and alteration MIDI cannot carry: a black key is a sharpened lower letter
/// in sharp keys, a flattened upper letter in flat keys. Natural notes keep their own letter.
fn spell(pitch: u8, fifths: i8) -> (usize, i8) {
    let pitch_class = pitch % 12;
    if let Some(letter) = (0..7).find(|&letter| LETTER_PITCH[letter] == pitch_class) {
        return (letter, 0);
    }
    let natural = if fifths >= 0 {
        pitch_class.wrapping_sub(1)
    } else {
        pitch_class + 1
    };
    let letter = (0..7)
        .find(|&letter| LETTER_PITCH[letter] == natural)
        .expect("every black key sits between two naturals");
    (letter, if fifths >= 0 { 1 } else { -1 })
}

/// Which letters the key signature already alters.
fn key_altered_letters(fifths: i8) -> [bool; 7] {
    let mut altered = [false; 7];
    let count = fifths.unsigned_abs().min(7) as usize;
    let order = if fifths >= 0 {
        SHARP_LETTERS
    } else {
        FLAT_LETTERS
    };
    for &letter in &order[..count] {
        altered[letter] = true;
    }
    altered
}

/// Whether an accidental must be printed: a natural on an altered letter, or an altered note the
/// key does not already cover.
fn accidental_needed(letter: usize, alter: i8, fifths: i8) -> bool {
    let altered = key_altered_letters(fifths)[letter];
    if alter == 0 { altered } else { !altered }
}

fn push_rests(
    events: &mut Vec<Event>,
    length_ticks: u64,
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) {
    for duration in quantizer::notate_duration(length_ticks, ticks_per_quarter, min_duration_ticks)
    {
        events.push(Event::Rest { duration });
    }
}

#[cfg(test)]
mod tests {
    use super::quantizer::Base;
    use super::{Chord, Duration, Event, MidiFile, Score, TimeSignature, Timeline, engrave};
    use crate::engine::midi::{Division, NoteEvent};

    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");

    fn file_of(notes: &[(u8, u64, u64)]) -> MidiFile {
        file_with(notes, &[])
    }

    /// Notes are (pitch, start tick, end tick) on channel 0; extras are (channel, pitch, start, end).
    fn file_with(notes: &[(u8, u64, u64)], extras: &[(u8, u8, u64, u64)]) -> MidiFile {
        let mut events: Vec<NoteEvent> = notes
            .iter()
            .map(|(pitch, start, end)| NoteEvent {
                track: 0,
                channel: 0,
                pitch: *pitch,
                velocity: 100,
                start_tick: *start,
                end_tick: *end,
            })
            .collect();
        events.extend(extras.iter().map(|(channel, pitch, start, end)| NoteEvent {
            track: 1,
            channel: *channel,
            pitch: *pitch,
            velocity: 100,
            start_tick: *start,
            end_tick: *end,
        }));
        MidiFile {
            format: 0,
            division: Division::Metrical(480),
            tempo_map: Vec::new(),
            time_signatures: Vec::new(),
            key_signatures: Vec::new(),
            lyrics: Vec::new(),
            tracks: Vec::new(),
            notes: events,
        }
    }

    fn engrave_of(notes: &[(u8, u64, u64)]) -> Score {
        let file = file_of(notes);
        let timeline = Timeline::from_midi(&file);
        engrave(&file, &timeline)
    }

    fn chord(events: &[Event]) -> Vec<&Chord> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Chord(chord) => Some(chord),
                Event::Rest { .. } => None,
            })
            .collect()
    }

    fn quarters() -> Vec<Duration> {
        vec![
            Duration::Straight {
                base: Base::Quarter,
                dotted: false
            };
            4
        ]
    }

    #[test]
    fn carries_the_files_key_signature_into_the_score() {
        let mut file = file_of(&[(60, 0, 480)]);
        file.key_signatures = vec![crate::engine::midi::KeySignature {
            tick: 0,
            fifths: 2,
            minor: false,
        }];
        let timeline = Timeline::from_midi(&file);
        let score = engrave(&file, &timeline);

        assert_eq!(
            score.key_signature,
            Some(super::KeySignature {
                fifths: 2,
                minor: false
            })
        );
    }

    #[test]
    fn a_piece_without_a_key_signature_has_none() {
        assert_eq!(engrave_of(&[(60, 0, 480)]).key_signature, None);
    }

    #[test]
    fn engraves_the_scale_fixture_into_one_measure_of_quarters() {
        let path = std::path::Path::new(FIXTURE_DIR).join("two-track-scale.mid");
        let file = crate::engine::midi::parse(&std::fs::read(&path).unwrap()).unwrap();
        let timeline = Timeline::from_midi(&file);
        let score = engrave(&file, &timeline);

        assert_eq!(score.measures.len(), 1);
        let measure = &score.measures[0];
        assert_eq!(
            measure.time_signature,
            TimeSignature {
                numerator: 4,
                denominator: 4
            }
        );
        assert_eq!(measure.start_ms, 0.0);
        assert_eq!(measure.number, 1);

        // All four notes sit above middle C: one voice of four quarter chords.
        assert_eq!(measure.treble.len(), 1);
        let events = &measure.treble[0].events;
        assert_eq!(events.len(), 4);
        let pitches: Vec<Vec<u8>> = chord(events)
            .iter()
            .map(|chord| chord.notes.iter().map(|note| note.pitch).collect())
            .collect();
        assert_eq!(pitches, vec![vec![60], vec![62], vec![64], vec![65]]);
        assert_eq!(
            chord(events)
                .iter()
                .map(|chord| chord.duration)
                .collect::<Vec<_>>(),
            quarters()
        );
        // Onsets follow the file's tempo map: 0, 500, 1000, then 1250 after the change.
        let onsets: Vec<f64> = chord(events)
            .iter()
            .map(|chord| chord.notes[0].onset_ms)
            .collect();
        let expected_onsets = [0.0, 500.0, 1000.0, 1250.0];
        for (actual, expected) in onsets.iter().zip(expected_onsets) {
            assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
        }

        // The bass staff carries nothing but a whole-measure rest, in one voice.
        assert_eq!(measure.bass.len(), 1);
        assert_eq!(
            measure.bass[0].events,
            vec![Event::Rest {
                duration: Duration::Straight {
                    base: Base::Whole,
                    dotted: false
                }
            }]
        );
    }

    #[test]
    fn a_pickup_measure_is_numbered_zero_and_shifts_the_bars() {
        let score = engrave_of(&[
            (60, 0, 480),
            (62, 480, 960),
            (64, 960, 1440),
            (65, 1440, 1920),
            (67, 1920, 2400),
            (69, 2400, 4320),
        ]);

        assert_eq!(score.measures.len(), 3);
        assert_eq!(score.measures[0].number, 0);
        assert_eq!(score.measures[1].number, 1);
        assert_eq!(score.measures[2].number, 2);
        // The pickup holds just the one quarter; the next bars are full.
        assert_eq!(
            chord(&score.measures[0].treble[0].events)
                .iter()
                .map(|chord| chord.notes[0].pitch)
                .collect::<Vec<_>>(),
            vec![60]
        );
        assert_eq!(score.measures[0].treble[0].events.len(), 1);
        assert_eq!(
            score.measures[1].treble[0]
                .events
                .iter()
                .filter(|event| matches!(event, Event::Chord(_)))
                .count(),
            4
        );
    }

    #[test]
    fn a_note_crossing_the_barline_is_tied_across_measures() {
        let score = engrave_of(&[(60, 0, 2400)]);

        assert_eq!(score.measures.len(), 2);
        let first = &score.measures[0].treble[0].events;
        let second = &score.measures[1].treble[0].events;
        let Event::Chord(chord) = &first[0] else {
            panic!("expected a chord");
        };
        assert!(chord.notes[0].tied_to_next);
        assert!(!chord.notes[0].tied_from_previous);
        let Event::Chord(chord) = &second[0] else {
            panic!("expected a chord");
        };
        assert!(chord.notes[0].tied_from_previous);
        assert!(!chord.notes[0].tied_to_next);
    }

    #[test]
    fn simultaneous_notes_form_one_chord_and_held_plus_moving_splits_voices() {
        // A held note and a moving line on separate instrument tracks: two voices.
        let file = file_with(&[(60, 0, 1920)], &[(0, 64, 480, 960), (0, 67, 960, 1440)]);
        let timeline = Timeline::from_midi(&file);
        let score = engrave(&file, &timeline);

        let measure = &score.measures[0];
        assert_eq!(measure.treble.len(), 2);
        // Voice one: the held whole note. Voice two: rest, two quarters, rest.
        assert_eq!(measure.treble[0].events.len(), 1);
        assert_eq!(measure.treble[1].events.len(), 4);
        assert!(matches!(measure.treble[1].events[0], Event::Rest { .. }));
        assert!(matches!(measure.treble[1].events[1], Event::Chord(_)));
        assert!(matches!(measure.treble[1].events[3], Event::Rest { .. }));
    }

    #[test]
    fn notes_below_middle_c_land_on_the_bass_staff() {
        let score = engrave_of(&[(36, 0, 480), (72, 0, 480)]);

        let measure = &score.measures[0];
        let treble_pitches: Vec<u8> = chord(&measure.treble[0].events)
            .iter()
            .flat_map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|note| note.pitch)
                    .collect::<Vec<_>>()
            })
            .collect();
        let bass_pitches: Vec<u8> = chord(&measure.bass[0].events)
            .iter()
            .flat_map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|note| note.pitch)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(treble_pitches, vec![72]);
        assert_eq!(bass_pitches, vec![36]);
    }

    #[test]
    fn the_drums_are_not_engraved() {
        let file = file_with(&[(60, 0, 480)], &[(9, 38, 0, 480)]);
        let timeline = Timeline::from_midi(&file);
        let score = engrave(&file, &timeline);

        let pitches: Vec<u8> = chord(&score.measures[0].treble[0].events)
            .iter()
            .flat_map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|note| note.pitch)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(pitches, vec![60]);
    }

    #[test]
    fn triplet_eighths_notate_as_triplets() {
        // Three triplet eighths, then a held note out to the end of the 4/4 measure.
        let score = engrave_of(&[
            (60, 0, 160),
            (62, 160, 320),
            (64, 320, 480),
            (67, 480, 1920),
        ]);

        let events = &score.measures[0].treble[0].events;
        let durations: Vec<Duration> = events
            .iter()
            .map(|event| match event {
                Event::Chord(chord) => chord.duration,
                Event::Rest { duration } => *duration,
            })
            .collect();
        assert_eq!(durations.len(), 4);
        for duration in &durations[..3] {
            assert_eq!(*duration, Duration::Triplet { base: Base::Eighth });
        }
        assert_eq!(
            durations[3],
            Duration::Straight {
                base: Base::Half,
                dotted: true
            }
        );
    }

    #[test]
    fn a_sharp_key_spells_black_keys_sharp_and_silences_in_key_accidentals() {
        // D major (two sharps: F#, C#).
        let score = engrave_with_key(
            &[
                (66, 0, 480),
                (65, 480, 960),
                (61, 960, 1440),
                (60, 1440, 1920),
            ],
            2,
        );
        let notes: Vec<(u8, u8, i8, bool)> = chord(&score.measures[0].treble[0].events)
            .iter()
            .flat_map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|note| (note.pitch, note.letter, note.alter, note.accidental))
            })
            .collect();

        assert_eq!(
            notes,
            vec![
                (66, 3, 1, false), // F#, covered by the key
                (65, 3, 0, true),  // F natural, needs a natural sign
                (61, 0, 1, false), // C#, covered by the key
                (60, 0, 0, true),  // C natural, cancelling the key's C#
            ]
        );
    }

    #[test]
    fn a_flat_key_spells_black_keys_flat() {
        // F major (one flat: Bb).
        let score = engrave_with_key(
            &[
                (70, 0, 480),
                (71, 480, 960),
                (61, 960, 1440),
                (62, 1440, 1920),
            ],
            -1,
        );
        let notes: Vec<(u8, u8, i8, bool)> = chord(&score.measures[0].treble[0].events)
            .iter()
            .flat_map(|chord| {
                chord
                    .notes
                    .iter()
                    .map(|note| (note.pitch, note.letter, note.alter, note.accidental))
            })
            .collect();

        assert_eq!(
            notes,
            vec![
                (70, 6, -1, false), // Bb, covered by the key
                (71, 6, 0, true),   // B natural, needs a natural sign
                (61, 1, -1, true),  // Db, not in the key
                (62, 1, 0, false),  // D natural, unaltered
            ]
        );
    }

    #[test]
    fn without_a_key_every_black_key_keeps_its_sharp() {
        let score = engrave_with_key(&[(61, 0, 480)], 0);
        let note = chord(&score.measures[0].treble[0].events)[0].notes[0];

        assert_eq!((note.letter, note.alter, note.accidental), (0, 1, true));
    }

    #[test]
    fn an_empty_file_engraves_to_nothing() {
        assert_eq!(engrave_of(&[]).measures, Vec::new());
    }

    fn engrave_with_key(notes: &[(u8, u64, u64)], fifths: i8) -> Score {
        let mut file = file_of(notes);
        file.key_signatures = vec![crate::engine::midi::KeySignature {
            tick: 0,
            fifths,
            minor: false,
        }];
        let timeline = Timeline::from_midi(&file);
        engrave(&file, &timeline)
    }
}
