//! The PC-keyboard fallback layout, used when no hardware MIDI keyboard is connected.
//!
//! Two rows, with `A` as middle C (C4): the `a` row plays the white keys and the `q` row the
//! black keys above it. Browser `KeyboardEvent.code` values are used so the layout follows the
//! physical keys rather than the user's keyboard language.

/// Velocity for a key press: a fixed, comfortably-loud value; the PC keyboard has no dynamics.
pub const KEY_VELOCITY: u8 = 100;

/// MIDI pitch for a browser `KeyboardEvent.code`, or `None` if that key plays no note.
pub fn pitch_for(code: &str) -> Option<u8> {
    let pitch = match code {
        // White keys, middle C upward.
        "KeyA" => 60,
        "KeyS" => 62,
        "KeyD" => 64,
        "KeyF" => 65,
        "KeyG" => 67,
        "KeyH" => 69,
        "KeyJ" => 71,
        "KeyK" => 72,
        "KeyL" => 74,
        "Semicolon" => 76,
        // Black keys, on the row above.
        "KeyW" => 61,
        "KeyE" => 63,
        "KeyT" => 66,
        "KeyY" => 68,
        "KeyU" => 70,
        "KeyO" => 73,
        "KeyP" => 75,
        _ => return None,
    };
    Some(pitch)
}

#[cfg(test)]
mod tests {
    use super::pitch_for;

    #[test]
    fn maps_the_a_row_to_white_keys_from_middle_c() {
        let keys = [
            ("KeyA", 60),
            ("KeyS", 62),
            ("KeyD", 64),
            ("KeyF", 65),
            ("KeyG", 67),
            ("KeyH", 69),
            ("KeyJ", 71),
            ("KeyK", 72),
            ("KeyL", 74),
            ("Semicolon", 76),
        ];
        for (code, pitch) in keys {
            assert_eq!(pitch_for(code), Some(pitch), "{code}");
        }
    }

    #[test]
    fn maps_the_q_row_to_black_keys() {
        let keys = [
            ("KeyW", 61),
            ("KeyE", 63),
            ("KeyT", 66),
            ("KeyY", 68),
            ("KeyU", 70),
            ("KeyO", 73),
            ("KeyP", 75),
        ];
        for (code, pitch) in keys {
            assert_eq!(pitch_for(code), Some(pitch), "{code}");
        }
    }

    #[test]
    fn ignores_keys_that_play_no_note() {
        assert_eq!(pitch_for("KeyZ"), None);
        assert_eq!(pitch_for("Space"), None);
    }
}
