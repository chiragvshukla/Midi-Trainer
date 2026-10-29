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

/// The beat a signature is felt in: three eighths (a dotted quarter) in compound meters
/// such as 6/8 and 9/8, otherwise the denominator value.
pub fn beat_ticks(signature: TimeSignature, ticks_per_quarter: u64) -> u64 {
    let unit = (4 * ticks_per_quarter / u64::from(signature.denominator.max(1))).max(1);
    if signature.denominator == 8 && signature.numerator.is_multiple_of(3) {
        unit * 3
    } else {
        unit
    }
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
/// the music end exactly on a barline, provided the shift is a whole number of beats no
/// longer than half a bar, music actually starts before it, and some onset then lands on a
/// downbeat. Anything else is a piece that merely starts or ends off the grid.
pub fn pickup_offset(
    notes: &[QuantizedNote],
    signature: TimeSignature,
    ticks_per_quarter: u64,
) -> u64 {
    let measure_length_ticks = measure_ticks(signature, ticks_per_quarter);
    let last_end = notes
        .iter()
        .map(|note| note.end_tick)
        .max()
        .unwrap_or_default();
    if last_end == 0 || last_end.is_multiple_of(measure_length_ticks) {
        return 0;
    }
    let offset = last_end % measure_length_ticks;
    if !offset.is_multiple_of(beat_ticks(signature, ticks_per_quarter))
        || offset * 2 > measure_length_ticks
    {
        return 0;
    }
    let starts_before_the_downbeat = notes.first().is_some_and(|note| note.start_tick < offset);
    if !starts_before_the_downbeat {
        return 0;
    }
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
/// still sounds goes to voice two; a third overlapping line shortens the tail of the
/// earlier cell it would overlap (two voices per staff is the documented limit). Keeping
/// the later note's true onset means playback verdicts stay aligned. Returns owned cells,
/// so callers no longer need the original list.
pub fn two_voices(
    cells: &[Cell],
    ticks_per_quarter: u64,
    min_duration_ticks: u64,
) -> (Vec<Cell>, Vec<Cell>) {
    let mut voice_one: Vec<Cell> = Vec::new();
    let mut voice_two: Vec<Cell> = Vec::new();
    for cell in cells {
        if voice_one
            .last()
            .is_none_or(|last| cell.start_tick >= last.end_tick)
        {
            voice_one.push(cell.clone());
        } else if voice_two
            .last()
            .is_none_or(|last| cell.start_tick >= last.end_tick)
        {
            voice_two.push(cell.clone());
        } else {
            if let Some(last) = voice_two.pop() {
                voice_two.extend(retime(
                    &last,
                    cell.start_tick,
                    ticks_per_quarter,
                    min_duration_ticks,
                ));
            }
            voice_two.push(cell.clone());
        }
    }
    (voice_one, voice_two)
}

/// Re-times a cell so it ends at `new_end`, re-notating the new length. Shorter ends trim
/// the tail (overlap resolution); longer ends extend it (absorbing a gap too short to be a
/// rest). A length that is not a single notatable value becomes a tied chain, exactly as
/// `pieces_of` does, and the cell's original end-tie is preserved on the last piece. A cell
/// re-timed to nothing disappears.
fn retime(cell: &Cell, new_end: u64, ticks_per_quarter: u64, min_duration_ticks: u64) -> Vec<Cell> {
    if new_end <= cell.start_tick {
        return Vec::new();
    }
    if new_end == cell.end_tick {
        return vec![cell.clone()];
    }
    let chain = quantizer::notate_duration(
        new_end - cell.start_tick,
        ticks_per_quarter,
        min_duration_ticks,
    );
    let last_in_chain = chain.len().saturating_sub(1);
    let mut cells = Vec::new();
    let mut position = cell.start_tick;
    let mut tied_from_previous = cell.notes.iter().any(|note| note.tied_from_previous);
    for (chain_index, duration) in chain.iter().enumerate() {
        let start_tick = position;
        position += duration.ticks(ticks_per_quarter).round() as u64;
        let last = chain_index == last_in_chain;
        let end_tick = if last { new_end } else { position.min(new_end) };
        cells.push(Cell {
            start_tick,
            end_tick,
            duration: *duration,
            notes: cell
                .notes
                .iter()
                .map(|note| Piece {
                    tied_from_previous,
                    tied_to_next: if last { note.tied_to_next } else { true },
                    ..*note
                })
                .collect(),
        });
        tied_from_previous = true;
    }
    cells
}

/// Absorbs gaps too short to be a rest into the preceding note, so a voice always tiles its
/// measure exactly: a 20-tick legato gap in a file whose shortest notatable value is 40
/// ticks cannot be drawn, and rounding it up would overrun the bar. `measure_end` closes the
/// voice, absorbing a short gap before the barline the same way.
pub fn fill_short_gaps(
    cells: Vec<Cell>,
    measure_end: u64,
    min_duration_ticks: u64,
    ticks_per_quarter: u64,
) -> Vec<Cell> {
    let mut result: Vec<Cell> = Vec::new();
    let count = cells.len();
    for (index, cell) in cells.into_iter().enumerate() {
        if let Some(last) = result.last() {
            let gap = cell.start_tick.saturating_sub(last.end_tick);
            if gap > 0 && gap < min_duration_ticks {
                let last = result.pop().expect("the last cell was just checked");
                result.extend(retime(
                    &last,
                    cell.start_tick,
                    ticks_per_quarter,
                    min_duration_ticks,
                ));
            }
        }
        let is_last = index + 1 == count;
        result.push(cell);
        if is_last {
            let last = result.pop().expect("the last cell was just pushed");
            let gap = measure_end.saturating_sub(last.end_tick);
            if gap > 0 && gap < min_duration_ticks {
                result.extend(retime(
                    &last,
                    measure_end,
                    ticks_per_quarter,
                    min_duration_ticks,
                ));
            } else {
                result.push(last);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        Cell, MeasureSpan, QuantizedNote, Staff, TimeSignature, cells_of, fill_short_gaps,
        measure_spans, measure_ticks, pickup_offset, pieces_of, signature_at, staff_of, two_voices,
    };
    use crate::engine::quantizer::{Base, Duration};

    const TPQ: u64 = 480;

    fn cell(start_tick: u64, end_tick: u64, duration: Duration) -> Cell {
        Cell {
            start_tick,
            end_tick,
            duration,
            notes: Vec::new(),
        }
    }

    fn quarter() -> Duration {
        Duration::Straight {
            base: Base::Quarter,
            dotted: false,
        }
    }

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

    fn spans_of(cells: &[super::Cell]) -> Vec<(u64, u64)> {
        cells
            .iter()
            .map(|cell| (cell.start_tick, cell.end_tick))
            .collect()
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

        assert_eq!(pickup_offset(&notes, four_four(), TPQ), 480);
    }

    #[test]
    fn a_piece_ending_mid_measure_has_no_pickup() {
        let notes = vec![
            note(0, 60, 0, 480),
            note(1, 62, 480, 960),
            note(2, 64, 960, 1440),
            note(3, 65, 1440, 2160),
        ];

        assert_eq!(pickup_offset(&notes, four_four(), TPQ), 0);
    }

    #[test]
    fn a_pickup_that_is_not_a_whole_number_of_beats_is_rejected() {
        // Two and three quarter beats: the end lands on a barline lookalike, but a real
        // pickup is a whole number of beats.
        let notes = vec![note(0, 60, 0, 1320), note(1, 62, 1320, 3240)];

        assert_eq!(pickup_offset(&notes, four_four(), TPQ), 0);
    }

    #[test]
    fn an_aligned_piece_has_no_pickup() {
        let notes = vec![note(0, 60, 0, 1920)];

        assert_eq!(pickup_offset(&notes, four_four(), TPQ), 0);
    }

    #[test]
    fn the_pickup_measure_comes_before_the_first_downbeat() {
        let notes = vec![note(0, 60, 0, 480), note(1, 62, 480, 4320)];
        let pickup = pickup_offset(&notes, four_four(), TPQ);
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
        let (one, two) = two_voices(&cells, TPQ, 40);

        assert_eq!(spans_of(&one), vec![(0, 1920)]);
        assert_eq!(spans_of(&two), vec![(480, 960), (960, 1440)]);
    }

    #[test]
    fn a_third_overlapping_line_shortens_the_earlier_tail() {
        let spans = vec![MeasureSpan {
            start_tick: 0,
            end_tick: 1920,
            pickup: false,
            signature: four_four(),
        }];
        // A whole-note line over two moving lines; the third line cuts into the second.
        let notes = vec![
            note(0, 60, 0, 1920),
            note(1, 62, 480, 960),
            note(2, 64, 720, 1440),
        ];
        let pieces = pieces_of(&notes, &spans, TPQ, 40);
        let cells = cells_of(&pieces);
        let (one, two) = two_voices(&cells, TPQ, 40);

        assert_eq!(spans_of(&one), vec![(0, 1920)]);
        // The 62 note keeps its onset but its tail is cut to where the 64 note starts.
        assert_eq!(spans_of(&two), vec![(480, 720), (720, 1440)]);
        assert_eq!(
            two[0].duration,
            Duration::Straight {
                base: Base::Eighth,
                dotted: false
            }
        );
    }

    #[test]
    fn a_gap_too_short_to_notate_is_absorbed_into_the_previous_note() {
        // Two quarters 20 ticks apart: a 20-tick rest is finer than any symbol, so the
        // first note is extended to meet the second and the voice tiles its measure.
        let cells = vec![cell(0, 460, quarter()), cell(480, 960, quarter())];

        let filled = fill_short_gaps(cells, 1920, 40, TPQ);

        assert_eq!(spans_of(&filled), vec![(0, 480), (480, 960)]);
        assert_eq!(filled[0].duration.ticks(TPQ), 480.0);
    }

    #[test]
    fn a_short_gap_before_the_barline_is_absorbed() {
        let cells = vec![cell(
            0,
            1900,
            Duration::Straight {
                base: Base::Half,
                dotted: false,
            },
        )];

        let filled = fill_short_gaps(cells, 1920, 40, TPQ);

        assert_eq!(spans_of(&filled), vec![(0, 1920)]);
    }

    #[test]
    fn the_staff_follows_middle_c() {
        assert_eq!(staff_of(59), Staff::Bass);
        assert_eq!(staff_of(60), Staff::Treble);
        assert_eq!(staff_of(72), Staff::Treble);
    }
}
