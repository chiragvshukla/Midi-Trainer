/** Names by pitch class, for the note-name labels and the shortcut help. */
const PITCH_NAMES = [
  "C",
  "C#",
  "D",
  "D#",
  "E",
  "F",
  "F#",
  "G",
  "G#",
  "A",
  "A#",
  "B",
];

/** A MIDI pitch as a name and octave, e.g. 60 -> C4. */
export function pitchName(pitch: number): string {
  return `${PITCH_NAMES[pitch % 12]}${Math.floor(pitch / 12) - 1}`;
}
