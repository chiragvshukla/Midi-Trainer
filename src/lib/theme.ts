import { create } from "zustand";

/** The chosen base brightness; `system` follows the operating system. */
export type Theme = "dark" | "light" | "system";

/** A named chrome treatment laid over the theme. */
export type Look = "matte" | "atomic";

/** The accent hue for lit controls. */
export type Accent = "orange" | "teal" | "magenta" | "green";

/** Where the chosen appearance is remembered on this machine. */
const STORAGE_KEY = "miditrainer.appearance";

/** The old theme-only key, read once so an existing dark/light choice carries over. */
const LEGACY_KEY = "miditrainer.theme";

/** How many tracks can get a manual colour override (matches the --roll-track-N tokens). */
export const TRACK_COLOR_COUNT = 6;

/** The appearance the user has chosen; everything here is UI-only. */
interface Appearance {
  theme: Theme;
  look: Look;
  accent: Accent;
  /** Per-track overrides, index 0..5; an empty string means "use the theme's colour". */
  trackColors: string[];
}

const DEFAULTS: Appearance = {
  theme: "dark",
  look: "matte",
  accent: "orange",
  trackColors: Array<string>(TRACK_COLOR_COUNT).fill(""),
};

function isTheme(value: unknown): value is Theme {
  return value === "dark" || value === "light" || value === "system";
}

function isAccent(value: unknown): value is Accent {
  return (
    value === "orange" ||
    value === "teal" ||
    value === "magenta" ||
    value === "green"
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
        look: saved.look === "atomic" ? "atomic" : "matte",
        accent: isAccent(saved.accent) ? saved.accent : DEFAULTS.accent,
        trackColors: Array.from({ length: TRACK_COLOR_COUNT }, (_, index) =>
          typeof saved.trackColors?.[index] === "string"
            ? saved.trackColors[index]
            : "",
        ),
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
  root.dataset.look = state.look;
  root.dataset.accent = state.accent;
  state.trackColors.forEach((color, index) => {
    const name = `--roll-track-${index + 1}`;
    if (color) {
      root.style.setProperty(name, color);
    } else {
      root.style.removeProperty(name);
    }
  });
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // A machine that forbids storage just forgets the choice; the look still applies.
  }
}

interface AppearanceState extends Appearance {
  /** The brightness actually in effect, after resolving `system`. */
  resolved: "dark" | "light";
  /** Bumped when the theme or a canvas colour changes, so the canvases redraw. */
  version: number;
  setTheme: (theme: Theme) => void;
  setLook: (look: Look) => void;
  setAccent: (accent: Accent) => void;
  setTrackColor: (index: number, color: string) => void;
  resetTrackColors: () => void;
  toggleTheme: () => void;
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
  setLook: (look) => set((state) => advance(state, { look })),
  setAccent: (accent) => set((state) => advance(state, { accent })),
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
  toggleTheme: () =>
    set((state) =>
      advance(state, {
        theme: state.resolved === "dark" ? "light" : "dark",
      }),
    ),
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
    note: read("--roll-note"),
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

/** The sheet-music colours for the current theme, read from the CSS tokens. */
export function scoreColors() {
  const style = getComputedStyle(document.documentElement);
  const read = (name: string) => style.getPropertyValue(name).trim();
  return {
    correct: read("--roll-correct"),
    wrong: read("--roll-wrong"),
    cursor: read("--roll-playhead"),
  };
}
