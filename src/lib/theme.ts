import { create } from "zustand";

/** The chosen base brightness; `system` follows the operating system. */
export type Theme = "dark" | "light" | "system";

/**
 * The fixed colour palettes offered, each a well-known publishing with dark and light variants,
 * with its display label. A palette owns every chrome and note colour, so the whole app stays
 * cohesive instead of mixing a hue into a background that is already tinted. This is the single
 * source of truth: the `Palette` union and the stored-value check both derive from it.
 */
export const PALETTES = [
  { id: "slate", label: "Slate (neutral)" },
  { id: "catppuccin", label: "Catppuccin" },
  { id: "nord", label: "Nord" },
  { id: "solarized", label: "Solarized" },
  { id: "gruvbox", label: "Gruvbox" },
  { id: "tokyo", label: "Tokyo Night" },
  { id: "dracula", label: "Dracula" },
  { id: "one-dark", label: "One Dark" },
  { id: "rose-pine", label: "Rosé Pine" },
  { id: "everforest", label: "Everforest" },
] as const;

export type Palette = (typeof PALETTES)[number]["id"];

/** Where note names are drawn: on the keys, on the falling notes, both, or nowhere. */
export type NoteLabels = "off" | "keys" | "notes" | "both";

/** Where the chosen appearance is remembered on this machine. */
const STORAGE_KEY = "miditrainer.appearance";

/** The old theme-only key, read once so an existing dark/light choice carries over. */
const LEGACY_KEY = "miditrainer.theme";

/** How many tracks can get a manual colour override (matches the --roll-track-N tokens). */
export const TRACK_COLOR_COUNT = 6;

/** The appearance the user has chosen; everything here is UI-only. */
interface Appearance {
  theme: Theme;
  palette: Palette;
  /** Per-track overrides, index 0..5; an empty string means "use the palette's colour". */
  trackColors: string[];
  /** Whether the left navigation rail is collapsed to icons. */
  collapsed: boolean;
  /** Disable transitions and animations. */
  reducedMotion: boolean;
  /** Use the colour-blind-safe note palette instead of per-theme track colours. */
  safeColors: boolean;
  /** Where note names are drawn. */
  noteLabels: NoteLabels;
  /** Whether the falling-note view draws the lane and bar-line backdrop. */
  noteBackdrop: boolean;
}

const DEFAULTS: Appearance = {
  theme: "dark",
  palette: "gruvbox",
  trackColors: Array<string>(TRACK_COLOR_COUNT).fill(""),
  collapsed: false,
  reducedMotion: false,
  safeColors: false,
  noteLabels: "off",
  noteBackdrop: true,
};

function isTheme(value: unknown): value is Theme {
  return value === "dark" || value === "light" || value === "system";
}

function isPalette(value: unknown): value is Palette {
  return PALETTES.some((palette) => palette.id === value);
}

function isNoteLabels(value: unknown): value is NoteLabels {
  return (
    value === "off" || value === "keys" || value === "notes" || value === "both"
  );
}

/** The dark/light value a `system` choice resolves to right now. */
export function resolvedTheme(theme: Theme): "dark" | "light" {
  if (theme !== "system") {
    return theme;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function read(): Appearance {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const saved = JSON.parse(raw) as Partial<Appearance>;
      return {
        theme: isTheme(saved.theme) ? saved.theme : DEFAULTS.theme,
        palette: isPalette(saved.palette) ? saved.palette : DEFAULTS.palette,
        trackColors: Array.from({ length: TRACK_COLOR_COUNT }, (_, index) =>
          typeof saved.trackColors?.[index] === "string"
            ? saved.trackColors[index]
            : "",
        ),
        collapsed: saved.collapsed === true,
        reducedMotion: saved.reducedMotion === true,
        safeColors: saved.safeColors === true,
        noteLabels: isNoteLabels(saved.noteLabels) ? saved.noteLabels : "off",
        noteBackdrop: saved.noteBackdrop !== false,
      };
    }
    return {
      ...DEFAULTS,
      theme: localStorage.getItem(LEGACY_KEY) === "light" ? "light" : "dark",
    };
  } catch {
    return { ...DEFAULTS };
  }
}

/** Puts the appearance on the document, where the CSS tokens key off it, and remembers it. */
function apply(state: Appearance): void {
  const root = document.documentElement;
  root.dataset.theme = resolvedTheme(state.theme);
  root.dataset.palette = state.palette;
  if (state.reducedMotion) {
    root.dataset.motion = "reduced";
  } else {
    delete root.dataset.motion;
  }
  if (state.safeColors) {
    root.dataset.cvd = "on";
  } else {
    delete root.dataset.cvd;
  }
  // A manual per-track colour wins over the theme, but the colour-blind-safe palette takes
  // over entirely while it is on, so the overrides are held back rather than cleared.
  state.trackColors.forEach((color, index) => {
    const name = `--roll-track-${index + 1}`;
    if (color && !state.safeColors) {
      root.style.setProperty(name, color);
    } else {
      root.style.removeProperty(name);
    }
  });
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // A machine that forbids storage just forgets the choice; the palette still applies.
  }
}

interface AppearanceState extends Appearance {
  /** The brightness actually in effect, after resolving `system`. */
  resolved: "dark" | "light";
  /** Bumped when the theme or a canvas colour changes, so the canvases redraw. */
  version: number;
  setTheme: (theme: Theme) => void;
  setPalette: (palette: Palette) => void;
  setTrackColor: (index: number, color: string) => void;
  resetTrackColors: () => void;
  setReducedMotion: (on: boolean) => void;
  setSafeColors: (on: boolean) => void;
  setNoteLabels: (labels: NoteLabels) => void;
  setNoteBackdrop: (on: boolean) => void;
  toggleSidebar: () => void;
}

/** Applies a change and returns the next state, bumping the redraw version. */
function advance(
  state: AppearanceState,
  patch: Partial<Appearance>,
): AppearanceState {
  const next = { ...state, ...patch };
  apply(next);
  return {
    ...next,
    resolved: resolvedTheme(next.theme),
    version: state.version + 1,
  };
}

const initial = read();

export const useAppearance = create<AppearanceState>((set) => ({
  ...initial,
  resolved: resolvedTheme(initial.theme),
  version: 0,
  setTheme: (theme) => set((state) => advance(state, { theme })),
  setPalette: (palette) => set((state) => advance(state, { palette })),
  setTrackColor: (index, color) =>
    set((state) => {
      const trackColors = [...state.trackColors];
      trackColors[index] = color;
      return advance(state, { trackColors });
    }),
  resetTrackColors: () =>
    set((state) =>
      advance(state, {
        trackColors: Array<string>(TRACK_COLOR_COUNT).fill(""),
      }),
    ),
  toggleSidebar: () =>
    set((state) => advance(state, { collapsed: !state.collapsed })),
  setReducedMotion: (on) =>
    set((state) => advance(state, { reducedMotion: on })),
  setSafeColors: (on) => set((state) => advance(state, { safeColors: on })),
  setNoteLabels: (labels) =>
    set((state) => advance(state, { noteLabels: labels })),
  setNoteBackdrop: (on) => set((state) => advance(state, { noteBackdrop: on })),
}));

/** Applies the remembered appearance and follows the OS while the theme is `system`. */
export function initAppearance(): void {
  apply(useAppearance.getState());
  window
    .matchMedia("(prefers-color-scheme: dark)")
    .addEventListener("change", () => {
      if (useAppearance.getState().theme === "system") {
        apply(useAppearance.getState());
        useAppearance.setState((state) => ({
          resolved: resolvedTheme(state.theme),
          version: state.version + 1,
        }));
      }
    });
}

/** The falling-note canvas colours for the current theme, read from the CSS tokens. */
export function canvasColors() {
  const style = getComputedStyle(document.documentElement);
  const read = (name: string) => style.getPropertyValue(name).trim();
  return {
    bg: read("--roll-bg"),
    lane: read("--roll-lane"),
    grid: read("--roll-grid"),
    label: read("--roll-label"),
    left: read("--roll-left"),
    right: read("--roll-right"),
    correct: read("--roll-correct"),
    wrong: read("--roll-wrong"),
    soundingKey: read("--roll-sounding-key"),
    pressedWhite: read("--roll-pressed-white"),
    pressedBlack: read("--roll-pressed-black"),
    white: read("--roll-white-key"),
    black: read("--roll-black-key"),
    keyBorder: read("--roll-key-border"),
    playhead: read("--roll-playhead"),
    track1: read("--roll-track-1"),
    track2: read("--roll-track-2"),
    track3: read("--roll-track-3"),
    track4: read("--roll-track-4"),
    track5: read("--roll-track-5"),
    track6: read("--roll-track-6"),
  };
}
