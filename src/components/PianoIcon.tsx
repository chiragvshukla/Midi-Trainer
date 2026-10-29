/** A small piano-keyboard glyph for the Practice nav entry, sized by the surrounding icon slot. */
export function PianoIcon() {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.1"
      aria-hidden="true"
    >
      <rect x="1.5" y="3.5" width="13" height="9" rx="1.5" />
      <path d="M4.75 3.5V12.5M8 3.5V12.5M11.25 3.5V12.5" />
      <g fill="currentColor" stroke="none">
        <rect x="3.95" y="4" width="1.6" height="3" />
        <rect x="7.2" y="4" width="1.6" height="3" />
        <rect x="10.45" y="4" width="1.6" height="3" />
      </g>
    </svg>
  );
}
