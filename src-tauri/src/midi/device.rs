//! Hardware MIDI input devices.
//!
//! Opening a device runs a callback on midir's own thread. That callback only pushes the parsed
//! note into a lock-free [`InputSink`], so it never takes a lock, allocates, or blocks.

use midir::{MidiInput, MidiInputConnection};

use crate::audio::{AudioCommand, InputSink, PLAYER_CHANNEL};

/// The name this client presents to the MIDI subsystem.
const CLIENT_NAME: &str = "MidiTrainer";

/// A port name used when the connection is created.
const PORT_NAME: &str = "midi-trainer-input";

/// Errors from talking to hardware MIDI input.
#[derive(Debug)]
pub enum MidiError {
    /// The MIDI subsystem could not be initialized.
    Init(String),
    /// No connected input device has the requested name.
    NotFound(String),
    /// The device could not be opened.
    Connect(String),
}

impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::Init(err) => write!(f, "MIDI is unavailable: {err}"),
            MidiError::NotFound(name) => write!(f, "no MIDI input device named {name:?}"),
            MidiError::Connect(err) => write!(f, "could not open the MIDI input device: {err}"),
        }
    }
}

impl std::error::Error for MidiError {}

/// An open connection to a hardware MIDI input. Dropping it closes the device.
pub struct MidiInputHandle {
    // The connection owns the input it was built from and closes it on drop.
    _connection: MidiInputConnection<()>,
}

/// The names of the connected MIDI input devices, in system order.
pub fn input_ports() -> Result<Vec<String>, MidiError> {
    let input = MidiInput::new(CLIENT_NAME).map_err(|err| MidiError::Init(err.to_string()))?;
    Ok(input
        .ports()
        .iter()
        .filter_map(|port| input.port_name(port).ok())
        .collect())
}

/// Opens the input device with `name`, forwarding its notes into `sink`.
pub fn connect(name: &str, sink: InputSink) -> Result<MidiInputHandle, MidiError> {
    let input = MidiInput::new(CLIENT_NAME).map_err(|err| MidiError::Init(err.to_string()))?;
    let port = input
        .ports()
        .into_iter()
        .find(|port| {
            input
                .port_name(port)
                .is_ok_and(|port_name| port_name == name)
        })
        .ok_or_else(|| MidiError::NotFound(name.to_string()))?;

    let connection = input
        .connect(
            &port,
            PORT_NAME,
            move |_timestamp, message, _| {
                if let Some(command) = to_command(message) {
                    sink.send(command);
                }
            },
            (),
        )
        .map_err(|err| MidiError::Connect(err.to_string()))?;

    Ok(MidiInputHandle {
        _connection: connection,
    })
}

/// The command a raw MIDI message becomes for the player, remapped onto the player's channel so
/// player input is one stream regardless of the device's channel. Messages that are not notes
/// (clock, controllers, pitch bend) yield `None`.
fn to_command(message: &[u8]) -> Option<AudioCommand> {
    let status = *message.first()?;
    let pitch = *message.get(1)?;
    let velocity = message.get(2).copied().unwrap_or(0);
    match status & 0xF0 {
        // A note-on with velocity zero means note-off by convention.
        0x90 if velocity > 0 => Some(AudioCommand::NoteOn {
            channel: PLAYER_CHANNEL,
            pitch,
            velocity,
        }),
        0x80 | 0x90 => Some(AudioCommand::NoteOff {
            channel: PLAYER_CHANNEL,
            pitch,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::to_command;
    use crate::audio::{AudioCommand, PLAYER_CHANNEL};

    #[test]
    fn maps_note_messages_onto_the_player_channel() {
        assert_eq!(
            to_command(&[0x90, 60, 100]),
            Some(AudioCommand::NoteOn {
                channel: PLAYER_CHANNEL,
                pitch: 60,
                velocity: 100
            })
        );
        assert_eq!(
            to_command(&[0x85, 62, 0]),
            Some(AudioCommand::NoteOff {
                channel: PLAYER_CHANNEL,
                pitch: 62
            })
        );
    }

    #[test]
    fn treats_a_zero_velocity_note_on_as_note_off() {
        assert_eq!(
            to_command(&[0x90, 60, 0]),
            Some(AudioCommand::NoteOff {
                channel: PLAYER_CHANNEL,
                pitch: 60
            })
        );
    }

    #[test]
    fn ignores_messages_that_are_not_notes() {
        assert_eq!(to_command(&[0xB0, 7, 80]), None);
        assert_eq!(to_command(&[0xE0, 0, 64]), None);
        assert_eq!(to_command(&[0xF8]), None);
        assert_eq!(to_command(&[]), None);
    }
}
