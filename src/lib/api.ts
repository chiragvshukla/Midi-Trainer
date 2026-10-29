import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  os: string;
  arch: string;
}

export type Verdict = "Correct" | "Wrong";
export type Timing = "Ahead" | "OnTime" | "Behind";

export interface SoundfontStatus {
  installed: boolean;
  path: string;
}

export interface DownloadProgress {
  received: number;
  total: number;
}

export interface Settings {
  recent_files: string[];
  midi_device: string | null;
  volume: number;
  tempo_percent: number;
  transpose: number;
}

export interface NoteInfo {
  pitch: number;
  start_ms: number;
  end_ms: number;
}

/** Notatable note heads, longest to shortest, as the engine names them. */
export type Base = "Whole" | "Half" | "Quarter" | "Eighth" | "Sixteenth" | "ThirtySecond";

export type Duration =
  | { type: "Straight"; base: Base; dotted: boolean }
  | { type: "Triplet"; base: Base };

export interface TimeSignature {
  numerator: number;
  denominator: number;
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

export type VoiceEvent = ({ type: "Chord" } & Chord) | { type: "Rest"; duration: Duration };

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
}

export interface Piece {
  notes: NoteInfo[];
  duration_ms: number;
  score: Score;
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

export function playFile(path: string): Promise<Piece> {
  return invoke<Piece>("play_file", { path });
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

export function setVolume(volume: number): Promise<void> {
  return invoke("set_volume", { volume });
}

export function setFollow(follow: boolean): Promise<void> {
  return invoke("set_follow", { follow });
}

export function playbackPosition(): Promise<number> {
  return invoke<number>("playback_position");
}

export function keyDown(code: string): Promise<number | null> {
  return invoke<number | null>("key_down", { code });
}

export function keyUp(code: string): Promise<void> {
  return invoke("key_up", { code });
}
