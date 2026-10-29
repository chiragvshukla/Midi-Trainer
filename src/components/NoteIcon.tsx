/** A small music-note glyph marking a MIDI file. */
export function NoteIcon() {
  return (
    <svg className="note-icon" viewBox="0 0 16 16" aria-hidden="true">
      <rect x="7.2" y="2.4" width="1.4" height="8.8" fill="currentColor" />
      <ellipse
        cx="5.3"
        cy="11.4"
        rx="3.2"
        ry="2.3"
        fill="currentColor"
        transform="rotate(-20 5.3 11.4)"
      />
    </svg>
  );
}
