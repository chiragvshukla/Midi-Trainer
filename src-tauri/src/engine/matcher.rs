//! Judges the notes the player presses against the piece being played, in real time.
//!
//! The audio thread reports each pressed note with the score position it landed on; this module
//! compares it with the notes the piece expects there and reuses `scoring` for the verdict and
//! the timing marker.

use serde::Serialize;

use super::scoring::{Judge, NoteMatch, Timing, Verdict};
use super::timeline::Timeline;

/// One note the piece expects the player to play.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ExpectedNote {
    pitch: u8,
    start_ms: f64,
    matched: bool,
}

/// The result of judging one played note, as the frontend sees it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NoteJudgement {
    pub pitch: u8,
    pub verdict: Verdict,
    pub timing: Timing,
    /// Played time minus expected time in milliseconds; negative is early.
    pub offset_ms: i64,
    /// The pitch the piece expected, when one was due.
    pub expected_pitch: Option<u8>,
}

/// Matches the player's notes against a piece, one pressed note at a time.
#[derive(Debug)]
pub struct LiveMatcher {
    notes: Vec<ExpectedNote>,
    /// Index of the first note that could still be matched; everything before it is too late.
    cursor: usize,
    judge: Judge,
}

impl LiveMatcher {
    /// Builds a matcher expecting the given notes, as `(pitch, start_ms)` pairs in time order.
    pub fn new(notes: Vec<(u8, f64)>, judge: Judge) -> Self {
        LiveMatcher {
            notes: notes
                .into_iter()
                .map(|(pitch, start_ms)| ExpectedNote {
                    pitch,
                    start_ms,
                    matched: false,
                })
                .collect(),
            cursor: 0,
            judge,
        }
    }

    /// Builds the matcher for a piece: every note in its timeline is expected.
    pub fn for_timeline(timeline: &Timeline) -> Self {
        Self::new(
            timeline
                .notes
                .iter()
                .map(|note| (note.pitch, note.start_ms))
                .collect(),
            Judge::default(),
        )
    }

    /// Judges a note the player pressed at `position_ms`, or `None` when nothing was due there.
    pub fn evaluate(&mut self, position_ms: f64, pitch: u8) -> Option<NoteJudgement> {
        let window = self.judge.window_ms as f64;
        // Notes that began before the window can no longer be matched.
        while self
            .notes
            .get(self.cursor)
            .is_some_and(|note| note.start_ms < position_ms - window)
        {
            self.cursor += 1;
        }

        // Notes are sorted by start, so the window is a run: the first unplayed note with the
        // right pitch wins, otherwise the earliest unplayed note is what should have been played.
        let mut nearest: Option<usize> = None;
        let mut index = self.cursor;
        while let Some(note) = self.notes.get(index) {
            if note.start_ms > position_ms + window {
                break;
            }
            if !note.matched {
                if note.pitch == pitch {
                    return Some(self.judgement(index, position_ms, pitch));
                }
                if nearest.is_none() {
                    nearest = Some(index);
                }
            }
            index += 1;
        }

        nearest.map(|index| self.judgement(index, position_ms, pitch))
    }

    /// Marks the note at `index` as played and describes how the attempt landed.
    fn judgement(&mut self, index: usize, position_ms: f64, pitch: u8) -> NoteJudgement {
        let note = &mut self.notes[index];
        note.matched = true;
        let expected_pitch = note.pitch;
        let offset_ms = (position_ms - note.start_ms).round() as i64;
        let NoteMatch {
            verdict,
            timing,
            offset_ms,
        } = self.judge.judge(expected_pitch, pitch, offset_ms);
        NoteJudgement {
            pitch,
            verdict,
            timing,
            offset_ms,
            expected_pitch: Some(expected_pitch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Judge, LiveMatcher};

    fn matcher(notes: &[(u8, f64)]) -> LiveMatcher {
        LiveMatcher::new(notes.to_vec(), Judge::default())
    }

    #[test]
    fn a_note_played_on_time_is_correct_and_on_the_beat() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        let judgement = matcher.evaluate(1020.0, 60).expect("the note was due");

        assert_eq!(judgement.verdict, super::Verdict::Correct);
        assert_eq!(judgement.timing, super::Timing::OnTime);
        assert_eq!(judgement.offset_ms, 20);
        assert_eq!(judgement.expected_pitch, Some(60));
    }

    #[test]
    fn a_note_played_early_is_reported_as_ahead() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        let judgement = matcher.evaluate(900.0, 60).expect("the note was due");

        assert_eq!(judgement.verdict, super::Verdict::Correct);
        assert_eq!(judgement.timing, super::Timing::Ahead);
        assert_eq!(judgement.offset_ms, -100);
    }

    #[test]
    fn a_wrong_pitch_reports_what_was_expected() {
        let mut matcher = matcher(&[(62, 1000.0)]);

        let judgement = matcher.evaluate(1000.0, 60).expect("a note was due");

        assert_eq!(judgement.verdict, super::Verdict::Wrong);
        assert_eq!(judgement.expected_pitch, Some(62));
    }

    #[test]
    fn nothing_is_reported_when_no_note_is_due() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        assert_eq!(matcher.evaluate(100.0, 60), None);
    }

    #[test]
    fn a_note_cannot_be_matched_twice() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        assert!(matcher.evaluate(1000.0, 60).is_some());
        assert_eq!(matcher.evaluate(1010.0, 60), None);
    }

    #[test]
    fn chords_match_each_note_once() {
        let mut matcher = matcher(&[(60, 1000.0), (64, 1000.0)]);

        assert_eq!(
            matcher.evaluate(1000.0, 64).map(|j| j.verdict),
            Some(super::Verdict::Correct)
        );
        // The C is still there to be played.
        assert_eq!(
            matcher.evaluate(1010.0, 60).map(|j| j.verdict),
            Some(super::Verdict::Correct)
        );
    }

    #[test]
    fn notes_older_than_the_window_stop_being_matchable() {
        let mut matcher = matcher(&[(60, 0.0), (64, 5000.0)]);

        // Nothing is played at the first note; by the time the player presses, it is gone.
        assert_eq!(
            matcher.evaluate(5000.0, 60).map(|j| j.verdict),
            Some(super::Verdict::Wrong)
        );
    }
}
