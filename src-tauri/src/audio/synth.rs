use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use super::AudioError;
use super::event::AudioCommand;

/// A SoundFont-backed synthesizer. It is owned by the audio callback and never leaves it.
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

    /// Applies a command. Runs inside the audio callback, so it must not block or allocate.
    pub fn apply(&mut self, command: AudioCommand) {
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

    /// Renders one block into `left` and `right`, which must be the same length.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.synthesizer.render(left, right);
    }
}
