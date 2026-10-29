import { create } from "zustand";
import { open } from "@tauri-apps/plugin-dialog";
import {
  cancelSoundfontDownload,
  connectMidiInput,
  deleteLibraryEntry,
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
  seek,
  setSkill,
  setLibraryFolders,
  setHandSplit,
  setPlayAlong,
  setMusicVolume,
  setPlayerPart,
  setPlayerVolume,
  setCueVolume,
  setOutputVolume,
  setTempoPercent,
  setTrackVolume,
  setTranspose,
  setSoundfont,
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

/** Optimistically applies a numeric setting, pushes it to the engine, then persists it. */
function applyNumericSetting(
  set: (updater: (state: SessionState) => Partial<SessionState>) => void,
  label: string,
  patch: Partial<Settings>,
  apply: () => Promise<void>,
): Promise<void> {
  set((state) => ({
    settings: state.settings ? { ...state.settings, ...patch } : state.settings,
  }));
  return apply()
    .then(scheduleSave)
    .catch((error) => log.error(`${label}: ${String(error)}`));
}

/** Key of one falling note in the verdict map: pitch and score time. */
export function verdictKey(pitch: number, startMs: number): string {
  return `${pitch}:${Math.round(startMs)}`;
}

/** A path in one canonical form for comparing: forward slashes, lowercased. */
export function normalizePath(path: string): string {
  return path.replace(/\\/g, "/").toLowerCase();
}

/** How long a wrong played key stays red on the on-screen keyboard. Short, so fast passages
 * do not keep old wrong keys lit. */
export const WRONG_FLASH_MS = 120;

/** A pitch the player just played wrong, and when, so the keyboard can flash it red. */
export interface WrongFlash {
  pitch: number;
  at: number;
}

/** The bar line nearest `ms` in the engraved measures, so loop points land on a bar. */
export function nearestBarMs(piece: Piece | null, ms: number): number {
  const measures = piece?.score.measures ?? [];
  let best = ms;
  let bestDistance = Infinity;
  for (const measure of measures) {
    const distance = Math.abs(measure.start_ms - ms);
    if (distance < bestDistance) {
      bestDistance = distance;
      best = measure.start_ms;
    }
  }
  return best;
}

/** The start of the bar before the one holding `ms`; the piece's start at the first bar. */
export function previousBarMs(piece: Piece | null, ms: number): number {
  let previous = 0;
  for (const measure of piece?.score.measures ?? []) {
    if (measure.start_ms < ms - 1) {
      previous = measure.start_ms;
    } else {
      break;
    }
  }
  return previous;
}

/** The start of the bar after the one holding `ms`; the piece's end at the last bar. */
export function nextBarMs(piece: Piece | null, ms: number): number {
  for (const measure of piece?.score.measures ?? []) {
    if (measure.start_ms > ms + 1) {
      return measure.start_ms;
    }
  }
  return Math.max(ms, piece?.duration_ms ?? ms);
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
  /** Pitches just played wrong, for the keyboard's red flash. */
  wrongFlashes: WrongFlash[];
  /** The running practice tallies, from the engine, once a note has been judged. */
  stats: PracticeStats | null;
  /** Timing of the most recent judged note, for the live marker. */
  lastTiming: Timing | null;
  /** Offset in milliseconds of the most recent judged note; negative is early. */
  lastOffsetMs: number | null;
  /** Pitches currently held down on any keyboard. */
  pressed: number[];
  /** The playhead in milliseconds: updated by seeks and by the transport's position poll. */
  positionMs: number;
  /** Loop region start in milliseconds, snapped to a bar; null until set. */
  loopStartMs: number | null;
  /** Loop region end in milliseconds, snapped to a bar; null until set. */
  loopEndMs: number | null;
  /** Whether playback loops the region instead of running to the end. */
  looping: boolean;
  /** Whether the transport is playing, for the play/stop toggle. */
  playing: boolean;
  /** First-run setup: load a SoundFont, then the saved settings. */
  startup: () => Promise<void>;
  initSoundfont: () => Promise<void>;
  loadSettings: () => Promise<Settings | null>;
  cancelDownload: () => Promise<void>;
  updateDownload: (progress: DownloadState | null) => void;
  /** Downloads the full GM bank and switches to it. */
  downloadFullSoundfont: () => Promise<void>;
  /** Picks a `.sf2` file for the user and switches to it. */
  chooseSoundfontFile: () => Promise<void>;
  /** Returns to the bundled baseline font. */
  useBundledSoundfont: () => Promise<void>;
  /** Switches to the downloaded full bank, if present. */
  useDownloadedSoundfont: () => Promise<void>;
  loadDevices: () => Promise<void>;
  connectDevice: (name: string | null) => Promise<void>;
  loadLibrary: () => Promise<void>;
  addLibraryFolder: () => Promise<void>;
  removeLibraryFolder: (folder: string) => Promise<void>;
  /**
   * Removes a file from the library. With `deleteStorage` the file is deleted too; `remember`
   * stores that choice so later removals skip the prompt.
   */
  removeFromLibrary: (
    path: string,
    deleteStorage: boolean,
    remember: boolean,
  ) => Promise<void>;
  openPiece: (path: string) => Promise<void>;
  toggleTrack: (index: number) => Promise<void>;
  resume: () => Promise<void>;
  stop: () => Promise<void>;
  seekTo: (positionMs: number) => Promise<void>;
  setPositionMs: (positionMs: number) => void;
  startOver: () => Promise<void>;
  previousBar: () => Promise<void>;
  nextBar: () => Promise<void>;
  resetStats: () => Promise<void>;
  setLoopStart: () => void;
  setLoopEnd: () => void;
  toggleLoop: () => void;
  clearLoop: () => void;
  changeSkill: (skill: Skill) => Promise<void>;
  changePart: (hand: Hand, track: number | null) => Promise<void>;
  changeHandSplit: (on: boolean) => Promise<void>;
  changePlayAlong: (on: boolean) => Promise<void>;
  changeTempo: (percent: number) => Promise<void>;
  changeTranspose: (semitones: number) => Promise<void>;
  changeMusicVolume: (volume: number) => Promise<void>;
  changePlayerVolume: (volume: number) => Promise<void>;
  changeCueVolume: (volume: number) => Promise<void>;
  changeOutputVolume: (volume: number) => Promise<void>;
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
  wrongFlashes: [],
  stats: null,
  lastTiming: null,
  lastOffsetMs: null,
  pressed: [],
  positionMs: 0,
  loopStartMs: null,
  loopEndMs: null,
  looping: false,
  playing: false,

  startup: async () => {
    await get().initSoundfont();
    await get().loadSettings();
    await get().loadLibrary();
  },

  initSoundfont: async () => {
    set({ soundfontError: null, download: null });
    try {
      const soundfont = await fetchSoundfontStatus();
      set({ soundfont });
      if (soundfont.path) {
        await loadSoundfont(soundfont.path);
      } else {
        set({ soundfontError: "No SoundFont is available." });
      }
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`soundfont setup failed: ${String(error)}`);
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

  downloadFullSoundfont: async () => {
    set({ soundfontError: null, download: { received: 0, total: 0 } });
    try {
      const path = await downloadSoundfont();
      set({ soundfont: await setSoundfont(path) });
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`downloading the full SoundFont failed: ${String(error)}`);
    } finally {
      set({ download: null });
    }
  },

  chooseSoundfontFile: async () => {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "SoundFont", extensions: ["sf2"] }],
      });
      if (typeof path !== "string") {
        return;
      }
      set({ soundfont: await setSoundfont(path), soundfontError: null });
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`choosing a SoundFont failed: ${String(error)}`);
    }
  },

  useBundledSoundfont: async () => {
    try {
      set({ soundfont: await setSoundfont(null), soundfontError: null });
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`switching to the bundled SoundFont failed: ${String(error)}`);
    }
  },

  useDownloadedSoundfont: async () => {
    const full = get().soundfont?.full_path;
    if (!full) {
      return;
    }
    try {
      set({ soundfont: await setSoundfont(full), soundfontError: null });
    } catch (error) {
      set({ soundfontError: String(error) });
      log.error(`switching to the full SoundFont failed: ${String(error)}`);
    }
  },

  loadSettings: async () => {
    try {
      const settings = await fetchSettings();
      set({ settings });
      set({ libraryFolders: settings.library_folders });
      // The engine starts fresh every run; put the saved practice setup back.
      await setMusicVolume(settings.music_volume);
      await setPlayerVolume(settings.player_volume);
      await setCueVolume(settings.cue_volume);
      await setOutputVolume(settings.output_volume);
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

  removeFromLibrary: async (path, deleteStorage, remember) => {
    await deleteLibraryEntry(
      path,
      deleteStorage,
      remember ? deleteStorage : null,
    );
    const key = normalizePath(path);
    // An open file that was deleted must be let go of, or the engine holds a piece that is gone.
    const current = get().piecePath;
    if (deleteStorage && current && normalizePath(current) === key) {
      await get().stop();
      set({ piece: null, piecePath: null });
    }
    set((state) => {
      if (!state.settings) {
        return {};
      }
      const settings = state.settings;
      return {
        settings: {
          ...settings,
          recent_files: settings.recent_files.filter(
            (entry) => normalizePath(entry) !== key,
          ),
          hidden_files: settings.hidden_files.some(
            (entry) => normalizePath(entry) === key,
          )
            ? settings.hidden_files
            : [...settings.hidden_files, key],
          delete_from_storage: remember
            ? deleteStorage
            : settings.delete_from_storage,
        },
      };
    });
  },

  openPiece: async (path) => {
    try {
      // A new piece starts clean: every track, and the player back to both hands, so the
      // previous piece's part cannot carry over to a file with different tracks.
      const isNew = path !== get().piecePath;
      const tracks = isNew ? null : get().selectedTracks;
      if (isNew) {
        await setPlayerPart("Both", null);
        set((state) => ({
          settings: state.settings && {
            ...state.settings,
            player_hand: "Both",
            player_track: null,
          },
        }));
      }
      const piece = await playFile(path, tracks);
      set({
        piece,
        piecePath: path,
        selectedTracks: tracks,
        verdicts: {},
        wrongFlashes: [],
        stats: null,
        lastTiming: null,
        lastOffsetMs: null,
        positionMs: 0,
        loopStartMs: null,
        loopEndMs: null,
        looping: false,
        playing: false,
      });
      // play_file records the path in the backend's Recent list and returns it, so the library
      // shows the file right away without a second settings round-trip.
      set((state) => ({
        settings: state.settings && {
          ...state.settings,
          recent_files: piece.recent_files,
        },
      }));
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
      const piece = get().piece;
      // A piece that already ran to its end plays again from the top; otherwise resume where the
      // playhead was left.
      if (
        piece &&
        piece.duration_ms > 0 &&
        get().positionMs >= piece.duration_ms
      ) {
        await get().seekTo(0);
      }
      await play();
      set({ playing: true });
    } catch (error) {
      log.error(`starting playback failed: ${String(error)}`);
    }
  },

  stop: async () => {
    // Flip the flag first: the transport poll can call this at the end of a piece, and the next
    // poll must see "not playing" rather than firing another stop.
    set({ playing: false });
    try {
      await stopPlayback();
    } catch (error) {
      log.error(`stopping playback failed: ${String(error)}`);
    }
  },

  seekTo: async (positionMs) => {
    if (!get().piece) {
      return;
    }
    const target = Math.max(0, Math.round(positionMs));
    // Set it here and now, so a repeat press steps from the new point instead of reading the
    // engine's position before it has processed the previous seek.
    set({ positionMs: target });
    try {
      await seek(target);
      // The engine rewinds its scoring with the playhead; the readout mirrors that reset.
      set({
        verdicts: {},
        wrongFlashes: [],
        stats: null,
        lastTiming: null,
        lastOffsetMs: null,
      });
    } catch (error) {
      log.error(`seeking failed: ${String(error)}`);
    }
  },

  setPositionMs: (positionMs) => {
    set({ positionMs });
    // The transport poll reports the playhead here; a piece reaching its end on its own stops,
    // so the button and the Space key return to "play" instead of a no-op stop.
    const duration = get().piece?.duration_ms ?? 0;
    if (get().playing && duration > 0 && positionMs >= duration) {
      void get().stop();
    }
  },

  startOver: async () => {
    await get().seekTo(0);
  },

  previousBar: async () => {
    await get().seekTo(previousBarMs(get().piece, get().positionMs));
  },

  nextBar: async () => {
    await get().seekTo(nextBarMs(get().piece, get().positionMs));
  },

  resetStats: async () => {
    // A seek rewinds the engine's matcher and clears the readout, so resetting is a seek to
    // where the playhead already is.
    await get().seekTo(get().positionMs);
  },

  setLoopStart: () => {
    const piece = get().piece;
    const start = nearestBarMs(piece, get().positionMs);
    const end = get().loopEndMs;
    // Keep the region valid: if the new start is at or past the end, either swap them or push
    // the end to the next bar, so two presses always make a loop instead of an empty one.
    if (end === null) {
      set({ loopStartMs: start });
    } else if (start > end) {
      set({ loopStartMs: end, loopEndMs: start });
    } else {
      set({
        loopStartMs: start,
        loopEndMs: start === end ? nextBarMs(piece, start) : end,
      });
    }
  },

  setLoopEnd: () => {
    const piece = get().piece;
    const end = nearestBarMs(piece, get().positionMs);
    const start = get().loopStartMs;
    if (start === null) {
      set({ loopEndMs: end });
    } else if (end < start) {
      set({ loopStartMs: end, loopEndMs: start });
    } else {
      set({
        loopEndMs: end === start ? nextBarMs(piece, start) : end,
        loopStartMs: start,
      });
    }
  },

  toggleLoop: () => set((state) => ({ looping: !state.looping })),

  clearLoop: () => set({ loopStartMs: null, loopEndMs: null, looping: false }),

  changeSkill: async (skill) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, skill },
    }));
    try {
      await setSkill(skill);
      // The score the app plays depends on the mode (a guide, silent, or the other hand), so
      // a loaded piece is rebuilt; nothing to do before one is loaded.
      const path = get().piecePath;
      if (path) {
        await get().openPiece(path);
      }
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

  changeHandSplit: async (on) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, hand_split: on },
    }));
    try {
      await setHandSplit(on);
      // Reload only if a hand is selected: turning off drops it back to both, turning on
      // restores it. Both change the part the engine expects.
      const hand = get().settings?.player_hand ?? "Both";
      if (hand !== "Both") {
        await get().changePart(on ? hand : "Both", null);
      }
    } catch (error) {
      log.error(`setting hand practice failed: ${String(error)}`);
    }
  },

  changePlayAlong: async (on) => {
    set((state) => ({
      settings: state.settings && { ...state.settings, play_along: on },
    }));
    try {
      await setPlayAlong(on);
      // The accompaniment is scheduled when a piece loads, so rebuild it to hear the change.
      const path = get().piecePath;
      if (path) {
        await get().openPiece(path);
      }
    } catch (error) {
      log.error(`setting play-along failed: ${String(error)}`);
    }
  },

  changeTempo: (percent) =>
    applyNumericSetting(set, "setting tempo", { tempo_percent: percent }, () =>
      setTempoPercent(percent),
    ),

  changeTranspose: (semitones) =>
    applyNumericSetting(
      set,
      "setting transpose",
      { transpose: semitones },
      () => setTranspose(semitones),
    ),

  changeMusicVolume: (volume) =>
    applyNumericSetting(
      set,
      "setting the music volume",
      { music_volume: volume },
      () => setMusicVolume(volume),
    ),

  changePlayerVolume: (volume) =>
    applyNumericSetting(
      set,
      "setting the player volume",
      { player_volume: volume },
      () => setPlayerVolume(volume),
    ),

  changeCueVolume: (volume) =>
    applyNumericSetting(
      set,
      "setting the cue volume",
      { cue_volume: volume },
      () => setCueVolume(volume),
    ),

  changeOutputVolume: (volume) =>
    applyNumericSetting(
      set,
      "setting the output volume",
      { output_volume: volume },
      () => setOutputVolume(volume),
    ),

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
    const now = performance.now();
    const verdicts = { ...get().verdicts };
    const flashes: { pitch: number; at: number }[] = [];
    let last: Judgement | undefined;
    for (const judgement of judgements) {
      const expected = judgement.expected_pitch ?? judgement.pitch;
      verdicts[verdictKey(expected, judgement.expected_start_ms)] =
        judgement.verdict;
      // A missed note marks the note itself but was never pressed, so it has no key to flash
      // red and no timing to report; only a played note drives the marker.
      if (!judgement.missed) {
        last = judgement;
        if (judgement.verdict === "Wrong") {
          flashes.push({ pitch: judgement.pitch, at: now });
        }
      }
    }
    // Expired flashes drop off as new ones come.
    const wrongFlashes = [
      ...get().wrongFlashes.filter((flash) => now - flash.at < WRONG_FLASH_MS),
      ...flashes,
    ];
    set({
      verdicts,
      wrongFlashes,
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
