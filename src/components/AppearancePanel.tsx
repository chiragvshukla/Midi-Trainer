import { useAppearance, canvasColors, PALETTES } from "../lib/theme";
import type { Look, NoteLabels, Palette, PianoLook, Theme } from "../lib/theme";
import { useAppStore } from "../state/appStore";

/**
 * The global (app-wide) settings: appearance and accessibility. These are preferences that hold
 * regardless of the piece being practised; anything about playing lives in the player settings.
 * Opens as a small centred window. Rendering only — every change goes through the appearance
 * store.
 */
export function AppearancePanel({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const theme = useAppearance((state) => state.theme);
  const look = useAppearance((state) => state.look);
  const palette = useAppearance((state) => state.palette);
  const pianoLook = useAppearance((state) => state.pianoLook);
  const trackColors = useAppearance((state) => state.trackColors);
  const setTheme = useAppearance((state) => state.setTheme);
  const setLook = useAppearance((state) => state.setLook);
  const setPalette = useAppearance((state) => state.setPalette);
  const setPianoLook = useAppearance((state) => state.setPianoLook);
  const setTrackColor = useAppearance((state) => state.setTrackColor);
  const resetTrackColors = useAppearance((state) => state.resetTrackColors);
  const reducedMotion = useAppearance((state) => state.reducedMotion);
  const safeColors = useAppearance((state) => state.safeColors);
  const setReducedMotion = useAppearance((state) => state.setReducedMotion);
  const setSafeColors = useAppearance((state) => state.setSafeColors);
  const noteLabels = useAppearance((state) => state.noteLabels);
  const setNoteLabels = useAppearance((state) => state.setNoteLabels);
  const noteBackdrop = useAppearance((state) => state.noteBackdrop);
  const setNoteBackdrop = useAppearance((state) => state.setNoteBackdrop);
  const version = useAppStore((state) => state.appInfo?.version ?? null);

  if (!open) {
    return null;
  }

  // The colour swatches fall back to the palette's track tokens when not overridden.
  const tokens = canvasColors();
  const trackFallbacks = [
    tokens.track1,
    tokens.track2,
    tokens.track3,
    tokens.track4,
    tokens.track5,
    tokens.track6,
  ];

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="App settings"
      >
        <header className="drawer-head">
          <h2>App settings</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Close settings"
            onClick={onClose}
          >
            ✕
          </button>
        </header>
        <div className="drawer-body">
          <section className="settings-group">
            <h3>Appearance</h3>
            <label className="field">
              <span>Appearance</span>
              <select
                value={theme}
                onChange={(event) => setTheme(event.target.value as Theme)}
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">Follow system</option>
              </select>
            </label>
            <label className="field">
              <span>Theme</span>
              <select
                value={look}
                onChange={(event) => setLook(event.target.value as Look)}
              >
                <option value="matte">Minimal</option>
                <option value="win95">Win95</option>
                <option value="atomic">Atomic</option>
              </select>
            </label>
            <label className="field">
              <span>Palette</span>
              <select
                value={palette}
                onChange={(event) => setPalette(event.target.value as Palette)}
              >
                {PALETTES.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Piano look</span>
              <select
                value={pianoLook}
                onChange={(event) =>
                  setPianoLook(event.target.value as PianoLook)
                }
              >
                <option value="minimalistic">Minimalistic</option>
                <option value="realistic">Realistic</option>
              </select>
            </label>
            <div className="field">
              <span>Note names</span>
              <select
                value={noteLabels}
                onChange={(event) =>
                  setNoteLabels(event.target.value as NoteLabels)
                }
              >
                <option value="off">None</option>
                <option value="keys">On the keys</option>
                <option value="notes">On the notes</option>
                <option value="both">On both</option>
              </select>
            </div>
            <label className="check">
              <input
                type="checkbox"
                checked={noteBackdrop}
                onChange={(event) => setNoteBackdrop(event.target.checked)}
              />
              Falling-note backdrop
            </label>
            <div className="field">
              <span>Note colours per track</span>
              <div className="color-grid">
                {trackColors.map((color, index) => (
                  <label className="color-field" key={index}>
                    {index + 1}
                    <input
                      type="color"
                      value={color || trackFallbacks[index]}
                      disabled={safeColors}
                      onChange={(event) =>
                        setTrackColor(index, event.target.value)
                      }
                    />
                  </label>
                ))}
              </div>
              <button
                type="button"
                className="btn"
                disabled={safeColors}
                onClick={resetTrackColors}
              >
                Reset colours
              </button>
            </div>
          </section>

          <section className="settings-group">
            <h3>Accessibility</h3>
            <label className="check">
              <input
                type="checkbox"
                checked={reducedMotion}
                onChange={(event) => setReducedMotion(event.target.checked)}
              />
              Reduce motion
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={safeColors}
                onChange={(event) => setSafeColors(event.target.checked)}
              />
              Colour-blind-safe note colours
            </label>
          </section>

          {version ? (
            <p className="app-version">MidiTrainer v{version}</p>
          ) : null}
        </div>
      </div>
    </>
  );
}
