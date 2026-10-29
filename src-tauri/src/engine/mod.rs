pub mod matcher;
pub mod midi;
pub mod quantizer;
pub mod scheduler;
pub mod scoring;
pub mod timeline;

use std::fmt;

/// Errors produced by the engine.
#[derive(Debug)]
pub enum EngineError {
    /// The file could not be read from disk.
    Io(std::io::Error),
    /// The data is not a valid Standard MIDI File.
    InvalidMidi(midly::Error),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Io(err) => write!(f, "failed to read MIDI file: {err}"),
            EngineError::InvalidMidi(err) => write!(f, "invalid MIDI data: {err}"),
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EngineError::Io(err) => Some(err),
            EngineError::InvalidMidi(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for EngineError {
    fn from(err: std::io::Error) -> Self {
        EngineError::Io(err)
    }
}

impl From<midly::Error> for EngineError {
    fn from(err: midly::Error) -> Self {
        EngineError::InvalidMidi(err)
    }
}

#[cfg(test)]
mod tests {
    use super::EngineError;
    use std::error::Error;

    #[test]
    fn describes_io_failures() {
        let error = EngineError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "no file"));
        assert_eq!(error.to_string(), "failed to read MIDI file: no file");
        assert!(error.source().is_some());
    }

    #[test]
    fn describes_midi_failures() {
        let error = EngineError::from(midly::Smf::parse(b"not midi").unwrap_err());
        assert!(error.to_string().starts_with("invalid MIDI data: "));
        assert!(error.source().is_some());
    }
}
