use std::sync::Arc;

/// A command sent from a control thread to the audio callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCommand {
    NoteOn {
        channel: u8,
        pitch: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        pitch: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    /// Stops every sounding note, for example when playback stops.
    AllNotesOff,
}

/// A command to apply at a score time, in milliseconds from the start of the piece.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduledCommand {
    pub at_ms: f64,
    pub command: AudioCommand,
}

/// A message that changes what the score playback is doing.
#[derive(Debug)]
pub enum PlaybackControl {
    /// Replaces the piece being played and starts it from the beginning.
    SetScore(Arc<Vec<ScheduledCommand>>),
    /// Tempo as a percentage of the original; 100 is normal speed.
    SetTempoPercent(f32),
    /// Semitones shifted up (positive) or down (negative).
    SetTranspose(i32),
    Start,
    Stop,
}

/// A raw MIDI message: channel, status byte, and the two data bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    pub channel: i32,
    pub status: i32,
    pub data1: i32,
    pub data2: i32,
}

impl AudioCommand {
    /// The MIDI message this command becomes, or `None` when it has no single message.
    pub fn message(self) -> Option<MidiMessage> {
        let message = match self {
            AudioCommand::NoteOn {
                channel,
                pitch,
                velocity,
            } => MidiMessage {
                channel: i32::from(channel),
                status: 0x90,
                data1: i32::from(pitch),
                data2: i32::from(velocity),
            },
            AudioCommand::NoteOff { channel, pitch } => MidiMessage {
                channel: i32::from(channel),
                status: 0x80,
                data1: i32::from(pitch),
                data2: 0,
            },
            AudioCommand::ProgramChange { channel, program } => MidiMessage {
                channel: i32::from(channel),
                status: 0xC0,
                data1: i32::from(program),
                data2: 0,
            },
            AudioCommand::ControlChange {
                channel,
                controller,
                value,
            } => MidiMessage {
                channel: i32::from(channel),
                status: 0xB0,
                data1: i32::from(controller),
                data2: i32::from(value),
            },
            AudioCommand::AllNotesOff => return None,
        };
        Some(message)
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioCommand, MidiMessage};

    #[test]
    fn maps_commands_to_midi_messages() {
        assert_eq!(
            AudioCommand::NoteOn {
                channel: 1,
                pitch: 60,
                velocity: 100
            }
            .message(),
            Some(MidiMessage {
                channel: 1,
                status: 0x90,
                data1: 60,
                data2: 100
            })
        );
        assert_eq!(
            AudioCommand::NoteOff {
                channel: 2,
                pitch: 62
            }
            .message(),
            Some(MidiMessage {
                channel: 2,
                status: 0x80,
                data1: 62,
                data2: 0
            })
        );
        assert_eq!(
            AudioCommand::ProgramChange {
                channel: 0,
                program: 40
            }
            .message(),
            Some(MidiMessage {
                channel: 0,
                status: 0xC0,
                data1: 40,
                data2: 0
            })
        );
        assert_eq!(
            AudioCommand::ControlChange {
                channel: 0,
                controller: 7,
                value: 80
            }
            .message(),
            Some(MidiMessage {
                channel: 0,
                status: 0xB0,
                data1: 7,
                data2: 80
            })
        );
    }

    #[test]
    fn all_notes_off_has_no_single_message() {
        assert_eq!(AudioCommand::AllNotesOff.message(), None);
    }
}
