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
  setSkill,
  setLibraryFolders,
  setMusicVolume,
  setPlayerPart,
  setPlayerVolume,
  setTempoPercent,
  setTrackVolume,
  setTranspose,
  stopPlayback,
  type Hand,
  type Judgement,
  type LibraryEntry,
  type Piece,
  type PracticeStats,
  type Settings,
  type Skill,
  type SoundfontStatus,
  type Timing,
  type Verdict,
} from "../lib/api";
import { log } from "../lib/logger";

export interface DownloadState {
  received: number;
  total: number;
}

/** One library folder and the MIDI files found under it. */
export interface LibraryGroup {
  folder: string;
  entries: LibraryEntry[];
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
  /** The folders the user browses as their library. */
  libraryFolders: string[];
  /** Each library folder's MIDI files, in folder order. */
  libraries: LibraryGroup[];
  piece: Piece | null;
  /** Path of the loaded piece, so the library can highlight it. */
  piecePath: string | null;
  /** Track indices to keep; null means every track. */
  selectedTracks: number[] | null;
  /** Per-channel track gain, 0 to 1; a channel absent from the map plays at full. */
  trackVolumes: Record<number, number>;
  /** Verdict per falling note, keyed by pitch and score time. */
  verdicts: Record<string, Verdict>;
  /** The running practice tallies, from the engine, once a note has been judged. */
  stats: PracticeStats | null;
  /** Timing of the most recent judged note, for the live marker. */
  lastTiming: Timing | null;
  /** Offset in milliseconds of the most recent judged note; negative is early. */
  lastOffsetMs: number | null;
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
  addLibraryFolder: () => Promise<void>;
  removeLibraryFolder: (folder: string) => Promise<void>;
  openPiece: (path: string) => Promise<void>;
  toggleTrack: (index: number) => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  changeSkill: (skill: Skill) => Promise<void>;
  changePart: (hand: Hand, track: number | null) => Promise<void>;
  changeTempo: (percent: number) => Promise<void>;
  changeTranspose: (semitones: number) => Promise<void>;
  changeMusicVolume: (volume: number) => Promise<void>;
  changePlayerVolume: (volume: number) => Promise<void>;
  changeTrackVolume: (channel: number, gain: number) => Promise<void>;
  recordJudgements: (judgements: Judgement[]) => void;
  recordStats: (stats: PracticeStats) => void;
  pressPitch: (pitch: number, down: boolean) => void;
}

export const useSession = create<SessionState>((set, get) => ({
  soundfont: null,
  download: null,
  soundfontError: null,
  settings: null,
  devices: [],
  libraryFolders: [],
  libraries: [],
  piece: null,
  piecePath: null,
  selectedTracks: null,
  trackVolumes: {},
  verdicts: {},
  stats: null,
  lastTiming: null,
  lastOffsetMs: null,
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
      set({ libraryFolders: settings.library_folders });
      // The engine starts fresh every run; put the saved practice setup back.
      await setMusicVolume(settings.music_volume);
      await setPlayerVolume(settings.player_volume);
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
    const folders = get().libraryFolders;
    if (folders.length === 0) {
      set({ libraries: [] });
      return;
    }
    const libraries = await Promise.all(
      folders.map(async (folder) => {
        try {
          return { folder, entries: await listLibrary(folder) };
        } catch (error) {
          log.error(`listing ${folder} failed: ${String(error)}`);
          return { folder, entries: [] };
        }
      }),
    );
    set({ libraries });
  },

  addLibraryFolder: async () => {
    try {
      const folder = await open({ directory: true });
      if (typeof folder !== "string" || get().libraryFolders.includes(folder)) {
        return;
      }
      const folders = [...get().libraryFolders, folder];
      await setLibraryFolders(folders);
      set({ libraryFolders: folders });
      await get().loadLibrary();
    } catch (error) {
      log.error(`adding a library folder failed: ${String(error)}`);
    }
  },

  removeLibraryFolder: async (folder) => {
    const folders = get().libraryFolders.filter((path) => path !== folder);
    try {
      await setLibraryFolders(folders);
      set({ libraryFolders: folders });
      await get().loadLibrary();
    } catch (error) {
      log.error(`removing a library folder failed: ${String(error)}`);
    }
  },

  openPiece: async (path) => {
    try {
      // Reopening the same piece keeps the track selection; a new one starts with every track.
      const tracks = path === get().piecePath ? get().selectedTracks : null;
      const piece = await playFile(path, tracks);
      set({
        piece,
        piecePath: path,
        selectedTracks: tracks,
        verdicts: {},
        stats: null,
        lastTiming: null,
        lastOffsetMs: null,
      });
    } catch (error) {
      log.error(`failed to play ${path}: ${String(error)}`);
    }
  },

  toggleTrack: async (index) => {
    const piece = get().piece;
    if (!piece) {
      return;
    }
    const selected =
      get().selectedTracks ?? piece.tracks.map((track) => track.index);
    const next = selected.includes(index)
      ? selected.filter((track) => track !== index)
      : [...selected, index].sort((a, b) => a - b);
    // Every track selected is the same as no filter at all.
    set({
      selectedTracks: next.length === piece.tracks.length ? null : next,
    });
    const path = get().piecePath;
    if (path) {
      await get().openPiece(path);
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

  changeSkill: async (skill) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, skill },
    }));
    try {
      await setSkill(skill);
    } catch (error) {
      log.error(`setting the skill level failed: ${String(error)}`);
    }
  },

  changePart: async (hand, track) => {
    try {
      await setPlayerPart(hand, track);
      set((state) => ({
        settings: state.settings && {
          ...state.settings,
          player_hand: hand,
          player_track: track,
        },
      }));
      // The gates, matcher and the app's part are built when a piece loads, so rebuild it.
      const path = get().piecePath;
      if (path) {
        await get().openPiece(path);
      }
    } catch (error) {
      log.error(`setting the player's part failed: ${String(error)}`);
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

  changeMusicVolume: async (volume) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, music_volume: volume },
    }));
    try {
      await setMusicVolume(volume);
      scheduleSave();
    } catch (error) {
      log.error(`setting the music volume failed: ${String(error)}`);
    }
  },

  changePlayerVolume: async (volume) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, player_volume: volume },
    }));
    try {
      await setPlayerVolume(volume);
      scheduleSave();
    } catch (error) {
      log.error(`setting the player volume failed: ${String(error)}`);
    }
  },

  changeTrackVolume: async (channel, gain) => {
    set((state) => ({
      trackVolumes: { ...state.trackVolumes, [channel]: gain },
    }));
    try {
      await setTrackVolume(channel, gain);
    } catch (error) {
      log.error(`setting a track's volume failed: ${String(error)}`);
    }
  },

  recordJudgements: (judgements) => {
    const verdicts = { ...get().verdicts };
    for (const judgement of judgements) {
      const expected = judgement.expected_pitch ?? judgement.pitch;
      verdicts[verdictKey(expected, judgement.expected_start_ms)] =
        judgement.verdict;
    }
    const last = judgements[judgements.length - 1];
    set({
      verdicts,
      lastTiming: last ? last.timing : get().lastTiming,
      lastOffsetMs: last ? last.offset_ms : get().lastOffsetMs,
    });
  },

  recordStats: (stats) => set({ stats }),

  pressPitch: (pitch, down) => {
    const pressed = get().pressed.filter((held) => held !== pitch);
    set({ pressed: down ? [...pressed, pitch] : pressed });
  },
}));
