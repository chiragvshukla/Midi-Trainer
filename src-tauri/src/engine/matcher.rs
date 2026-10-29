//! Judges the notes the player presses against the piece being played, in real time.
//!
//! The audio thread reports each pressed note with the score position it landed on; this module
//! compares it with the notes the piece expects there and reuses `scoring` for the verdict and
//! the timing marker.

use serde::Serialize;

use super::scheduler::{Part, player_plays};
use super::scoring::{Judge, NoteMatch, PracticeStats, Score, Timing, Verdict};
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
    /// Score time of the expected note, so the frontend can find it in the piece.
    pub expected_start_ms: f64,
    /// True when the note was never played and was judged as it expired, rather than by a press.
    pub missed: bool,
}

/// Matches the player's notes against a piece, one pressed note at a time.
#[derive(Debug)]
pub struct LiveMatcher {
    notes: Vec<ExpectedNote>,
    /// Index of the first note that could still be matched; everything before it is too late.
    cursor: usize,
    judge: Judge,
    /// Running tally of the player's attempts, for the practice readout.
    score: Score,
    /// Length of the piece in score milliseconds; 0 when unknown.
    duration_ms: f64,
}

impl LiveMatcher {
    /// Builds a matcher expecting the given notes, as `(pitch, start_ms)` pairs in time order.
    pub fn new(notes: Vec<(u8, f64)>, judge: Judge, duration_ms: f64) -> Self {
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
            score: Score::default(),
            duration_ms,
        }
    }

    /// Builds the matcher for a piece: every note the player is expected to play for `part`. The
    /// app plays the percussion itself, so drums are never expected from the player.
    pub fn for_timeline(timeline: &Timeline, part: Part) -> Self {
        Self::new(
            timeline
                .notes
                .iter()
                .filter(|note| player_plays(note, part))
                .map(|note| (note.pitch, note.start_ms))
                .collect(),
            Judge::default(),
            timeline.duration_ms,
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
        let (expected_pitch, expected_start_ms) = (note.pitch, note.start_ms);
        let offset_ms = (position_ms - expected_start_ms).round() as i64;
        let matched = self.judge.judge(expected_pitch, pitch, offset_ms);
        self.score.record(matched);
        let NoteMatch {
            verdict,
            timing,
            offset_ms,
        } = matched;
        NoteJudgement {
            pitch,
            verdict,
            timing,
            offset_ms,
            expected_pitch: Some(expected_pitch),
            expected_start_ms,
            missed: false,
        }
    }

    /// Judges every expected note whose window has passed without a press as a missed wrong note,
    /// so a note that is skipped is scored like any other and every note of a chord is scored on
    /// its own. Returns the notes that expired since the last call.
    pub fn expire(&mut self, position_ms: f64) -> Vec<NoteJudgement> {
        let window = self.judge.window_ms as f64;
        self.flush(|note| note.start_ms < position_ms - window)
    }

    /// Judges every note still expected once the playhead has reached the end of the piece, so a
    /// note that starts within the judging window of the end is scored rather than left unjudged
    /// when playback stops. Does nothing before the end.
    pub fn expire_at_end(&mut self, position_ms: f64) -> Vec<NoteJudgement> {
        if self.duration_ms > 0.0 && position_ms >= self.duration_ms {
            self.flush(|_| true)
        } else {
            Vec::new()
        }
    }

    /// Marks every due note from the cursor on as a missed wrong note, one judgement each, and
    /// returns them. `due` decides how far the run reaches.
    fn flush(&mut self, due: impl Fn(&ExpectedNote) -> bool) -> Vec<NoteJudgement> {
        let mut expired = Vec::new();
        // The same run the evaluate cursor skips over would drop silently; here each is judged.
        while let Some(&note) = self.notes.get(self.cursor) {
            if !due(&note) {
                break;
            }
            if !note.matched {
                self.notes[self.cursor].matched = true;
                self.score.record_missed();
                expired.push(NoteJudgement {
                    pitch: note.pitch,
                    verdict: Verdict::Wrong,
                    timing: Timing::OnTime,
                    offset_ms: 0,
                    expected_pitch: Some(note.pitch),
                    expected_start_ms: note.start_ms,
                    missed: true,
                });
            }
            self.cursor += 1;
        }
        expired
    }

    /// The running practice tallies, for the frontend's accuracy readout.
    pub fn stats(&self) -> PracticeStats {
        self.score.stats()
    }

    /// Moves the matcher back to `position_ms`: every note from here on can be judged again and
    /// the tally starts over. Used when the playhead rewinds or a loop repeats, so a replayed
    /// section is scored fresh instead of counting as missed.
    pub fn restart(&mut self, position_ms: f64) {
        let window = self.judge.window_ms as f64;
        self.cursor = self
            .notes
            .partition_point(|note| note.start_ms < position_ms - window);
        for note in &mut self.notes[self.cursor..] {
            note.matched = false;
        }
        self.score = Score::default();
    }
}

#[cfg(test)]
mod tests {
    use super::{Judge, LiveMatcher};

    fn matcher(notes: &[(u8, f64)]) -> LiveMatcher {
        LiveMatcher::new(notes.to_vec(), Judge::default(), 0.0)
    }

    /// A matcher for a piece of the given length, so the end-of-piece flush can be exercised.
    fn matcher_of_length(notes: &[(u8, f64)], duration_ms: f64) -> LiveMatcher {
        LiveMatcher::new(notes.to_vec(), Judge::default(), duration_ms)
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
    fn stats_count_each_attempt() {
        let mut matcher = matcher(&[(60, 1000.0), (64, 1000.0), (67, 2000.0)]);

        matcher.evaluate(900.0, 60); // correct, early
        matcher.evaluate(1000.0, 62); // wrong pitch
        matcher.evaluate(2100.0, 67); // correct, late

        let stats = matcher.stats();
        assert_eq!(stats.attempts, 3);
        assert_eq!(stats.correct, 2);
        assert_eq!(stats.wrong, 1);
        assert_eq!(stats.ahead, 1);
        assert_eq!(stats.on_time, 1);
        assert_eq!(stats.behind, 1);
    }

    #[test]
    fn restart_makes_a_section_matchable_again() {
        let mut matcher = matcher(&[(60, 1000.0), (64, 2000.0)]);
        matcher.evaluate(1000.0, 60);
        matcher.evaluate(2000.0, 64);
        assert_eq!(matcher.stats().attempts, 2);

        // Rewinding to the second note leaves the first matched but forgets the second, and the
        // tally starts over.
        matcher.restart(2000.0);
        assert_eq!(
            matcher.evaluate(2000.0, 64).map(|j| j.verdict),
            Some(super::Verdict::Correct)
        );
        assert_eq!(matcher.stats().attempts, 1);

        // Rewinding to the start makes both matchable again.
        matcher.restart(0.0);
        assert!(matcher.evaluate(1000.0, 60).is_some());
        assert!(matcher.evaluate(2000.0, 64).is_some());
        assert_eq!(matcher.stats().attempts, 2);
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

    #[test]
    fn a_note_that_is_never_played_expires_as_missed_and_wrong() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        // Still matchable at its window edge, so a plainly late press can still count.
        assert!(matcher.expire(1250.0).is_empty());
        let expired = matcher.expire(1261.0);

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].verdict, super::Verdict::Wrong);
        assert!(expired[0].missed);
        assert_eq!(expired[0].expected_pitch, Some(60));
        assert_eq!(expired[0].expected_start_ms, 1000.0);
        assert_eq!(matcher.stats().wrong, 1);
    }

    #[test]
    fn every_note_of_a_missed_chord_expires_on_its_own() {
        let mut matcher = matcher(&[(60, 1000.0), (64, 1000.0), (67, 1000.0)]);

        let expired = matcher.expire(1300.0);

        let pitches: Vec<u8> = expired.iter().map(|j| j.expected_pitch.unwrap()).collect();
        assert_eq!(pitches, vec![60, 64, 67]);
        assert!(expired.iter().all(|j| j.verdict == super::Verdict::Wrong));
        assert_eq!(matcher.stats().wrong, 3);
        // A second call has nothing left to expire.
        assert!(matcher.expire(1300.0).is_empty());
        assert_eq!(matcher.stats().wrong, 3);
    }

    #[test]
    fn a_played_note_does_not_expire() {
        let mut matcher = matcher(&[(60, 1000.0), (64, 1000.0)]);

        matcher.evaluate(1000.0, 60);
        let expired = matcher.expire(1300.0);

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].expected_pitch, Some(64));
        assert_eq!(matcher.stats().correct, 1);
        assert_eq!(matcher.stats().wrong, 1);
    }

    #[test]
    fn restart_makes_an_expired_note_matchable_again() {
        let mut matcher = matcher(&[(60, 1000.0)]);

        matcher.expire(1300.0);
        assert_eq!(matcher.stats().wrong, 1);

        matcher.restart(1000.0);
        assert_eq!(matcher.stats().wrong, 0);
        assert_eq!(
            matcher.evaluate(1000.0, 60).map(|j| j.verdict),
            Some(super::Verdict::Correct)
        );
    }

    #[test]
    fn a_note_within_the_window_of_the_end_is_scored_when_the_piece_finishes() {
        // A 10 ms note ending at 1000: its start sits inside the 250 ms window of the end, so
        // the ordinary expiry never reaches it.
        let mut matcher = matcher_of_length(&[(60, 990.0)], 1000.0);

        assert!(matcher.expire(1000.0).is_empty());
        let expired = matcher.expire_at_end(1000.0);

        assert_eq!(expired.len(), 1);
        assert!(expired[0].missed);
        assert_eq!(expired[0].verdict, super::Verdict::Wrong);
        // A finished piece has nothing left to flush, so a later poll is a no-op.
        assert!(matcher.expire_at_end(1000.0).is_empty());
    }

    #[test]
    fn the_end_flush_does_nothing_before_the_piece_is_over() {
        let mut matcher = matcher_of_length(&[(60, 1000.0)], 2000.0);

        assert!(matcher.expire_at_end(1000.0).is_empty());
        assert_eq!(matcher.stats().attempts, 0);
    }

    #[test]
    fn a_played_note_is_not_flushed_by_the_end() {
        let mut matcher = matcher_of_length(&[(60, 990.0)], 1000.0);

        matcher.evaluate(990.0, 60);
        assert!(matcher.expire_at_end(1000.0).is_empty());
        assert_eq!(matcher.stats().correct, 1);
        assert_eq!(matcher.stats().wrong, 0);
    }
}
