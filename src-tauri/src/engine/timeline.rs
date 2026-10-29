use std::path::Path;

use super::EngineError;
use super::midi::{self, Division, MidiFile, TempoChange, TrackInfo};
/// A note placed on the real-time timeline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub track: u16,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
    pub start_ms: f64,
    pub end_ms: f64,
}

/// A lyric syllable placed on the real-time timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricAt {
    pub time_ms: f64,
    pub text: String,
}

/// A MIDI file converted to milliseconds through its tempo map. This is the form the
/// displays and the playback scheduler consume.
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    /// Sorted by start time, then pitch.
    pub notes: Vec<Note>,
    pub tracks: Vec<TrackInfo>,
    pub lyrics: Vec<LyricAt>,
    /// End of the last note to finish, in milliseconds.
    pub duration_ms: f64,
    timebase: Timebase,
}

/// Converts tick positions to milliseconds.
#[derive(Debug, Clone, PartialEq)]
struct Timebase {
    division: Division,
    tempo_map: Vec<TempoChange>,
}

impl Timeline {
    /// Converts a parsed file to milliseconds using its tempo map.
    pub fn from_midi(file: &MidiFile) -> Self {
        let timebase = Timebase {
            division: file.division,
            tempo_map: file.tempo_map.clone(),
        };

        let mut notes: Vec<Note> = file
            .notes
            .iter()
            .map(|note| Note {
                track: note.track,
                channel: note.channel,
                pitch: note.pitch,
                velocity: note.velocity,
                start_ms: timebase.ms_at(note.start_tick),
                end_ms: timebase.ms_at(note.end_tick),
            })
            .collect();
        notes.sort_by(|a, b| {
            a.start_ms
                .total_cmp(&b.start_ms)
                .then_with(|| a.pitch.cmp(&b.pitch))
        });

        let lyrics = file
            .lyrics
            .iter()
            .map(|lyric| LyricAt {
                time_ms: timebase.ms_at(lyric.tick),
                text: lyric.text.clone(),
            })
            .collect();

        let duration_ms = notes.iter().map(|note| note.end_ms).fold(0.0, f64::max);

        Timeline {
            notes,
            tracks: file.tracks.clone(),
            lyrics,
            duration_ms,
            timebase,
        }
    }

    /// Parses `.mid` and `.kar` data straight to a timeline.
    pub fn parse(bytes: &[u8]) -> Result<Self, EngineError> {
        Ok(Timeline::from_midi(&midi::parse(bytes)?))
    }

    /// Reads and parses a file from disk.
    pub fn load(path: &Path) -> Result<Self, EngineError> {
        Timeline::parse(&std::fs::read(path)?)
    }

    /// Milliseconds at a tick position, following every tempo change.
    pub fn tick_to_ms(&self, tick: u64) -> f64 {
        self.timebase.ms_at(tick)
    }
}

impl Timebase {
    fn ms_at(&self, tick: u64) -> f64 {
        match self.division {
            Division::Metrical(ticks_per_quarter) => self.metrical_ms(tick, ticks_per_quarter),
            Division::Timecode {
                fps,
                ticks_per_frame,
            } => {
                let ticks_per_second = f64::from(fps) * f64::from(ticks_per_frame);
                if ticks_per_second <= 0.0 {
                    0.0
                } else {
                    tick as f64 * 1000.0 / ticks_per_second
                }
            }
        }
    }

    /// Adds up every tempo segment up to the tick.
    fn metrical_ms(&self, tick: u64, ticks_per_quarter: u16) -> f64 {
        if ticks_per_quarter == 0 {
            return 0.0;
        }
        let ms_per_tick = |micros_per_quarter: u32| {
            f64::from(micros_per_quarter) / (f64::from(ticks_per_quarter) * 1000.0)
        };

        let mut ms = 0.0;
        let mut previous_tick = 0u64;
        let mut current = self
            .tempo_map
            .first()
            .map_or(midi::DEFAULT_MICROS_PER_QUARTER, |change| {
                change.micros_per_quarter
            });

        for change in &self.tempo_map {
            if change.tick >= tick {
                break;
            }
            ms += (change.tick - previous_tick) as f64 * ms_per_tick(current);
            previous_tick = change.tick;
            current = change.micros_per_quarter;
        }

        ms + (tick - previous_tick) as f64 * ms_per_tick(current)
    }
}

#[cfg(test)]
mod tests {
    use super::{LyricAt, Timeline};
    use crate::engine::midi::{Division, MidiFile, NoteEvent, TempoChange};

    const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../test_data");

    #[test]
    fn converts_ticks_across_a_tempo_change() {
        let timeline = Timeline::load(&fixture("two-track-scale.mid")).unwrap();

        assert_ms(timeline.tick_to_ms(0), 0.0);
        assert_ms(timeline.tick_to_ms(480), 500.0);
        assert_ms(timeline.tick_to_ms(960), 1000.0);
        assert_ms(timeline.tick_to_ms(1440), 1250.0);
        assert_ms(timeline.tick_to_ms(1920), 1500.0);
    }

    #[test]
    fn places_notes_in_milliseconds() {
        let timeline = Timeline::load(&fixture("two-track-scale.mid")).unwrap();

        let spans: Vec<(f64, f64)> = timeline
            .notes
            .iter()
            .map(|note| (note.start_ms, note.end_ms))
            .collect();
        let expected = [
            (0.0, 500.0),
            (500.0, 1000.0),
            (1000.0, 1250.0),
            (1250.0, 1500.0),
        ];
        assert_eq!(spans.len(), expected.len());
        for (span, want) in spans.iter().zip(expected) {
            assert_ms(span.0, want.0);
            assert_ms(span.1, want.1);
        }
        assert_ms(timeline.duration_ms, 1500.0);
        assert_eq!(timeline.tracks.len(), 2);
    }

    #[test]
    fn places_lyrics_in_milliseconds() {
        let timeline = Timeline::load(&fixture("karaoke.kar")).unwrap();

        assert_eq!(
            timeline.lyrics,
            vec![
                LyricAt {
                    time_ms: 0.0,
                    text: "Hel-".to_string()
                },
                LyricAt {
                    time_ms: 600.0,
                    text: "lo ".to_string()
                },
            ]
        );
        assert_ms(timeline.duration_ms, 1200.0);
    }

    #[test]
    fn a_note_spanning_a_tempo_change_adds_both_segments() {
        let file = midi_file(
            Division::Metrical(480),
            vec![tempo(0, 500_000), tempo(960, 250_000)],
            vec![note(0, 60, 0, 1440)],
        );

        let timeline = Timeline::from_midi(&file);

        assert_eq!(timeline.notes.len(), 1);
        assert_ms(timeline.notes[0].start_ms, 0.0);
        assert_ms(timeline.notes[0].end_ms, 1250.0);
    }

    #[test]
    fn defaults_to_120_bpm_without_tempo_events() {
        let file = midi_file(Division::Metrical(480), vec![], vec![note(0, 60, 0, 960)]);

        let timeline = Timeline::from_midi(&file);

        assert_ms(timeline.tick_to_ms(960), 1000.0);
        assert_ms(timeline.duration_ms, 1000.0);
    }

    #[test]
    fn converts_smpte_ticks_to_milliseconds() {
        let file = midi_file(
            Division::Timecode {
                fps: 25.0,
                ticks_per_frame: 40,
            },
            vec![],
            vec![note(0, 60, 0, 500)],
        );

        let timeline = Timeline::from_midi(&file);

        // 25 frames per second with 40 ticks per frame is 1000 ticks per second.
        assert_ms(timeline.tick_to_ms(40), 40.0);
        assert_ms(timeline.notes[0].end_ms, 500.0);
    }

    #[test]
    fn sorts_notes_by_start_time() {
        let file = midi_file(
            Division::Metrical(480),
            vec![tempo(0, 500_000)],
            vec![note(0, 72, 960, 1440), note(0, 60, 0, 480)],
        );

        let timeline = Timeline::from_midi(&file);

        assert_eq!(
            timeline
                .notes
                .iter()
                .map(|note| note.pitch)
                .collect::<Vec<_>>(),
            vec![60, 72]
        );
    }

    #[test]
    fn an_empty_file_has_no_duration() {
        let file = midi_file(Division::Metrical(480), vec![tempo(0, 500_000)], vec![]);

        let timeline = Timeline::from_midi(&file);

        assert!(timeline.notes.is_empty());
        assert_ms(timeline.duration_ms, 0.0);
    }

    fn assert_ms(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected} ms, got {actual} ms"
        );
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(FIXTURE_DIR).join(name)
    }

    fn midi_file(
        division: Division,
        tempo_map: Vec<TempoChange>,
        notes: Vec<NoteEvent>,
    ) -> MidiFile {
        MidiFile {
            format: 0,
            division,
            tempo_map,
            time_signatures: Vec::new(),
            lyrics: Vec::new(),
            tracks: Vec::new(),
            notes,
        }
    }

    fn tempo(tick: u64, micros_per_quarter: u32) -> TempoChange {
        TempoChange {
            tick,
            micros_per_quarter,
        }
    }

    fn note(track: u16, pitch: u8, start_tick: u64, end_tick: u64) -> NoteEvent {
        NoteEvent {
            track,
            channel: 0,
            pitch,
            velocity: 100,
            start_tick,
            end_tick,
        }
    }
}
