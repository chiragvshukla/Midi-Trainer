//! Rhythmic cleanup: raw MIDI ticks snapped to a grid and resolved to notatable values.
//!
//! The quantizer works purely in ticks, so it is independent of tempo. Every file gets the
//! same fine snap grid, but the smallest value the file actually uses decides how fine the
//! *notation* can get, so a piece of quarter notes never produces sixteenths.

use std::collections::BTreeMap;

use serde::Serialize;

use super::midi::{Division, MidiFile};
use crate::audio::PERCUSSION_CHANNEL;

/// Onsets are snapped to this many subdivisions of a quarter note. 24 is divisible by every
/// straight value down to a 32nd (8 per quarter) and every triplet value down to a triplet
/// 16th (12 per quarter), so binary and ternary rhythms both land exactly on the grid.
const SNAP_SUBDIVISIONS_PER_QUARTER: u64 = 24;

/// Ticks per quarter assumed for files without a metrical division (SMPTE-timed files).
const DEFAULT_TICKS_PER_QUARTER: u64 = 480;

/// Standard subdivisions of a quarter note a derived resolution can land on, coarsest first.
/// 3 and 6 and 12 are the triplet values.
const DERIVED_SUBDIVISIONS: [u64; 7] = [1, 2, 3, 4, 6, 8, 12];

/// A note after rhythmic cleanup, still linked to the note it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantizedNote {
    /// Index into the parsed file's note list, for mapping back to sound and verdicts.
    pub source: usize,
    pub track: u16,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
    pub start_tick: u64,
    pub end_tick: u64,
}

/// The rhythmic cleanup of a whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quantization {
    /// Sorted by start tick, then pitch.
    pub notes: Vec<QuantizedNote>,
    /// The file's own smallest note value, in ticks. Finer values are performance noise
    /// and are notated at this length instead.
    pub min_duration_ticks: u64,
}

/// Snaps a parsed file's notes onto the rhythmic grid.
pub fn quantize(file: &MidiFile) -> Quantization {
    let ticks_per_quarter = ticks_per_quarter(file);
    let grid = (ticks_per_quarter / SNAP_SUBDIVISIONS_PER_QUARTER).max(1);
    let min_duration_ticks = derived_min_duration(file, ticks_per_quarter);

    let mut notes: Vec<QuantizedNote> = file
        .notes
        .iter()
        .enumerate()
        .map(|(source, note)| {
            let start_tick = snap_tick(note.start_tick, grid);
            let mut end_tick = snap_tick(note.end_tick, grid);
            if end_tick <= start_tick {
                end_tick = start_tick + min_duration_ticks;
            }
            QuantizedNote {
                source,
                track: note.track,
                channel: note.channel,
                pitch: note.pitch,
                velocity: note.velocity,
                start_tick,
                end_tick,
            }
        })
        .collect();

    trim_overlaps(&mut notes);
    notes.sort_by_key(|note| (note.start_tick, note.pitch, note.track));

    Quantization {
        notes,
        min_duration_ticks,
    }
}

/// A notatable rhythmic value, as the sheet view will draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum Duration {
    /// A straight value, possibly dotted (one dot).
    Straight { base: Base, dotted: bool },
    /// One note of a triplet: three of these fill the time of two straight values of `base`.
    Triplet { base: Base },
}

/// The notatable note heads, longest to shortest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Base {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl Duration {
    /// Length in ticks.
    pub fn ticks(self, ticks_per_quarter: u64) -> f64 {
        let (base, multiplier) = match self {
            Duration::Straight {
                base,
                dotted: false,
            } => (base, 1.0),
            Duration::Straight { base, dotted: true } => (base, 1.5),
            Duration::Triplet { base } => (base, 2.0 / 3.0),
        };
        base.quarters() * multiplier * ticks_per_quarter as f64
    }
}

impl Base {
    fn quarters(self) -> f64 {
        match self {
            Base::Whole => 4.0,
            Base::Half => 2.0,
            Base::Quarter => 1.0,
            Base::Eighth => 0.5,
            Base::Sixteenth => 0.25,
            Base::ThirtySecond => 0.125,
        }
    }
}

/// Notates a length of ticks as one value, or a chain of tied values when no single value
/// fits (a note longer than a whole, say). The chain covers the length exactly whenever an
/// exact decomposition exists; only a length finer than the file's own smallest value
/// (a stray blip) falls back to the nearest value, which may overrun slightly.
pub fn notate_duration(ticks: u64, ticks_per_quarter: u64, min_ticks: u64) -> Vec<Duration> {
    if ticks == 0 {
        return Vec::new();
    }
    let values = notatable_values(ticks_per_quarter, min_ticks);
    if let Some(chain) = exact_chain(ticks, &values, ticks_per_quarter) {
        return chain;
    }
    // A length the file's own floor cannot tile (a stray short note or rest) still tiles
    // with the full value set; only a length nothing tiles falls back to the nearest value.
    let finest = notatable_values(ticks_per_quarter, 1);
    exact_chain(ticks, &finest, ticks_per_quarter)
        .unwrap_or_else(|| greedy_chain(ticks, &values, ticks_per_quarter))
}

/// Finds a chain of values summing to exactly `target`, preferring longer values first.
/// None when no exact decomposition exists (the length is not a combination of notatable
/// values), in which case the caller falls back to the nearest value.
fn exact_chain(target: u64, values: &[Duration], ticks_per_quarter: u64) -> Option<Vec<Duration>> {
    let lengths: Vec<u64> = values
        .iter()
        .map(|value| value.ticks(ticks_per_quarter).round() as u64)
        .collect();
    // Values share a common unit (20 ticks at 480 tpq), so scaling by it keeps the table
    // small: one entry per attainable slot, not per tick.
    let unit = lengths.iter().copied().fold(0u64, gcd);
    if unit == 0 || !target.is_multiple_of(unit) {
        return None;
    }
    let steps: Vec<u64> = lengths.iter().map(|length| length / unit).collect();
    let slots = (target / unit) as usize;
    let mut reachable = vec![false; slots + 1];
    reachable[0] = true;
    for slot in 1..=slots {
        reachable[slot] = steps
            .iter()
            .any(|&step| step as usize <= slot && reachable[slot - step as usize]);
    }
    if !reachable[slots] {
        return None;
    }
    let mut chain = Vec::new();
    let mut slot = slots;
    while slot > 0 {
        let index = steps
            .iter()
            .position(|&step| step as usize <= slot && reachable[slot - step as usize])
            .expect("the slot is reachable, so some value steps back onto a reachable slot");
        chain.push(values[index]);
        slot -= steps[index] as usize;
    }
    Some(chain)
}

/// The old greedy walk: take the longest value that fits, then round the remainder to the
/// nearest value. Used only when no exact chain exists.
fn greedy_chain(ticks: u64, values: &[Duration], ticks_per_quarter: u64) -> Vec<Duration> {
    let mut remaining = ticks as f64;
    let mut notated = Vec::new();
    while remaining > 0.0 {
        match values
            .iter()
            .find(|value| value.ticks(ticks_per_quarter) <= remaining)
        {
            Some(value) => {
                remaining -= value.ticks(ticks_per_quarter);
                notated.push(*value);
            }
            None => {
                notated.push(nearest_value(remaining, values, ticks_per_quarter));
                break;
            }
        }
    }
    notated
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Every value the engraver can notate, longest first and none finer than the file's own
/// smallest note value. Dotted 32nds and triplets over a whole note are out of scope.
fn notatable_values(ticks_per_quarter: u64, min_ticks: u64) -> Vec<Duration> {
    let bases = [
        Base::Whole,
        Base::Half,
        Base::Quarter,
        Base::Eighth,
        Base::Sixteenth,
        Base::ThirtySecond,
    ];
    let mut values = Vec::new();
    for base in bases {
        values.push(Duration::Straight {
            base,
            dotted: false,
        });
        // Dotted wholes and dotted 32nds are out of scope.
        if base != Base::Whole && base != Base::ThirtySecond {
            values.push(Duration::Straight { base, dotted: true });
        }
        if base != Base::Whole {
            values.push(Duration::Triplet { base });
        }
    }
    values.sort_by(|a, b| {
        b.ticks(ticks_per_quarter)
            .total_cmp(&a.ticks(ticks_per_quarter))
    });
    values.retain(|value| value.ticks(ticks_per_quarter) + 0.5 >= min_ticks as f64);
    values
}

fn nearest_value(remaining: f64, values: &[Duration], ticks_per_quarter: u64) -> Duration {
    values
        .iter()
        .copied()
        .min_by(|a, b| {
            (a.ticks(ticks_per_quarter) - remaining)
                .total_cmp(&(b.ticks(ticks_per_quarter) - remaining))
        })
        .expect("there is always at least one notatable value")
}

/// The file's own smallest note value: its shortest repeated gap between onsets or note
/// length, rounded to the nearest standard subdivision. Values that appear only once
/// (grace notes, stray blips) are ignored, and the drums are excluded because ghost notes
/// would pull the grid finer than the written music is.
fn derived_min_duration(file: &MidiFile, ticks_per_quarter: u64) -> u64 {
    let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
    let mut streams: BTreeMap<(u16, u8), Vec<u64>> = BTreeMap::new();

    for note in &file.notes {
        if note.channel == PERCUSSION_CHANNEL {
            continue;
        }
        let length = note.end_tick.saturating_sub(note.start_tick);
        if length > 0 {
            *counts.entry(length).or_default() += 1;
        }
        streams
            .entry((note.track, note.channel))
            .or_default()
            .push(note.start_tick);
    }

    for starts in streams.values_mut() {
        starts.sort_unstable();
        starts.dedup();
        for gap in starts.windows(2).map(|pair| pair[1] - pair[0]) {
            if gap > 0 {
                *counts.entry(gap).or_default() += 1;
            }
        }
    }

    // The map iterates shortest first, so the first repeat is the smallest repeated gap.
    let raw = counts
        .iter()
        .find(|(_, count)| **count >= 2)
        .map(|(length, _)| *length)
        .or_else(|| counts.keys().next().copied())
        .unwrap_or(ticks_per_quarter);
    nearest_notated_value(raw, ticks_per_quarter)
}

/// Rounds a raw tick length to the nearest standard subdivision of a quarter note, ties
/// going to the coarser value.
fn nearest_notated_value(length: u64, ticks_per_quarter: u64) -> u64 {
    let mut best = ticks_per_quarter;
    let mut best_error = f64::INFINITY;
    for &subdivisions in &DERIVED_SUBDIVISIONS {
        let value = ticks_per_quarter / subdivisions;
        if value == 0 {
            continue;
        }
        let error = (length as f64 - value as f64).abs();
        if error < best_error {
            best_error = error;
            best = value;
        }
    }
    best
}

fn snap_tick(tick: u64, grid: u64) -> u64 {
    (tick as f64 / grid as f64).round() as u64 * grid
}

/// Shortens a note that runs into the next distinct onset of the same instrument, so
/// legato lines and pedal blur do not turn into overlapping notation. Notes that start
/// together (a chord) are left alone: only a later onset cuts an earlier note. Voices
/// written on one instrument track are treated as one line, so a note held under a moving
/// line on the same track is shortened to the next attack.
fn trim_overlaps(notes: &mut [QuantizedNote]) {
    notes.sort_by_key(|note| (note.track, note.channel, note.start_tick, note.pitch));
    let mut run_start = 0;
    while run_start < notes.len() {
        let stream = (notes[run_start].track, notes[run_start].channel);
        let mut run_end = run_start + 1;
        while run_end < notes.len() && (notes[run_end].track, notes[run_end].channel) == stream {
            run_end += 1;
        }
        for i in run_start..run_end {
            let start = notes[i].start_tick;
            let next = notes[i + 1..run_end]
                .iter()
                .find(|note| note.start_tick > start)
                .map(|note| note.start_tick)
                .filter(|&next| notes[i].end_tick > next);
            if let Some(next) = next {
                notes[i].end_tick = next;
            }
        }
        run_start = run_end;
    }
}

/// Ticks per quarter note; SMPTE-timed files are treated as 480, a standard value.
pub fn ticks_per_quarter(file: &MidiFile) -> u64 {
    match file.division {
        Division::Metrical(tpq) if tpq > 0 => u64::from(tpq),
        _ => DEFAULT_TICKS_PER_QUARTER,
    }
}

#[cfg(test)]
mod tests {
    use super::{Base, Duration, MidiFile, QuantizedNote, notate_duration, quantize};
    use crate::engine::midi::{Division, NoteEvent};

    const TPQ: u64 = 480;

    #[test]
    fn on_the_grid_notes_pass_through() {
        let quantized = quantize(&file(&[(60, 0, 480), (62, 480, 960), (64, 960, 1440)]));

        assert_eq!(quantized.min_duration_ticks, 480);
        assert_eq!(
            quantized.notes,
            vec![
                source_note(0, 60, 0, 480),
                source_note(1, 62, 480, 960),
                source_note(2, 64, 960, 1440),
            ]
        );
    }

    #[test]
    fn humanized_onsets_snap_to_the_grid() {
        let quantized = quantize(&file(&[(60, 2, 483), (62, 478, 962), (64, 958, 1445)]));

        let spans: Vec<(u64, u64)> = quantized
            .notes
            .iter()
            .map(|note| (note.start_tick, note.end_tick))
            .collect();
        assert_eq!(spans, vec![(0, 480), (480, 960), (960, 1440)]);
    }

    #[test]
    fn the_grid_comes_from_the_smallest_repeated_gap() {
        // A melody of eighths over whole-note chords: eighths set the resolution.
        let quantized = quantize(&file(&[
            (36, 0, 1920),
            (60, 0, 240),
            (62, 240, 480),
            (64, 480, 720),
            (65, 720, 960),
        ]));

        assert_eq!(quantized.min_duration_ticks, 240);
    }

    #[test]
    fn triplets_set_a_ternary_grid() {
        // Triplet eighths: three notes in the space of two (160 ticks each at 480 tpq).
        let quantized = quantize(&file(&[(60, 0, 160), (62, 160, 320), (64, 320, 480)]));

        assert_eq!(quantized.min_duration_ticks, 160);
        assert_eq!(
            notate_duration(160, TPQ, quantized.min_duration_ticks),
            vec![Duration::Triplet { base: Base::Eighth }]
        );
        // A straight eighth next to triplets still notates as a straight eighth.
        assert_eq!(
            notate_duration(240, TPQ, quantized.min_duration_ticks),
            vec![Duration::Straight {
                base: Base::Eighth,
                dotted: false,
            }]
        );
    }

    #[test]
    fn a_single_odd_length_does_not_change_the_grid() {
        // One 30-tick blip among quarters: the resolution stays the quarter note.
        let quantized = quantize(&file(&[
            (60, 0, 480),
            (62, 480, 960),
            (64, 960, 990),
            (65, 960, 1440),
        ]));

        assert_eq!(quantized.min_duration_ticks, 480);
    }

    #[test]
    fn the_drums_do_not_pull_the_grid() {
        let notes = vec![(60, 0, 480), (62, 480, 960)];
        let drums = vec![(38, 0, 60), (38, 60, 120), (38, 120, 180)];
        let quantized = quantize(&file_with_drums(&notes, &drums));

        assert_eq!(quantized.min_duration_ticks, 480);
    }

    #[test]
    fn same_pitch_overlap_is_trimmed() {
        let quantized = quantize(&file(&[(60, 0, 700), (60, 480, 960)]));

        assert_eq!(quantized.notes[0].end_tick, 480);
        assert_eq!(quantized.notes[1].end_tick, 960);
    }

    #[test]
    fn a_melodic_legato_overlap_is_trimmed_to_the_next_onset() {
        // Different pitches on one instrument line, each held 20 ticks into the next.
        let quantized = quantize(&file(&[(60, 0, 500), (62, 480, 980), (64, 960, 1440)]));

        let spans: Vec<(u64, u64)> = quantized
            .notes
            .iter()
            .map(|note| (note.start_tick, note.end_tick))
            .collect();
        assert_eq!(spans, vec![(0, 480), (480, 960), (960, 1440)]);
    }

    #[test]
    fn a_chord_with_different_note_lengths_is_left_alone() {
        // Same onset: the longer note is not cut by its shorter chord-mate.
        let quantized = quantize(&file(&[(60, 0, 960), (64, 0, 480)]));

        let spans: Vec<(u64, u64)> = quantized
            .notes
            .iter()
            .map(|note| (note.start_tick, note.end_tick))
            .collect();
        assert_eq!(spans, vec![(0, 960), (0, 480)]);
    }

    #[test]
    fn a_zero_length_note_gets_the_minimum_length() {
        let quantized = quantize(&file(&[(60, 480, 480), (62, 480, 960)]));

        assert_eq!(quantized.notes[0].start_tick, 480);
        assert_eq!(quantized.notes[0].end_tick, 960);
    }

    #[test]
    fn durations_notate_to_the_nearest_value() {
        let min = 40;
        assert_eq!(
            notate_duration(480, TPQ, min),
            vec![Duration::Straight {
                base: Base::Quarter,
                dotted: false,
            }]
        );
        assert_eq!(
            notate_duration(720, TPQ, min),
            vec![Duration::Straight {
                base: Base::Quarter,
                dotted: true,
            }]
        );
        assert_eq!(
            notate_duration(320, TPQ, min),
            vec![Duration::Triplet {
                base: Base::Quarter
            }]
        );
        assert_eq!(
            notate_duration(120, TPQ, min),
            vec![Duration::Straight {
                base: Base::Sixteenth,
                dotted: false,
            }]
        );
    }

    #[test]
    fn a_length_longer_than_a_whole_becomes_tied_values() {
        assert_eq!(
            notate_duration(2400, TPQ, 40),
            vec![
                Duration::Straight {
                    base: Base::Whole,
                    dotted: false,
                },
                Duration::Straight {
                    base: Base::Quarter,
                    dotted: false,
                },
            ]
        );
        assert_eq!(
            notate_duration(2880, TPQ, 40),
            vec![
                Duration::Straight {
                    base: Base::Whole,
                    dotted: false,
                },
                Duration::Straight {
                    base: Base::Half,
                    dotted: false,
                },
            ]
        );
    }

    #[test]
    fn the_derived_floor_keeps_the_notation_coarse() {
        // A file whose smallest value is a 16th never notates finer, so jittered
        // 90-tick lengths become 16ths rather than 32nds.
        assert_eq!(
            notate_duration(90, TPQ, 120),
            vec![Duration::Straight {
                base: Base::Sixteenth,
                dotted: false,
            }]
        );
    }

    #[test]
    fn a_length_that_fits_nothing_still_notates() {
        let notated = notate_duration(10, TPQ, 240);
        assert_eq!(notated.len(), 1);
        assert!(notated[0].ticks(TPQ) >= 240.0);
    }

    #[test]
    fn a_length_is_tiled_exactly_when_it_can_be() {
        // 460 = dotted eighth + 32nd + triplet 32nd; the old greedy walk overshot to 480.
        let notated = notate_duration(460, TPQ, 40);
        let total: f64 = notated.iter().map(|value| value.ticks(TPQ)).sum();
        assert_eq!(total, 460.0);

        // Every multiple of the 20-tick grid at least as long as the floor tiles exactly.
        for ticks in (40..=2000).step_by(20) {
            let notated = notate_duration(ticks, TPQ, 40);
            let total: f64 = notated.iter().map(|value| value.ticks(TPQ)).sum();
            assert_eq!(total, ticks as f64, "length {ticks} did not tile exactly");
        }
    }

    fn source_note(source: usize, pitch: u8, start_tick: u64, end_tick: u64) -> QuantizedNote {
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

    fn file(notes: &[(u8, u64, u64)]) -> MidiFile {
        mixed_file(notes)
    }

    /// Builds a file where every note is on channel 0 track 0, i.e. all one instrument.
    fn mixed_file(notes: &[(u8, u64, u64)]) -> MidiFile {
        file_with_drums(notes, &[])
    }

    /// Adds percussion notes, which live on channel 9 and must not influence the grid.
    fn file_with_drums(notes: &[(u8, u64, u64)], drums: &[(u8, u64, u64)]) -> MidiFile {
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
        events.extend(drums.iter().map(|(pitch, start, end)| NoteEvent {
            track: 1,
            channel: 9,
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
            lyrics: Vec::new(),
            tracks: Vec::new(),
            notes: events,
        }
    }
}
