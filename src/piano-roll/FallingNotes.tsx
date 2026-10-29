import { useEffect, useRef } from "react";
import { playbackPosition, type NoteInfo, type Verdict } from "../lib/api";
import { useSession, verdictKey } from "../state/sessionStore";

/** Pitch classes drawn as black keys (C#, D#, F#, G#, A#). */
const BLACK_CLASSES = new Set([1, 3, 6, 8, 10]);

/** How much score time is visible above the keyboard, in milliseconds. */
const LOOKAHEAD_MS = 4000;

/** Fraction of the view the on-screen keyboard takes. */
const KEYBOARD_SHARE = 0.24;

const COLORS = {
  note: "#64b5f6",
  correct: "#43a047",
  wrong: "#e53935",
  soundingKey: "#4fc3f7",
  pressedWhite: "#66bb6a",
  pressedBlack: "#2e7d32",
  white: "#fafafa",
  black: "#222222",
  playhead: "#ff9800",
};

/** Geometry of one piano key within the keyboard strip. */
interface KeyRect {
  x: number;
  width: number;
  black: boolean;
}

/** Pitch range and longest note of a piece, computed once per load. */
interface PieceBounds {
  low: number;
  high: number;
  maxDurationMs: number;
}

function boundsOf(notes: NoteInfo[]): PieceBounds {
  let low = 48;
  let high = 84;
  let maxDurationMs = 0;
  if (notes.length > 0) {
    low = Infinity;
    high = 0;
    for (const note of notes) {
      low = Math.min(low, note.pitch);
      high = Math.max(high, note.pitch);
      maxDurationMs = Math.max(maxDurationMs, note.end_ms - note.start_ms);
    }
    low = Math.max(21, low - 2);
    high = Math.min(108, high + 2);
  }
  return { low, high, maxDurationMs };
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

    // The engine delivers the notes sorted by start time, so each frame scans one window
    // instead of filtering and sorting the whole piece.
    const notes = piece?.notes ?? [];
    const bounds = boundsOf(notes);

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
      for (let pitch = bounds.low; pitch <= bounds.high; pitch++) {
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
      const pxPerMs = keyboardTop / LOOKAHEAD_MS || 0.1;
      const sounding = new Set<number>();

      // First note that can still be on screen: nothing longer than the longest note.
      let windowStart = 0;
      let hi = notes.length;
      const earliest = positionMs - bounds.maxDurationMs;
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
        if (note.start_ms > positionMs + LOOKAHEAD_MS) {
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
              : COLORS.note;
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
        context.strokeStyle = "#cccccc";
        context.strokeRect(
          rect.x,
          keyboardTop,
          rect.width,
          height - keyboardTop,
        );
      }
      for (let pitch = bounds.low; pitch <= bounds.high; pitch++) {
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
  }, [piece, verdicts, pressed]);

  return <canvas ref={canvasRef} className="piano-roll" />;
}
