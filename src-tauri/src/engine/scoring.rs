/// How far a played note may sit from the expected note and still count as correct.
pub const DEFAULT_WINDOW_MS: i64 = 250;

/// Offsets within this range count as on time.
pub const DEFAULT_ON_TIME_WINDOW_MS: i64 = 60;

/// Whether a played note matched the note that was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    Wrong,
}

/// Where a played note landed relative to the beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    Ahead,
    OnTime,
    Behind,
}

/// The result of comparing one played note with the expected note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteMatch {
    pub verdict: Verdict,
    pub timing: Timing,
    /// Played time minus expected time in milliseconds; negative is early.
    pub offset_ms: i64,
}

/// The tolerances used to judge played notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Judge {
    /// Pitch matches further away than this count as wrong.
    pub window_ms: i64,
    /// Offsets within this range count as on time.
    pub on_time_window_ms: i64,
}

impl Default for Judge {
    fn default() -> Self {
        Judge {
            window_ms: DEFAULT_WINDOW_MS,
            on_time_window_ms: DEFAULT_ON_TIME_WINDOW_MS,
        }
    }
}

impl Judge {
    /// Compares a played note with the note expected at that moment.
    pub fn judge(&self, expected_pitch: u8, played_pitch: u8, offset_ms: i64) -> NoteMatch {
        let verdict = if expected_pitch == played_pitch && offset_ms.abs() <= self.window_ms {
            Verdict::Correct
        } else {
            Verdict::Wrong
        };
        NoteMatch {
            verdict,
            timing: timing_of(offset_ms, self.on_time_window_ms),
            offset_ms,
        }
    }
}

/// Running tally for a practice session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub correct: u32,
    pub wrong: u32,
    pub ahead: u32,
    pub on_time: u32,
    pub behind: u32,
    sum_abs_offset_ms: i64,
}

impl Score {
    pub fn record(&mut self, matched: NoteMatch) {
        match matched.verdict {
            Verdict::Correct => self.correct += 1,
            Verdict::Wrong => self.wrong += 1,
        }
        match matched.timing {
            Timing::Ahead => self.ahead += 1,
            Timing::OnTime => self.on_time += 1,
            Timing::Behind => self.behind += 1,
        }
        self.sum_abs_offset_ms += matched.offset_ms.abs();
    }

    /// Notes played so far.
    pub fn attempts(&self) -> u32 {
        self.correct + self.wrong
    }

    /// Share of attempts that were correct, 0.0 when nothing has been played.
    pub fn accuracy(&self) -> f32 {
        let attempts = self.attempts();
        if attempts == 0 {
            0.0
        } else {
            self.correct as f32 / attempts as f32
        }
    }

    /// Mean distance from the beat in milliseconds, 0.0 when nothing has been played.
    pub fn average_abs_offset_ms(&self) -> f32 {
        let attempts = self.attempts();
        if attempts == 0 {
            0.0
        } else {
            self.sum_abs_offset_ms as f32 / attempts as f32
        }
    }

    pub fn reset(&mut self) {
        *self = Score::default();
    }
}

/// Negative offset is early (ahead of the beat), positive is late.
fn timing_of(offset_ms: i64, on_time_window_ms: i64) -> Timing {
    if offset_ms < -on_time_window_ms {
        Timing::Ahead
    } else if offset_ms > on_time_window_ms {
        Timing::Behind
    } else {
        Timing::OnTime
    }
}

#[cfg(test)]
mod tests {
    use super::{Judge, Score, Timing, Verdict};

    #[test]
    fn correct_pitch_inside_the_window_is_correct() {
        let judge = Judge::default();

        for offset in [0, 100, 250, -250] {
            let matched = judge.judge(60, 60, offset);
            assert_eq!(matched.verdict, Verdict::Correct, "offset {offset}");
        }
    }

    #[test]
    fn wrong_pitch_is_wrong_even_when_on_time() {
        let judge = Judge::default();

        let matched = judge.judge(60, 62, 0);
        assert_eq!(matched.verdict, Verdict::Wrong);
        assert_eq!(matched.timing, Timing::OnTime);
        assert_eq!(matched.offset_ms, 0);
    }

    #[test]
    fn correct_pitch_outside_the_window_is_wrong() {
        let judge = Judge::default();

        assert_eq!(judge.judge(60, 60, 251).verdict, Verdict::Wrong);
        assert_eq!(judge.judge(60, 60, -251).verdict, Verdict::Wrong);
    }

    #[test]
    fn timing_reports_ahead_behind_and_on_time() {
        let judge = Judge::default();

        assert_eq!(judge.judge(60, 60, -100).timing, Timing::Ahead);
        assert_eq!(judge.judge(60, 60, -60).timing, Timing::OnTime);
        assert_eq!(judge.judge(60, 60, 60).timing, Timing::OnTime);
        assert_eq!(judge.judge(60, 60, 61).timing, Timing::Behind);
    }

    #[test]
    fn score_tallies_verdicts_and_timing() {
        let judge = Judge::default();
        let mut score = Score::default();

        score.record(judge.judge(60, 60, 0));
        score.record(judge.judge(62, 62, 150));
        score.record(judge.judge(64, 65, 300));

        assert_eq!(score.correct, 2);
        assert_eq!(score.wrong, 1);
        assert_eq!(score.attempts(), 3);
        assert_eq!(score.on_time, 1);
        assert_eq!(score.ahead, 0);
        assert_eq!(score.behind, 2);
        assert!((score.accuracy() - 2.0 / 3.0).abs() < 1e-6);
        assert!((score.average_abs_offset_ms() - 150.0).abs() < 1e-6);
    }

    #[test]
    fn empty_score_reports_zero() {
        let score = Score::default();

        assert_eq!(score.attempts(), 0);
        assert_eq!(score.accuracy(), 0.0);
        assert_eq!(score.average_abs_offset_ms(), 0.0);
    }

    #[test]
    fn reset_clears_the_score() {
        let judge = Judge::default();
        let mut score = Score::default();
        score.record(judge.judge(60, 60, 0));

        score.reset();

        assert_eq!(score, Score::default());
    }
}
