import { create } from "zustand";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cancelSoundfontDownload,
  connectMidiInput,
  disconnectMidiInput,
  downloadSoundfont,
  fetchSettings,
  fetchSoundfontStatus,
  listLibrary,
  listMidiInputs,
  loadSoundfont,
  play,
  playFile,
  saveSettings,
  setFollow,
  setLibraryFolder,
  setPlayerHand,
  setTempoPercent,
  setTranspose,
  setVolume,
  stopPlayback,
  type Hand,
  type Judgement,
  type LibraryEntry,
  type Piece,
  type Settings,
  type SoundfontStatus,
  type Verdict,
} from "../lib/api";
import { log } from "../lib/logger";

export interface DownloadState {
  received: number;
  total: number;
}

/** How long slider changes wait before the settings file is rewritten. */
const SAVE_DEBOUNCE_MS = 500;

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Persists the settings, but coalesces a burst of slider changes into one write. */
function scheduleSave(): void {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void saveSettings().catch((error) =>
      log.error(`saving settings failed: ${String(error)}`),
    );
  }, SAVE_DEBOUNCE_MS);
}

/** Key of one falling note in the verdict map: pitch and score time. */
export function verdictKey(pitch: number, startMs: number): string {
  return `${pitch}:${Math.round(startMs)}`;
}

interface SessionState {
  soundfont: SoundfontStatus | null;
  download: DownloadState | null;
  soundfontError: string | null;
  settings: Settings | null;
  /** The connected MIDI input devices, as the system names them. */
  devices: string[];
  /** The folder the user browses as their library, if one was chosen. */
  libraryFolder: string | null;
  /** The MIDI files found under the library folder. */
  library: LibraryEntry[];
  piece: Piece | null;
  /** Path of the loaded piece, so the library can highlight it. */
  piecePath: string | null;
  follow: boolean;
  /** Verdict per falling note, keyed by pitch and score time. */
  verdicts: Record<string, Verdict>;
  /** Pitches currently held down on any keyboard. */
  pressed: number[];
  /** First-run setup: SoundFont first (the engine needs it), then the saved settings. */
  startup: () => Promise<void>;
  initSoundfont: () => Promise<void>;
  loadSettings: () => Promise<Settings | null>;
  cancelDownload: () => Promise<void>;
  updateDownload: (progress: DownloadState | null) => void;
  loadDevices: () => Promise<void>;
  connectDevice: (name: string | null) => Promise<void>;
  loadLibrary: () => Promise<void>;
  chooseLibraryFolder: () => Promise<void>;
  openPiece: (path: string) => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  toggleFollow: (follow: boolean) => Promise<void>;
  changeHand: (hand: Hand) => Promise<void>;
  changeTempo: (percent: number) => Promise<void>;
  changeTranspose: (semitones: number) => Promise<void>;
  changeVolume: (volume: number) => Promise<void>;
  recordJudgements: (judgements: Judgement[]) => void;
  pressPitch: (pitch: number, down: boolean) => void;
}

export const useSession = create<SessionState>((set, get) => ({
  soundfont: null,
  download: null,
  soundfontError: null,
  settings: null,
  devices: [],
  libraryFolder: null,
  library: [],
  piece: null,
  piecePath: null,
  follow: false,
  verdicts: {},
  pressed: [],

  startup: async () => {
    await get().initSoundfont();
    if (!get().soundfontError) {
      await get().loadSettings();
      await get().loadLibrary();
    }
  },

  initSoundfont: async () => {
    set({ soundfontError: null, download: null });
    try {
      const soundfont = await fetchSoundfontStatus();
      set({ soundfont });
      if (soundfont.installed) {
        try {
          await loadSoundfont(soundfont.path);
          return;
        } catch (error) {
          // The installed file is broken; downloading replaces it.
          log.warn(`installed SoundFont failed to load: ${String(error)}`);
        }
      }
      // First run: fetch the baseline font behind the visible progress screen.
      const path = await downloadSoundfont();
      await loadSoundfont(path);
      set({ soundfont: { installed: true, path } });
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`soundfont setup failed: ${String(error)}`);
    } finally {
      set({ download: null });
    }
  },

  cancelDownload: async () => {
    try {
      await cancelSoundfontDownload();
    } catch (error) {
      log.error(`cancelling the download failed: ${String(error)}`);
    }
  },

  updateDownload: (progress) => set({ download: progress }),

  loadSettings: async () => {
    try {
      const settings = await fetchSettings();
      set({ settings });
      set({ libraryFolder: settings.library_folder ?? null });
      // The engine starts fresh every run; put the saved practice setup back.
      await setVolume(settings.volume);
      await setTempoPercent(settings.tempo_percent);
      await setTranspose(settings.transpose);
      if (settings.midi_device) {
        await get().connectDevice(settings.midi_device);
      }
      return settings;
    } catch (error) {
      log.error(`failed to load settings: ${String(error)}`);
      return null;
    }
  },

  loadDevices: async () => {
    try {
      set({ devices: await listMidiInputs() });
    } catch (error) {
      log.error(`listing MIDI inputs failed: ${String(error)}`);
    }
  },

  connectDevice: async (name) => {
    try {
      if (name) {
        await connectMidiInput(name);
      } else {
        await disconnectMidiInput();
      }
      set((state) => ({
        settings: state.settings && { ...state.settings, midi_device: name },
      }));
    } catch (error) {
      log.error(`connecting MIDI input failed: ${String(error)}`);
    }
  },

  loadLibrary: async () => {
    const folder = get().libraryFolder;
    if (!folder) {
      set({ library: [] });
      return;
    }
    try {
      set({ library: await listLibrary(folder) });
    } catch (error) {
      log.error(`listing the library folder failed: ${String(error)}`);
      set({ library: [] });
    }
  },

  chooseLibraryFolder: async () => {
    try {
      const folder = await open({ directory: true });
      if (typeof folder !== "string") {
        return;
      }
      await setLibraryFolder(folder);
      set({ libraryFolder: folder });
      await get().loadLibrary();
    } catch (error) {
      log.error(`choosing a library folder failed: ${String(error)}`);
    }
  },

  openPiece: async (path) => {
    try {
      const piece = await playFile(path);
      set({ piece, piecePath: path, verdicts: {} });
    } catch (error) {
      log.error(`failed to play ${path}: ${String(error)}`);
    }
  },

  resume: async () => {
    try {
      await play();
    } catch (error) {
      log.error(`starting playback failed: ${String(error)}`);
    }
  },

  stop: async () => {
    try {
      await stopPlayback();
    } catch (error) {
      log.error(`stopping playback failed: ${String(error)}`);
    }
  },

  toggleFollow: async (follow) => {
    try {
      await setFollow(follow);
      set({ follow });
    } catch (error) {
      log.error(`toggling follow mode failed: ${String(error)}`);
    }
  },

  changeHand: async (hand) => {
    try {
      await setPlayerHand(hand);
      set((state) => ({
        settings: state.settings && { ...state.settings, player_hand: hand },
      }));
      // The gates and the app's part are built when a piece loads, so rebuild it.
      const path = get().piecePath;
      if (path) {
        await get().openPiece(path);
      }
    } catch (error) {
      log.error(`setting the practice hand failed: ${String(error)}`);
    }
  },

  changeTempo: async (percent) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, tempo_percent: percent },
    }));
    try {
      await setTempoPercent(percent);
      scheduleSave();
    } catch (error) {
      log.error(`setting tempo failed: ${String(error)}`);
    }
  },

  changeTranspose: async (semitones) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, transpose: semitones },
    }));
    try {
      await setTranspose(semitones);
      scheduleSave();
    } catch (error) {
      log.error(`setting transpose failed: ${String(error)}`);
    }
  },

  changeVolume: async (volume) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, volume },
    }));
    try {
      await setVolume(volume);
      scheduleSave();
    } catch (error) {
      log.error(`setting volume failed: ${String(error)}`);
    }
  },

  recordJudgements: (judgements) => {
    const verdicts = { ...get().verdicts };
    for (const judgement of judgements) {
      const expected = judgement.expected_pitch ?? judgement.pitch;
      verdicts[verdictKey(expected, judgement.expected_start_ms)] =
        judgement.verdict;
    }
    set({ verdicts });
  },

  pressPitch: (pitch, down) => {
    const pressed = get().pressed.filter((held) => held !== pitch);
    set({ pressed: down ? [...pressed, pitch] : pressed });
  },
}));
