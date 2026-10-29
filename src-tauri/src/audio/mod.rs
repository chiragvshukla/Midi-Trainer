mod event;
mod output;
mod synth;

pub use event::{AudioCommand, Gate, PlayedNote, ScheduledCommand};
pub use output::{AudioEngine, InputSink};

use std::fmt;

/// Channel reserved for the player's own input, so it can be told apart from the score's parts
/// (the matcher filters on it) and stays on the piano program.
pub const PLAYER_CHANNEL: u8 = 15;

/// GM percussion channel: the drums are played by the app, never expected from the player.
pub const PERCUSSION_CHANNEL: u8 = 9;

/// Channel reserved for feedback cues, kept out of the Music gain so a mistake is heard even
/// with the piece turned down. A file that assigns a track to this channel would take it over.
pub const CUE_CHANNEL: u8 = 14;

/// The wrong-note cue's instrument: a xylophone, short and bright, clearly not the piano.
pub const CUE_PROGRAM: u8 = 13;

/// Pitch of the wrong-note cue, high and clean.
pub const CUE_PITCH: u8 = 72;

/// Velocity for the wrong-note cue: loud enough to stand out over the music.
pub const CUE_VELOCITY: u8 = 110;

/// Tempo range accepted from the outside, as a percentage.
pub const MIN_TEMPO_PERCENT: f32 = 10.0;
pub const MAX_TEMPO_PERCENT: f32 = 400.0;

/// Transpose range, in semitones either way.
pub const TRANSPOSE_SEMITONES: i32 = 12;

/// Errors from the audio subsystem.
#[derive(Debug)]
pub enum AudioError {
    /// The SoundFont file could not be read.
    Io(std::io::Error),
    /// No audio output device is available.
    NoOutputDevice,
    /// The output device offers no 32-bit float format.
    UnsupportedFormat,
    /// The SoundFont file is not a valid SoundFont.
    SoundFont(rustysynth::SoundFontError),
    /// The synthesizer could not be created.
    Synth(rustysynth::SynthesizerError),
    /// The audio device rejected a request.
    Device(String),
    /// The audio thread is no longer running.
    Stopped,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Io(err) => write!(f, "failed to read the SoundFont file: {err}"),
            AudioError::NoOutputDevice => write!(f, "no audio output device is available"),
            AudioError::UnsupportedFormat => {
                write!(f, "the audio output device has no 32-bit float format")
            }
            AudioError::SoundFont(err) => write!(f, "invalid SoundFont file: {err}"),
            AudioError::Synth(err) => write!(f, "could not create the synthesizer: {err}"),
            AudioError::Device(err) => write!(f, "audio device error: {err}"),
            AudioError::Stopped => write!(f, "the audio engine is no longer running"),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AudioError::Io(err) => Some(err),
            AudioError::SoundFont(err) => Some(err),
            AudioError::Synth(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AudioError {
    fn from(err: std::io::Error) -> Self {
        AudioError::Io(err)
    }
}

impl From<rustysynth::SoundFontError> for AudioError {
    fn from(err: rustysynth::SoundFontError) -> Self {
        AudioError::SoundFont(err)
    }
}

impl From<rustysynth::SynthesizerError> for AudioError {
    fn from(err: rustysynth::SynthesizerError) -> Self {
        AudioError::Synth(err)
    }
}

#[cfg(test)]
mod tests {
    use super::AudioError;

    #[test]
    fn describes_missing_device_and_format() {
        assert_eq!(
            AudioError::NoOutputDevice.to_string(),
            "no audio output device is available"
        );
        assert!(AudioError::UnsupportedFormat.to_string().contains("32-bit"));
    }

    #[test]
    fn exposes_source_errors() {
        use std::error::Error;
        let error = AudioError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "no file"));
        assert!(error.source().is_some());
        assert!(AudioError::NoOutputDevice.source().is_none());
    }
}
