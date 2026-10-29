use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use super::AudioError;
use super::event::AudioCommand;

/// Something that turns commands into sound. Implemented by the SoundFont synthesizer and
/// by test doubles, so the render loop can be checked without an audio device.
pub trait Synth: Send {
    /// Applies a command. Runs on the audio thread, so it must not block or allocate.
    fn apply(&mut self, command: AudioCommand);

    /// Renders one block into `left` and `right`, which must be the same length.
    fn render(&mut self, left: &mut [f32], right: &mut [f32]);
}

/// A SoundFont-backed synthesizer. It is owned by the audio thread and never leaves it.
pub struct SoundFontSynth {
    synthesizer: Synthesizer,
}

impl SoundFontSynth {
    /// Loads a `.sf2` file and prepares synthesis at the output sample rate.
    pub fn load(path: &Path, sample_rate: u32) -> Result<Self, AudioError> {
        let mut file = File::open(path)?;
        let sound_font = Arc::new(SoundFont::new(&mut file)?);
        let settings = SynthesizerSettings::new(sample_rate as i32);
        let synthesizer = Synthesizer::new(&sound_font, &settings)?;
        Ok(SoundFontSynth { synthesizer })
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
}
