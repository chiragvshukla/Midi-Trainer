import { useEffect, useState } from "react";
import { pcKeyboardLayout, type PcKey } from "../lib/api";
import { log } from "../lib/logger";
import { pitchName } from "../lib/pitch";

/** The transport shortcuts, in the order they are shown. */
const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: "Space", action: "Play or stop" },
  { keys: "Home", action: "Back to the start" },
  { keys: "←", action: "Previous bar" },
  { keys: "→", action: "Next bar" },
];

/** A browser key code as the keycap a user would read. */
function keyLabel(code: string): string {
  if (code.startsWith("Key")) {
    return code.slice(3);
  }
  return code === "Semicolon" ? ";" : code;
}

/**
 * The keyboard-shortcut help: the transport keys, plus the PC-keyboard note layout fetched from
 * the backend so the mapping has one source of truth. Opens as a drawer over the app.
 */
export function ShortcutsHelp({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [layout, setLayout] = useState<PcKey[]>([]);

  useEffect(() => {
    if (!open) {
      return;
    }
    void pcKeyboardLayout()
      .then(setLayout)
      .catch((error) =>
        log.error(`loading the PC keyboard layout failed: ${String(error)}`),
      );
  }, [open]);

  if (!open) {
    return null;
  }

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <aside className="drawer" role="dialog" aria-label="Keyboard shortcuts">
        <header className="drawer-head">
          <h2>Keyboard shortcuts</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Close shortcuts"
            onClick={onClose}
          >
            ✕
          </button>
        </header>
        <div className="drawer-body">
          <section className="settings-group">
            <h3>Transport</h3>
            <dl className="shortcut-list">
              {SHORTCUTS.map((shortcut) => (
                <div className="shortcut-row" key={shortcut.keys}>
                  <dt>{shortcut.keys}</dt>
                  <dd>{shortcut.action}</dd>
                </div>
              ))}
            </dl>
          </section>

          <section className="settings-group">
            <h3>Play notes on the PC keyboard</h3>
            {layout.length === 0 ? (
              <p className="field-hint">No layout available.</p>
            ) : (
              <ul className="key-layout">
                {layout.map((key) => (
                  <li key={key.code}>
                    <kbd>{keyLabel(key.code)}</kbd>
                    <span>{pitchName(key.pitch)}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      </aside>
    </>
  );
}
