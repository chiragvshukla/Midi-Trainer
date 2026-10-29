use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use super::AudioError;
use super::event::AudioCommand;

/// Master make-up gain targets this peak for the reference note, leaving headroom below full
/// scale before the soft limiter. A quiet font is lifted and a hot one pulled down, so no font is
/// driven into saturation by a one-size-fits-all gain.
const REFERENCE_TARGET_PEAK: f32 = 0.5;

/// Bounds on the per-font make-up gain. The floor pulls a hot font down toward the target; the
/// ceiling stops a near-silent or broken font running away.
const MIN_MAKEUP_GAIN: f32 = 0.25;
const MAX_MAKEUP_GAIN: f32 = 16.0;

/// The reference the probe plays to gauge a font's level: a loud middle C on the piano (GM
/// program 0), the loudest realistic case a piano trainer cares about.
const REFERENCE_PROGRAM: u8 = 0;
const REFERENCE_PITCH: u8 = 60;
const REFERENCE_VELOCITY: u8 = 127;

/// Seconds of audio rendered to measure the reference peak.
const PROBE_SECONDS: f32 = 0.3;

/// Frames rendered per probe block.
const PROBE_BLOCK: usize = 1024;

/// Something that turns commands into sound. Implemented by the SoundFont synthesizer and
/// by test doubles, so the render loop can be checked without an audio device.
pub trait Synth: Send {
    /// Applies a command. Runs on the audio thread, so it must not block or allocate.
    fn apply(&mut self, command: AudioCommand);

    /// Renders one block into `left` and `right`, which must be the same length.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);

    /// The master make-up gain that brings this synth's output to a usable level. Fixed per font.
    fn makeup(&self) -> f32;
}

/// A SoundFont-backed synthesizer. It is owned by the audio thread and never leaves it.
pub struct SoundFontSynth {
    synthesizer: Synthesizer,
    makeup: f32,
}

impl SoundFontSynth {
    /// Loads a `.sf2` file, measures its level, and prepares synthesis at the output sample rate.
    pub fn load(path: &Path, sample_rate: u32) -> Result<Self, AudioError> {
        let mut file = File::open(path)?;
        let sound_font = Arc::new(SoundFont::new(&mut file)?);
        let settings = SynthesizerSettings::new(sample_rate as i32);
        let makeup = makeup_for_peak(probe_peak(&sound_font, sample_rate));
        let synthesizer = Synthesizer::new(&sound_font, &settings)?;
        Ok(SoundFontSynth {
            synthesizer,
            makeup,
        })
    }
}

impl Synth for SoundFontSynth {
    fn apply(&mut self, command: AudioCommand) {
        match command.message() {
            Some(message) => self.synthesizer.process_midi_message(
                message.channel,
                message.status,
                message.data1,
                message.data2,
            ),
            None => self.synthesizer.note_off_all(false),
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.synthesizer.render(left, right);
    }

    fn makeup(&self) -> f32 {
        self.makeup
    }
}

/// The make-up gain that brings a font measuring `peak` to the target level. A font that measured
/// nothing (silent or unreadable) is treated as very quiet, so it is not left inaudible.
fn makeup_for_peak(peak: f32) -> f32 {
    if !peak.is_finite() || peak <= 0.0 {
        return MAX_MAKEUP_GAIN;
    }
    (REFERENCE_TARGET_PEAK / peak).clamp(MIN_MAKEUP_GAIN, MAX_MAKEUP_GAIN)
}

/// Renders one loud reference note on a throwaway synth and returns the peak level it reached, so
/// a font's mastering level can be measured before it is played. The throwaway keeps the probe's
/// notes out of the synth that goes on to play.
fn probe_peak(sound_font: &Arc<SoundFont>, sample_rate: u32) -> f32 {
    let Ok(mut synth) = Synthesizer::new(sound_font, &SynthesizerSettings::new(sample_rate as i32))
    else {
        return 0.0;
    };
    synth.process_midi_message(0, 0xC0, i32::from(REFERENCE_PROGRAM), 0);
    synth.process_midi_message(
        0,
        0x90,
        i32::from(REFERENCE_PITCH),
        i32::from(REFERENCE_VELOCITY),
    );

    let frames = (sample_rate as f32 * PROBE_SECONDS) as usize;
    let mut left = vec![0.0_f32; PROBE_BLOCK];
    let mut right = vec![0.0_f32; PROBE_BLOCK];
    let mut peak = 0.0_f32;
    let mut rendered = 0;
    while rendered < frames {
        let count = PROBE_BLOCK.min(frames - rendered);
        synth.render(&mut left[..count], &mut right[..count]);
        for &sample in left[..count].iter().chain(right[..count].iter()) {
            peak = peak.max(sample.abs());
        }
        rendered += count;
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_MAKEUP_GAIN, MIN_MAKEUP_GAIN, REFERENCE_TARGET_PEAK, makeup_for_peak, probe_peak,
    };
    use rustysynth::SoundFont;
    use std::fs::File;
    use std::sync::Arc;

    #[test]
    fn a_hot_font_is_pulled_toward_the_target() {
        assert!((makeup_for_peak(0.9) - REFERENCE_TARGET_PEAK / 0.9).abs() < 1e-6);
        assert_eq!(makeup_for_peak(10.0), MIN_MAKEUP_GAIN);
    }

    #[test]
    fn a_quiet_font_is_lifted_but_capped() {
        assert!((makeup_for_peak(0.05) - REFERENCE_TARGET_PEAK / 0.05).abs() < 1e-6);
        assert_eq!(makeup_for_peak(0.000_1), MAX_MAKEUP_GAIN);
    }

    #[test]
    fn a_silent_or_invalid_peak_is_treated_as_very_quiet() {
        assert_eq!(makeup_for_peak(0.0), MAX_MAKEUP_GAIN);
        assert_eq!(makeup_for_peak(f32::NAN), MAX_MAKEUP_GAIN);
    }

    #[test]
    fn the_bundled_font_measures_a_peak_within_the_gain_bounds() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/microGM.sf2");
        if !path.is_file() {
            return; // The bundled font is not in this checkout.
        }
        let mut file = File::open(&path).expect("open bundled font");
        let font = Arc::new(SoundFont::new(&mut file).expect("parse bundled font"));

        let peak = probe_peak(&font, 44_100);
        assert!(peak.is_finite() && peak > 0.0, "peak was {peak}");
        let makeup = makeup_for_peak(peak);
        assert!(
            (MIN_MAKEUP_GAIN..=MAX_MAKEUP_GAIN).contains(&makeup),
            "makeup was {makeup}"
        );
    }
}
