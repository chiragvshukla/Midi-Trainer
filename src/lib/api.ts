import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  os: string;
  arch: string;
}

export type Verdict = "Correct" | "Wrong";
export type Timing = "Ahead" | "OnTime" | "Behind";

/** Which hand the player takes in one-hand practice; the app plays the other. */
export type Hand = "Both" | "Left" | "Right";

/** How strictly the player's notes are judged. */
export type Skill = "Listen" | "Follow" | "Strict";

export interface SoundfontStatus {
  installed: boolean;
  path: string;
}

export interface DownloadProgress {
  received: number;
  total: number;
}

/** One key on the PC-keyboard fallback: the browser key code and the MIDI pitch it plays. */
export interface PcKey {
  code: string;
  pitch: number;
}

export interface Settings {
  recent_files: string[];
  library_folders: string[];
  midi_device: string | null;
  music_volume: number;
  player_volume: number;
  cue_volume: number;
  output_volume: number;
  tempo_percent: number;
  transpose: number;
  player_hand: Hand;
  player_track: number | null;
  hand_split: boolean;
  play_along: boolean;
  skill: Skill;
}

export interface NoteInfo {
  /** The file track this note belongs to, so the view can colour by track. */
  track: number;
  pitch: number;
  start_ms: number;
  end_ms: number;
}

/** Notatable note heads, longest to shortest, as the engine names them. */
export type Base =
  "Whole" | "Half" | "Quarter" | "Eighth" | "Sixteenth" | "ThirtySecond";

export type Duration =
  | { type: "Straight"; base: Base; dotted: boolean }
  | { type: "Triplet"; base: Base };

export interface TimeSignature {
  numerator: number;
  denominator: number;
}

/** A key signature: number of sharps (negative for flats) and whether the key is minor. */
export interface KeySignature {
  fifths: number;
  minor: boolean;
}

export interface ChordNote {
  pitch: number;
  /** The original note's onset, to match playback verdicts. */
  onset_ms: number;
  tied_from_previous: boolean;
  tied_to_next: boolean;
}

export interface Chord {
  duration: Duration;
  notes: ChordNote[];
}

export type VoiceEvent =
  ({ type: "Chord" } & Chord) | { type: "Rest"; duration: Duration };

export interface Voice {
  events: VoiceEvent[];
}

export interface Measure {
  start_ms: number;
  /** 1-based; the pickup measure, if any, is 0. */
  number: number;
  time_signature: TimeSignature;
  treble: Voice[];
  bass: Voice[];
}

export interface Score {
  measures: Measure[];
  /** The piece's key, when the file declares one. */
  key_signature: KeySignature | null;
}

/** One track of the loaded file, as the track picker shows it. */
export interface TrackInfo {
  index: number;
  name: string | null;
  program: number | null;
  /** The channel the track's notes use; 9 marks a percussion track. */
  channel: number | null;
  note_count: number;
}

/** One karaoke lyric syllable, placed on the timeline. */
export interface LyricAt {
  time_ms: number;
  text: string;
}

export interface Piece {
  notes: NoteInfo[];
  duration_ms: number;
  score: Score;
  /** Every track in the file; `notes` is filtered down to the selected ones. */
  tracks: TrackInfo[];
  /** Karaoke lyric syllables, in time order; empty for files without lyrics. */
  lyrics: LyricAt[];
}

export interface Judgement {
  pitch: number;
  verdict: Verdict;
  timing: Timing;
  offset_ms: number;
  expected_pitch: number | null;
  expected_start_ms: number;
}

export interface PlayerNote {
  pitch: number;
  down: boolean;
}

/** A practice session's running tallies, from the engine. */
export interface PracticeStats {
  attempts: number;
  correct: number;
  wrong: number;
  ahead: number;
  on_time: number;
  behind: number;
  accuracy: number;
  average_abs_offset_ms: number;
}

/** One MIDI file the library can open. */
export interface LibraryEntry {
  name: string;
  relative_path: string;
}

export function fetchAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function fetchSoundfontStatus(): Promise<SoundfontStatus> {
  return invoke<SoundfontStatus>("soundfont_status");
}

export function downloadSoundfont(): Promise<string> {
  return invoke<string>("download_soundfont");
}

export function cancelSoundfontDownload(): Promise<void> {
  return invoke("cancel_soundfont_download");
}

export function loadSoundfont(path: string): Promise<void> {
  return invoke("load_soundfont", { path });
}

export function fetchSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function playFile(
  path: string,
  tracks: number[] | null,
): Promise<Piece> {
  return invoke<Piece>("play_file", { path, tracks });
}

export function play(): Promise<void> {
  return invoke("play");
}

export function saveSettings(): Promise<void> {
  return invoke("save_settings");
}

export function listMidiInputs(): Promise<string[]> {
  return invoke<string[]>("list_midi_inputs");
}

export function listLibrary(folder: string): Promise<LibraryEntry[]> {
  return invoke<LibraryEntry[]>("list_library", { folder });
}

export function setLibraryFolders(folders: string[]): Promise<void> {
  return invoke("set_library_folders", { folders });
}

export function setPlayerPart(hand: Hand, track: number | null): Promise<void> {
  return invoke("set_player_part", { hand, track });
}

export function setHandSplit(on: boolean): Promise<void> {
  return invoke("set_hand_split", { on });
}

export function setPlayAlong(on: boolean): Promise<void> {
  return invoke("set_play_along", { on });
}

export function connectMidiInput(name: string): Promise<void> {
  return invoke("connect_midi_input", { name });
}

export function disconnectMidiInput(): Promise<void> {
  return invoke("disconnect_midi_input");
}

export function stopPlayback(): Promise<void> {
  return invoke("stop_playback");
}

export function setTempoPercent(percent: number): Promise<void> {
  return invoke("set_tempo_percent", { percent });
}

export function setTranspose(semitones: number): Promise<void> {
  return invoke("set_transpose", { semitones });
}

export function setMusicVolume(volume: number): Promise<void> {
  return invoke("set_music_volume", { volume });
}

export function setPlayerVolume(volume: number): Promise<void> {
  return invoke("set_player_volume", { volume });
}

export function setCueVolume(volume: number): Promise<void> {
  return invoke("set_cue_volume", { volume });
}

export function setOutputVolume(volume: number): Promise<void> {
  return invoke("set_output_volume", { volume });
}

export function setTrackVolume(channel: number, gain: number): Promise<void> {
  return invoke("set_track_volume", { channel, gain });
}

export function setSkill(skill: Skill): Promise<void> {
  return invoke("set_skill", { skill });
}

export function playbackPosition(): Promise<number> {
  return invoke<number>("playback_position");
}

export function seek(positionMs: number): Promise<void> {
  return invoke("seek", { positionMs });
}

export function keyDown(code: string): Promise<number | null> {
  return invoke<number | null>("key_down", { code });
}

export function keyUp(code: string): Promise<void> {
  return invoke("key_up", { code });
}

export function pcKeyboardLayout(): Promise<PcKey[]> {
  return invoke<PcKey[]>("pc_keyboard_layout");
}
