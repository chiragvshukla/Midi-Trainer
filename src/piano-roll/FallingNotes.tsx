import { useEffect, useRef, useState } from "react";
import { playbackPosition, type NoteInfo, type Verdict } from "../lib/api";
import { canvasColors, useTheme } from "../lib/theme";
import { useSession, verdictKey } from "../state/sessionStore";

/** Pitch classes drawn as black keys (C#, D#, F#, G#, A#). */
const BLACK_CLASSES = new Set([1, 3, 6, 8, 10]);

/** How much score time is visible above the keyboard at zoom 1, in milliseconds. */
const LOOKAHEAD_MS = 4000;

/** Middle C: the boundary between the hands, as the engine splits them. */
const MIDDLE_C = 60;

/** Zoom limits: below 1 fits more time in, above 1 spreads it out. */
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 4;

/** Fraction of the view the on-screen keyboard takes. */
const KEYBOARD_SHARE = 0.24;

/** Geometry of one piano key within the keyboard strip. */
interface KeyRect {
  x: number;
  width: number;
  black: boolean;
}

/** A full 88-key keyboard: A0 (21) through C8 (108). */
const LOWEST_KEY = 21;
const HIGHEST_KEY = 108;

/** The keyboard is always a full 88 keys; only the longest note depends on the piece. */
function longestNoteMs(notes: NoteInfo[]): number {
  let maxDurationMs = 0;
  for (const note of notes) {
    maxDurationMs = Math.max(maxDurationMs, note.end_ms - note.start_ms);
  }
  return maxDurationMs;
}

/**
 * The falling-note view: score notes scroll onto an on-screen keyboard, keys light up while
 * their notes sound or while the player holds them, and played notes are colored by verdict.
 * Rendering only — position comes from the engine, verdicts from the session store.
 */
export function FallingNotes() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const piece = useSession((state) => state.piece);
  const verdicts = useSession((state) => state.verdicts);
  const pressed = useSession((state) => state.pressed);
  const settings = useSession((state) => state.settings);
  const theme = useTheme((state) => state.theme);
  // Zoom lives here, so it is remembered across toggles to the sheet view and back.
  const [zoom, setZoom] = useState(1);

  // A single hand taken with hand practice: only that hand's notes are drawn. Otherwise both
  // hands show, coloured apart, unless a whole track is the player's part.
  const soloHand =
    settings &&
    settings.player_track == null &&
    settings.hand_split &&
    settings.player_hand !== "Both"
      ? settings.player_hand
      : null;
  // Hand colours are only used when hand practice is on; otherwise each track has its own
  // colour, so a multi-track piece reads apart at a glance.
  const byHand = settings?.hand_split ?? false;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    let frame = 0;

    // Colours come from the theme tokens, so the roll follows a light/dark switch.
    const COLORS = canvasColors();

    // The engine delivers the notes sorted by start time, so each frame scans one window
    // instead of filtering and sorting the whole piece.
    const all = piece?.notes ?? [];
    const measures = piece?.score.measures ?? [];
    const notes = soloHand
      ? all.filter((note) =>
          soloHand === "Left" ? note.pitch < MIDDLE_C : note.pitch >= MIDDLE_C,
        )
      : all;
    const maxDurationMs = longestNoteMs(notes);

    // Notes are coloured by hand only when hand practice is on: the two hands apart when both
    // show, the one hand's colour when solo. Otherwise each track gets its own palette colour.
    const trackColors = [
      COLORS.track1,
      COLORS.track2,
      COLORS.track3,
      COLORS.track4,
      COLORS.track5,
      COLORS.track6,
    ];
    const noteColor = (note: NoteInfo): string => {
      if (!byHand) {
        return trackColors[note.track % trackColors.length];
      }
      if (soloHand) {
        return soloHand === "Left" ? COLORS.left : COLORS.right;
      }
      return note.pitch < MIDDLE_C ? COLORS.left : COLORS.right;
    };

    const draw = (positionMs: number) => {
      const ratio = window.devicePixelRatio || 1;
      const width = canvas.clientWidth;
      const height = canvas.clientHeight;
      if (canvas.width !== width * ratio || canvas.height !== height * ratio) {
        canvas.width = width * ratio;
        canvas.height = height * ratio;
      }
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height);

      const whites: number[] = [];
      for (let pitch = LOWEST_KEY; pitch <= HIGHEST_KEY; pitch++) {
        if (!BLACK_CLASSES.has(pitch % 12)) {
          whites.push(pitch);
        }
      }
      const whiteIndex = new Map(whites.map((pitch, index) => [pitch, index]));
      const whiteWidth = width / Math.max(1, whites.length);
      const keyRect = (pitch: number): KeyRect => {
        if (!BLACK_CLASSES.has(pitch % 12)) {
          return {
            x: (whiteIndex.get(pitch) ?? 0) * whiteWidth,
            width: whiteWidth,
            black: false,
          };
        }
        const boundary = ((whiteIndex.get(pitch - 1) ?? -1) + 1) * whiteWidth;
        return {
          x: boundary - whiteWidth * 0.35,
          width: whiteWidth * 0.7,
          black: true,
        };
      };

      const keyboardTop = height * (1 - KEYBOARD_SHARE);
      const lookahead = LOOKAHEAD_MS / zoom;
      const pxPerMs = keyboardTop / lookahead || 0.1;
      const sounding = new Set<number>();

      // Background: a warm wash, alternating lane stripes per white key and faint bar lines,
      // so the falling notes read against a grid instead of a flat fill.
      context.fillStyle = COLORS.bg;
      context.fillRect(0, 0, width, height);
      context.fillStyle = COLORS.lane;
      for (let index = 0; index < whites.length; index += 2) {
        context.fillRect(index * whiteWidth, 0, whiteWidth, keyboardTop);
      }
      context.strokeStyle = COLORS.grid;
      context.lineWidth = 1;
      for (const measure of measures) {
        if (measure.start_ms < positionMs) {
          continue;
        }
        const y = keyboardTop - (measure.start_ms - positionMs) * pxPerMs;
        if (y < 0) {
          break;
        }
        if (y > keyboardTop) {
          continue;
        }
        context.beginPath();
        context.moveTo(0, y);
        context.lineTo(width, y);
        context.stroke();
      }

      // First note that can still be on screen: nothing longer than the longest note.
      let windowStart = 0;
      let hi = notes.length;
      const earliest = positionMs - maxDurationMs;
      while (windowStart < hi) {
        const mid = (windowStart + hi) >> 1;
        if (notes[mid].start_ms < earliest) {
          windowStart = mid + 1;
        } else {
          hi = mid;
        }
      }

      const visible: NoteInfo[] = [];
      for (let i = windowStart; i < notes.length; i++) {
        const note = notes[i];
        if (note.start_ms > positionMs + lookahead) {
          break;
        }
        if (note.end_ms <= positionMs) {
          continue;
        }
        if (note.start_ms <= positionMs) {
          sounding.add(note.pitch);
        }
        visible.push(note);
      }

      // Falling notes, drawn furthest first so notes near the keyboard land on top.
      for (let i = visible.length - 1; i >= 0; i--) {
        const note = visible[i];
        const rect = keyRect(note.pitch);
        const bottom = Math.min(
          keyboardTop,
          keyboardTop - (note.start_ms - positionMs) * pxPerMs,
        );
        const top = Math.max(
          0,
          keyboardTop - (note.end_ms - positionMs) * pxPerMs,
        );
        const verdict: Verdict | undefined =
          verdicts[verdictKey(note.pitch, note.start_ms)];
        context.fillStyle =
          verdict === "Correct"
            ? COLORS.correct
            : verdict === "Wrong"
              ? COLORS.wrong
              : noteColor(note);
        context.fillRect(
          rect.x + 1,
          top,
          rect.width - 2,
          Math.max(2, bottom - top),
        );
      }

      // Keyboard: white keys first, black keys over them.
      const isPressed = (pitch: number) => pressed.includes(pitch);
      for (const pitch of whites) {
        const rect = keyRect(pitch);
        context.fillStyle = isPressed(pitch)
          ? COLORS.pressedWhite
          : sounding.has(pitch)
            ? COLORS.soundingKey
            : COLORS.white;
        context.fillRect(rect.x, keyboardTop, rect.width, height - keyboardTop);
        context.strokeStyle = COLORS.keyBorder;
        context.strokeRect(
          rect.x,
          keyboardTop,
          rect.width,
          height - keyboardTop,
        );
      }
      for (let pitch = LOWEST_KEY; pitch <= HIGHEST_KEY; pitch++) {
        if (!BLACK_CLASSES.has(pitch % 12)) {
          continue;
        }
        const rect = keyRect(pitch);
        context.fillStyle = isPressed(pitch)
          ? COLORS.pressedBlack
          : sounding.has(pitch)
            ? COLORS.soundingKey
            : COLORS.black;
        context.fillRect(
          rect.x,
          keyboardTop,
          rect.width,
          (height - keyboardTop) * 0.62,
        );
      }

      context.strokeStyle = COLORS.playhead;
      context.beginPath();
      context.moveTo(0, keyboardTop);
      context.lineTo(width, keyboardTop);
      context.stroke();
    };

    const loop = () => {
      frame = requestAnimationFrame(loop);
      playbackPosition()
        .then(draw)
        .catch(() => {});
    };
    frame = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frame);
  }, [piece, verdicts, pressed, zoom, soloHand, byHand, theme]);

  return (
    <>
      <canvas ref={canvasRef} className="piano-roll" />
      <label className="roll-zoom">
        Zoom
        <input
          type="range"
          min={MIN_ZOOM}
          max={MAX_ZOOM}
          step={0.25}
          value={zoom}
          onChange={(event) => setZoom(Number(event.target.value))}
        />
      </label>
    </>
  );
}
