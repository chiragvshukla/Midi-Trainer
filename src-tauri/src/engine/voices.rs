//! Measure layout and voice splitting: which notes land in which bar, staff and voice.
//!
//! All of this works in ticks, before any milliseconds exist.

use super::midi::TimeSignature;
use super::quantizer::{self, Duration, QuantizedNote};

/// Which staff of the grand staff a note is written on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staff {
    Treble,
    Bass,
}

/// The split point between the hands: middle C and above is the right hand.
const MIDDLE_C: u8 = 60;

pub fn staff_of(pitch: u8) -> Staff {
    if pitch >= MIDDLE_C {
        Staff::Treble
    } else {
        Staff::Bass
    }
}

/// The length of a full measure, in ticks.
pub fn measure_ticks(signature: TimeSignature, ticks_per_quarter: u64) -> u64 {
    let beat_ticks = 4 * ticks_per_quarter / u64::from(signature.denominator.max(1));
    u64::from(signature.numerator.max(1)) * beat_ticks
}

/// The signature in force at a tick; 4/4 when the file declares nothing there.
pub fn signature_at(signatures: &[TimeSignature], tick: u64) -> TimeSignature {
    let mut declared: Vec<&TimeSignature> = signatures
        .iter()
        .filter(|signature| signature.tick <= tick)
        .collect();
    declared.sort_by_key(|signature| signature.tick);
    declared.last().copied().copied().unwrap_or(TimeSignature {
        tick: 0,
        numerator: 4,
        denominator: 4,
    })
}

/// One measure's span on the tick grid, with its governing time signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasureSpan {
    pub start_tick: u64,
    pub end_tick: u64,
    /// A short opening measure before the first downbeat.
    pub pickup: bool,
    pub signature: TimeSignature,
}

/// The first downbeat's offset when the piece begins with a pickup: the shift that makes
/// the music end exactly on a barline, provided some onset then lands on a downbeat
/// (otherwise the piece simply ends mid-measure and nothing should shift).
pub fn pickup_offset(notes: &[QuantizedNote], measure_length_ticks: u64) -> u64 {
    let last_end = notes
        .iter()
        .map(|note| note.end_tick)
        .max()
        .unwrap_or_default();
    if last_end == 0 || last_end % measure_length_ticks == 0 {
        return 0;
    }
    let offset = last_end % measure_length_ticks;
    let on_a_downbeat = notes.iter().any(|note| {
        note.start_tick >= offset && (note.start_tick - offset).is_multiple_of(measure_length_ticks)
    });
    if on_a_downbeat { offset } else { 0 }
}

/// Lays out measure boundaries from tick 0 (or from the pickup offset) to the end of the
/// music. A time signature change takes effect at the next boundary after its tick.
pub fn measure_spans(
    signatures: &[TimeSignature],
    ticks_per_quarter: u64,
    last_tick: u64,
    pickup_offset: u64,
) -> Vec<MeasureSpan> {
    let mut spans = Vec::new();
    if last_tick == 0 {
        return spans;
    }
    if pickup_offset > 0 {
        spans.push(MeasureSpan {
            start_tick: 0,
            end_tick: pickup_offset,
            pickup: true,
            signature: signature_at(signatures, 0),
        });
    }
    let mut tick = pickup_offset;
    loop {
        let signature = signature_at(signatures, tick);
        let length = measure_ticks(signature, ticks_per_quarter).max(1);
        let end = tick + length;
        let clamped = end.min(last_tick);
        spans.push(MeasureSpan {
            start_tick: tick,
            end_tick: clamped,
            pickup: false,
            signature,
        });
        if clamped >= last_tick {
            break;
        }
        tick = end;
    }
    spans
}

/// One drawable chunk of a note: a segment of it carrying a single notatable duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// Index into the parsed file's note list.
    pub source: usize,
    pub measure: usize,
    pub pitch: u8,
    pub start_tick: u64,
    pub end_tick: u64,
    pub duration: Duration,
    pub tied_from_previous: bool,
    pub tied_to_next: bool,
}

/// Splits every note at measure boundaries, then resolves each segment to notatable
/// durations. Wherever a note continues - across a barline, or across two values within
/// a measure - the pieces are marked as tied.
pub fn pieces_of(
    notes: &[QuantizedNote],
    spans: &[MeasureSpan],
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for note in notes {
        let segments = segments_of(note, spans);
        for (segment_index, (measure, start_tick, end_tick)) in segments.iter().enumerate() {
            let chain = quantizer::notate_duration(
                end_tick - start_tick,
                ticks_per_quarter,
                min_duration_ticks,
            );
            let last_in_chain = chain.len().saturating_sub(1);
            let mut position = *start_tick;
            for (chain_index, duration) in chain.iter().enumerate() {
                let piece_start = position;
                position += duration.ticks(ticks_per_quarter).round() as u64;
                let piece_end = if chain_index == last_in_chain {
                    *end_tick
                } else {
                    position.min(*end_tick)
                };
                pieces.push(Piece {
                    source: note.source,
                    measure: *measure,
                    pitch: note.pitch,
                    start_tick: piece_start,
                    end_tick: piece_end.max(piece_start + 1),
                    duration: *duration,
                    tied_from_previous: segment_index > 0 || chain_index > 0,
                    tied_to_next: segment_index + 1 < segments.len() || chain_index < last_in_chain,
                });
            }
        }
    }
    pieces
}

/// Cuts a note into the measures it sounds in, as (measure index, start, end) ticks.
fn segments_of(note: &QuantizedNote, spans: &[MeasureSpan]) -> Vec<(usize, u64, u64)> {
    let mut segments = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        let start = note.start_tick.max(span.start_tick);
        let end = note.end_tick.min(span.end_tick);
        if start < end {
            segments.push((index, start, end));
        }
    }
    segments
}

/// Notes that sound together with the same span: one drawable chord.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub start_tick: u64,
    pub end_tick: u64,
    pub duration: Duration,
    pub notes: Vec<Piece>,
}

/// Groups pieces that sound together, sorted by start. Pieces with the same span merge
/// into one cell, and a duplicated pitch (two hands in unison) collapses to one note.
pub fn cells_of(pieces: &[Piece]) -> Vec<Cell> {
    let mut sorted: Vec<Piece> = pieces.to_vec();
    sorted.sort_by_key(|piece| (piece.start_tick, piece.end_tick, piece.pitch));

    let mut cells: Vec<Cell> = Vec::new();
    for piece in sorted {
        match cells.last_mut() {
            Some(cell)
                if cell.start_tick == piece.start_tick && cell.end_tick == piece.end_tick =>
            {
                if cell
                    .notes
                    .iter()
                    .all(|existing| existing.pitch != piece.pitch)
                {
                    cell.notes.push(piece);
                }
            }
            _ => cells.push(Cell {
                start_tick: piece.start_tick,
                end_tick: piece.end_tick,
                duration: piece.duration,
                notes: vec![piece],
            }),
        }
    }
    cells
}

/// Splits a measure's cells into at most two voices: a cell that starts while voice one
/// still sounds goes to voice two, and a third overlapping line merges into voice two
/// (two voices per staff is the documented limit). Returns indices into `cells`.
pub fn two_voices(cells: &[Cell]) -> (Vec<usize>, Vec<usize>) {
    let mut voice_one = Vec::new();
    let mut voice_two = Vec::new();
    let mut free_one = 0u64;
    let mut free_two = 0u64;
    for (index, cell) in cells.iter().enumerate() {
        if cell.start_tick >= free_one {
            voice_one.push(index);
            free_one = cell.end_tick;
        } else if cell.start_tick >= free_two {
            voice_two.push(index);
            free_two = cell.end_tick;
        } else {
            voice_two.push(index);
            free_two = free_two.max(cell.end_tick);
        }
    }
    (voice_one, voice_two)
}

#[cfg(test)]
mod tests {
    use super::{
        MeasureSpan, QuantizedNote, Staff, TimeSignature, cells_of, measure_spans, measure_ticks,
        pickup_offset, pieces_of, signature_at, staff_of, two_voices,
    };
    use crate::engine::quantizer::{Base, Duration};

    const TPQ: u64 = 480;

    fn note(source: usize, pitch: u8, start_tick: u64, end_tick: u64) -> QuantizedNote {
        QuantizedNote {
            source,
            track: 0,
            channel: 0,
            pitch,
            velocity: 100,
            start_tick,
            end_tick,
        }
    }

    fn four_four() -> TimeSignature {
        TimeSignature {
            tick: 0,
            numerator: 4,
            denominator: 4,
        }
    }

    #[test]
    fn measure_lengths_follow_the_signature() {
        assert_eq!(measure_ticks(four_four(), TPQ), 1920);
        assert_eq!(
            measure_ticks(
                TimeSignature {
                    tick: 0,
                    numerator: 3,
                    denominator: 4
                },
                TPQ
            ),
            1440
        );
        assert_eq!(
            measure_ticks(
                TimeSignature {
                    tick: 0,
                    numerator: 6,
                    denominator: 8
                },
                TPQ
            ),
            1440
        );
    }

    #[test]
    fn a_missing_signature_defaults_to_four_four() {
        assert_eq!(signature_at(&[], 0), four_four());
    }

    #[test]
    fn a_signature_change_takes_effect_at_the_next_boundary() {
        let signatures = vec![
            four_four(),
            TimeSignature {
                tick: 1000,
                numerator: 3,
                denominator: 4,
            },
        ];
        let spans = measure_spans(&signatures, TPQ, 4800, 0);

        // 4/4 for the first bar, then 3/4 from the second boundary on.
        assert_eq!(spans[0].end_tick, 1920);
        assert_eq!(spans[1].start_tick, 1920);
        assert_eq!(spans[1].end_tick, 3360);
        assert_eq!(spans.len(), 3);
    }

    #[test]
    fn a_pickup_is_detected_when_the_end_then_lands_on_a_barline() {
        // A quarter pickup at tick 0, one full 4/4 bar, one full bar.
        let notes = vec![
            note(0, 60, 0, 480),
            note(1, 62, 480, 960),
            note(2, 64, 960, 1440),
            note(3, 65, 1440, 1920),
            note(4, 67, 1920, 2400),
            note(5, 69, 2400, 4320),
        ];

        assert_eq!(pickup_offset(&notes, 1920), 480);
    }

    #[test]
    fn a_piece_ending_mid_measure_has_no_pickup() {
        let notes = vec![
            note(0, 60, 0, 480),
            note(1, 62, 480, 960),
            note(2, 64, 960, 1440),
            note(3, 65, 1440, 2160),
        ];

        assert_eq!(pickup_offset(&notes, 1920), 0);
    }

    #[test]
    fn an_aligned_piece_has_no_pickup() {
        let notes = vec![note(0, 60, 0, 1920)];

        assert_eq!(pickup_offset(&notes, 1920), 0);
    }

    #[test]
    fn the_pickup_measure_comes_before_the_first_downbeat() {
        let notes = vec![note(0, 60, 0, 480), note(1, 62, 480, 4320)];
        let pickup = pickup_offset(&notes, 1920);
        let spans = measure_spans(&[four_four()], TPQ, 4320, pickup);

        assert_eq!(spans.len(), 3);
        assert!(spans[0].pickup);
        assert_eq!(spans[0].start_tick, 0);
        assert_eq!(spans[0].end_tick, 480);
        assert_eq!(spans[1].start_tick, 480);
        assert_eq!(spans[1].end_tick, 2400);
        assert_eq!(spans[2].end_tick, 4320);
    }

    #[test]
    fn notes_crossing_a_barline_are_cut_and_tied() {
        let notes = vec![note(0, 60, 0, 2400)];
        let spans = measure_spans(&[four_four()], TPQ, 2400, 0);
        let pieces = pieces_of(&notes, &spans, TPQ, 40);

        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].measure, 0);
        assert_eq!(
            pieces[0].duration,
            Duration::Straight {
                base: Base::Whole,
                dotted: false
            }
        );
        assert!(pieces[0].tied_to_next);
        assert!(!pieces[0].tied_from_previous);
        assert_eq!(pieces[1].measure, 1);
        assert_eq!(pieces[1].start_tick, 1920);
        assert_eq!(pieces[1].end_tick, 2400);
        assert!(pieces[1].tied_from_previous);
        assert!(!pieces[1].tied_to_next);
    }

    #[test]
    fn chord_members_merge_into_one_cell_and_unisons_collapse() {
        let spans = vec![MeasureSpan {
            start_tick: 0,
            end_tick: 1920,
            pickup: false,
            signature: four_four(),
        }];
        let notes = vec![
            note(0, 60, 0, 480),
            note(1, 64, 0, 480),
            note(2, 64, 0, 480), // same pitch again: unison duplicate
            note(3, 67, 0, 480),
        ];
        let pieces = pieces_of(&notes, &spans, TPQ, 40);
        let cells = cells_of(&pieces);

        assert_eq!(cells.len(), 1);
        assert_eq!(
            cells[0].notes.iter().map(|p| p.pitch).collect::<Vec<_>>(),
            vec![60, 64, 67]
        );
    }

    #[test]
    fn a_held_note_and_a_moving_line_split_into_two_voices() {
        let spans = vec![MeasureSpan {
            start_tick: 0,
            end_tick: 1920,
            pickup: false,
            signature: four_four(),
        }];
        let notes = vec![
            note(0, 60, 0, 1920),
            note(1, 62, 480, 960),
            note(2, 64, 960, 1440),
        ];
        let pieces = pieces_of(&notes, &spans, TPQ, 40);
        let cells = cells_of(&pieces);
        let (one, two) = two_voices(&cells);

        assert_eq!(one, vec![0]);
        assert_eq!(two, vec![1, 2]);
    }

    #[test]
    fn a_third_overlapping_line_merges_into_the_second_voice() {
        let spans = vec![MeasureSpan {
            start_tick: 0,
            end_tick: 1920,
            pickup: false,
            signature: four_four(),
        }];
        // Two lines starting together would be a chord; these overlap instead.
        let notes = vec![
            note(0, 60, 0, 1920),
            note(1, 62, 480, 960),
            note(2, 64, 480, 1440),
        ];
        let pieces = pieces_of(&notes, &spans, TPQ, 40);
        let cells = cells_of(&pieces);
        let (one, two) = two_voices(&cells);

        assert_eq!(one, vec![0]);
        assert_eq!(two, vec![1, 2]);
    }

    #[test]
    fn the_staff_follows_middle_c() {
        assert_eq!(staff_of(59), Staff::Bass);
        assert_eq!(staff_of(60), Staff::Treble);
        assert_eq!(staff_of(72), Staff::Treble);
    }
}
