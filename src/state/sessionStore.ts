import { create } from "zustand";
import {
  cancelSoundfontDownload,
  downloadSoundfont,
  fetchSettings,
  fetchSoundfontStatus,
  loadSoundfont,
  playFile,
  setFollow,
  setTempoPercent,
  setTranspose,
  setVolume,
  stopPlayback,
  type Judgement,
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

/** Key of one falling note in the verdict map: pitch and score time. */
export function verdictKey(pitch: number, startMs: number): string {
  return `${pitch}:${Math.round(startMs)}`;
}

interface SessionState {
  soundfont: SoundfontStatus | null;
  download: DownloadState | null;
  soundfontError: string | null;
  settings: Settings | null;
  piece: Piece | null;
  follow: boolean;
  /** Verdict per falling note, keyed by pitch and score time. */
  verdicts: Record<string, Verdict>;
  /** Pitches currently held down on any keyboard. */
  pressed: number[];
  initSoundfont: () => Promise<void>;
  cancelDownload: () => Promise<void>;
  updateDownload: (progress: DownloadState | null) => void;
  loadSettings: () => Promise<Settings | null>;
  openPiece: (path: string) => Promise<void>;
  stop: () => Promise<void>;
  toggleFollow: (follow: boolean) => Promise<void>;
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
  piece: null,
  follow: false,
  verdicts: {},
  pressed: [],

  initSoundfont: async () => {
    set({ soundfontError: null, download: null });
    try {
      const soundfont = await fetchSoundfontStatus();
      set({ soundfont });
      if (soundfont.installed) {
        await loadSoundfont(soundfont.path);
        return;
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
      // The engine starts fresh every run; put the saved practice setup back.
      await setVolume(settings.volume);
      await setTempoPercent(settings.tempo_percent);
      await setTranspose(settings.transpose);
      return settings;
    } catch (error) {
      log.error(`failed to load settings: ${String(error)}`);
      return null;
    }
  },

  openPiece: async (path) => {
    try {
      const piece = await playFile(path);
      set({ piece, verdicts: {} });
    } catch (error) {
      log.error(`failed to play ${path}: ${String(error)}`);
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

  changeTempo: async (percent) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, tempo_percent: percent },
    }));
    try {
      await setTempoPercent(percent);
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
