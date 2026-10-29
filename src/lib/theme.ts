import { create } from "zustand";

export type Theme = "dark" | "light";

/** Where the chosen theme is remembered on this machine. */
const STORAGE_KEY = "miditrainer.theme";

function readStored(): Theme {
  try {
    return localStorage.getItem(STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

/** Puts the theme on the document, where the CSS tokens key off it, and remembers it. */
function apply(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // A machine that forbids storage just forgets the choice; the theme still applies.
  }
}

interface ThemeState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
}

export const useTheme = create<ThemeState>((set, get) => ({
  theme: readStored(),
  setTheme: (theme) => {
    apply(theme);
    set({ theme });
  },
  toggleTheme: () => get().setTheme(get().theme === "dark" ? "light" : "dark"),
}));

/** Applies the remembered theme; call once at startup, before the first paint. */
export function initTheme(): void {
  apply(useTheme.getState().theme);
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
