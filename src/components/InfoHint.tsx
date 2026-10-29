/**
 * A small (i) button that explains a control in a tooltip. It is a button, not a bare span, so
 * it is reachable by keyboard; it sits beside its control rather than inside its label, so
 * reading the hint never toggles the control.
 */
export function InfoHint({ text }: { text: string }) {
  return (
    <button
      type="button"
      className="info-button"
      title={text}
      aria-label={text}
    >
      i
    </button>
  );
}
