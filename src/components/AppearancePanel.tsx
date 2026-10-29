import { useAppearance, canvasColors } from "../lib/theme";
import type { NoteLabels, Palette, Theme } from "../lib/theme";

/** The fixed colour palettes offered, in the order shown. */
const PALETTES: { id: Palette; label: string }[] = [
  { id: "slate", label: "Slate (neutral)" },
  { id: "catppuccin", label: "Catppuccin" },
  { id: "nord", label: "Nord" },
  { id: "solarized", label: "Solarized" },
  { id: "gruvbox", label: "Gruvbox" },
  { id: "tokyo", label: "Tokyo Night" },
];

/**
 * The global (app-wide) settings drawer: appearance and accessibility. These are preferences
 * that hold regardless of the piece being practised; anything about playing lives in the
 * player settings. Rendering only — every change goes through the appearance store.
 */
export function AppearancePanel({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const theme = useAppearance((state) => state.theme);
  const palette = useAppearance((state) => state.palette);
  const trackColors = useAppearance((state) => state.trackColors);
  const setTheme = useAppearance((state) => state.setTheme);
  const setPalette = useAppearance((state) => state.setPalette);
  const setTrackColor = useAppearance((state) => state.setTrackColor);
  const resetTrackColors = useAppearance((state) => state.resetTrackColors);
  const reducedMotion = useAppearance((state) => state.reducedMotion);
  const safeColors = useAppearance((state) => state.safeColors);
  const setReducedMotion = useAppearance((state) => state.setReducedMotion);
  const setSafeColors = useAppearance((state) => state.setSafeColors);
  const noteLabels = useAppearance((state) => state.noteLabels);
  const setNoteLabels = useAppearance((state) => state.setNoteLabels);

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
      <aside className="drawer" role="dialog" aria-label="App settings">
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
              <span>Theme</span>
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
        </div>
      </aside>
    </>
  );
}
