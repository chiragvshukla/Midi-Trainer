import { useEffect, useRef, useState } from "react";
import { type NoteInfo, type Verdict } from "../lib/api";
import { pitchName } from "../lib/pitch";
import { canvasColors, useAppearance } from "../lib/theme";
import { useSession, verdictKey, WRONG_FLASH_MS } from "../state/sessionStore";

/** Pitch classes drawn as black keys (C#, D#, F#, G#, A#). */
const BLACK_CLASSES = new Set([1, 3, 6, 8, 10]);

/** The font the note-name labels are drawn in. */
const LABEL_FONT = '10px "Cascadia Mono", Consolas, monospace';

/** How much score time is visible above the keyboard at zoom 1, in milliseconds. */
const LOOKAHEAD_MS = 4000;

/** How long a note lingers at the keyboard after it ends. A skipped note is judged once the
 * playhead passes its start by the matcher's window (±250 ms), so a short note needs a tail
 * this long to still be on screen when its verdict arrives. */
const NOTE_LINGER_MS = 300;

/** A minimum time gap before the next note in the same lane, so close notes never touch. */
const NOTE_GAP_MS = 20;

/** A minimum pixel gap before the next note in the same lane, so the separation stays visible
 * however far the view is zoomed out. */
const NOTE_GAP_PX = 2;

/** A note is never drawn shorter than this, so even a short note reads as a bar. */
const MIN_NOTE_HEIGHT = 4;

/** Small gutter between a lane's edge and its note, so neighbouring columns stay distinct. */
const NOTE_INSET = 1;

/** Corner radius of a falling note, so its edges are rounded rather than square. */
const NOTE_CORNER_RADIUS = 3;

/** Middle C: the boundary between the hands, as the engine splits them. */
const MIDDLE_C = 60;

/** Zoom limits: below 1 fits more time in, above 1 spreads it out. */
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 4;

/** Fraction of the view the on-screen keyboard takes. */
const KEYBOARD_SHARE = 0.24;

/** A black key's height as a fraction of the keyboard strip. */
const BLACK_KEY_SHARE = 0.62;

/** How far a held key sinks, to read as a physical press. */
const KEY_PRESS_DEPTH = 2;

/** Rounded corner radius of a key's front edge. */
const KEY_CORNER_RADIUS = 2;

/** The felt strip along the back of the keys. */
const FELT_COLOR = "#7a1f1f";

/** Height of the soft shadow the keyboard casts onto the falling notes. */
const KEYBOARD_SHADOW_PX = 14;

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

/** The soft black/white alpha ramps that give the keys depth. They are palette-independent, so
 * they only need rebuilding when the canvas size changes, never per frame. */
interface KeyShading {
  whiteBack: CanvasGradient;
  blackGloss: CanvasGradient;
  keyboardShadow: CanvasGradient;
}

function buildShading(
  context: CanvasRenderingContext2D,
  keyboardTop: number,
  height: number,
): KeyShading {
  const blackBottom = keyboardTop + (height - keyboardTop) * BLACK_KEY_SHARE;
  const whiteBack = context.createLinearGradient(0, keyboardTop, 0, height);
  whiteBack.addColorStop(0, "rgba(0, 0, 0, 0.18)");
  whiteBack.addColorStop(0.45, "rgba(0, 0, 0, 0)");
  whiteBack.addColorStop(1, "rgba(0, 0, 0, 0.06)");
  const blackGloss = context.createLinearGradient(
    0,
    keyboardTop,
    0,
    blackBottom,
  );
  blackGloss.addColorStop(0, "rgba(255, 255, 255, 0.30)");
  blackGloss.addColorStop(0.35, "rgba(255, 255, 255, 0)");
  blackGloss.addColorStop(1, "rgba(0, 0, 0, 0.30)");
  const keyboardShadow = context.createLinearGradient(
    0,
    keyboardTop - KEYBOARD_SHADOW_PX,
    0,
    keyboardTop,
  );
  keyboardShadow.addColorStop(0, "rgba(0, 0, 0, 0)");
  keyboardShadow.addColorStop(1, "rgba(0, 0, 0, 0.38)");
  return { whiteBack, blackGloss, keyboardShadow };
}

/** A rectangle rounded only along its front (bottom) edge, clamped so the radius always fits. */
function roundBottomEdge(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
): void {
  const r = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.roundRect(x, y, width, height, [0, 0, r, r]);
}

/** A note's path: a rectangle rounded on all four corners, clamped so the radius always fits. */
function roundedNotePath(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
): void {
  const r = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.roundRect(x, y, width, height, r);
}

/**
 * The falling-note view: score notes scroll onto an on-screen keyboard, keys light up while
 * their notes sound or while the player holds them, and judged notes are marked by verdict.
 * Rendering only — position comes from the engine, verdicts from the session store.
 */
export function FallingNotes() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const piece = useSession((state) => state.piece);
  const settings = useSession((state) => state.settings);
  const version = useAppearance((state) => state.version);
  const noteLabels = useAppearance((state) => state.noteLabels);
  const noteBackdrop = useAppearance((state) => state.noteBackdrop);
  const pianoLook = useAppearance((state) => state.pianoLook);
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

    // Realistic piano look only: key shading, rebuilt only when the canvas size changes.
    const realistic = pianoLook === "realistic";
    let shading: { key: string; data: KeyShading } | null = null;

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
    // For each note, the start of the next note in the same lane (Infinity if it is the last),
    // so a note's drawn tail can be capped before the next one begins.
    const nextStart = new Array<number>(notes.length).fill(
      Number.POSITIVE_INFINITY,
    );
    const lastAtPitch = new Map<number, number>();
    for (let i = notes.length - 1; i >= 0; i--) {
      const pitch = notes[i].pitch;
      const later = lastAtPitch.get(pitch);
      if (later !== undefined) {
        nextStart[i] = notes[later].start_ms;
      }
      lastAtPitch.set(pitch, i);
    }
    const labelKeys = noteLabels === "keys" || noteLabels === "both";
    const labelNotes = noteLabels === "notes" || noteLabels === "both";

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
      // Verdicts, held keys and flashes change every poll tick; read them live so those changes
      // never rebuild this effect and re-run the colour lookup.
      const { verdicts, pressed, wrongFlashes } = useSession.getState();
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
      const shadingKey = `${width}x${height}`;
      if (realistic && shading?.key !== shadingKey) {
        shading = {
          key: shadingKey,
          data: buildShading(context, keyboardTop, height),
        };
      }
      const shade = realistic ? shading!.data : null;
      const lookahead = LOOKAHEAD_MS / zoom;
      const pxPerMs = keyboardTop / lookahead || 0.1;
      const sounding = new Set<number>();

      // Background: a warm wash. When the backdrop is on, alternate lane stripes per white key
      // and faint bar lines give the falling notes a grid to read against.
      context.fillStyle = COLORS.bg;
      context.fillRect(0, 0, width, height);
      if (noteBackdrop) {
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

      // Kept as indices so a note can look up the next note in its own lane.
      const visible: number[] = [];
      for (let i = windowStart; i < notes.length; i++) {
        const note = notes[i];
        if (note.start_ms > positionMs + lookahead) {
          break;
        }
        if (note.end_ms + NOTE_LINGER_MS <= positionMs) {
          continue;
        }
        if (note.start_ms <= positionMs) {
          sounding.add(note.pitch);
        }
        visible.push(i);
      }

      // Falling notes, drawn furthest first so notes near the keyboard land on top.
      for (let v = visible.length - 1; v >= 0; v--) {
        const i = visible[v];
        const note = notes[i];
        const rect = keyRect(note.pitch);
        // The linger tail is capped at the next note in the same lane, so two close notes never paint
        // over each other; a lane's last note keeps the full linger for its verdict. The cap may
        // shorten a legato note (whose end meets the next start) — that is the point.
        const cappedEndMs = Math.min(
          note.end_ms + NOTE_LINGER_MS,
          nextStart[i] - NOTE_GAP_MS,
        );
        if (cappedEndMs <= positionMs) {
          continue;
        }
        const bottom = Math.min(
          keyboardTop,
          keyboardTop - (note.start_ms - positionMs) * pxPerMs,
        );
        let top = Math.max(
          0,
          keyboardTop - (cappedEndMs - positionMs) * pxPerMs,
        );
        // Keep a visible gap to the next note in the lane at any zoom, in pixels.
        if (Number.isFinite(nextStart[i])) {
          const nextBottom = Math.min(
            keyboardTop,
            keyboardTop - (nextStart[i] - positionMs) * pxPerMs,
          );
          if (nextBottom > 0 && nextBottom < keyboardTop) {
            top = Math.max(top, nextBottom + NOTE_GAP_PX);
          }
        }
        const verdict: Verdict | undefined =
          verdicts[verdictKey(note.pitch, note.start_ms)];
        // The track colour always fills the note; a verdict is shown by shape (outline and, for
        // a wrong note, a cross), never by hue, so it stays distinct from a track that is itself
        // red or green and from any custom track colour.
        const noteX = rect.x + NOTE_INSET;
        const noteWidth = rect.width - NOTE_INSET * 2;
        const noteHeight = Math.max(MIN_NOTE_HEIGHT, bottom - top);
        const radius = Math.min(
          NOTE_CORNER_RADIUS,
          noteWidth / 2,
          noteHeight / 2,
        );
        context.fillStyle = noteColor(note);
        if (verdict === "Wrong") {
          context.globalAlpha = 0.45;
        }
        roundedNotePath(context, noteX, top, noteWidth, noteHeight, radius);
        context.fill();
        context.globalAlpha = 1;
        // Every note keeps a thin border, so crowded notes still read apart; a verdict then
        // overdraws it in the correct/wrong colour.
        context.strokeStyle = COLORS.keyBorder;
        context.lineWidth = 1;
        roundedNotePath(
          context,
          noteX + 0.5,
          top + 0.5,
          noteWidth - 1,
          noteHeight - 1,
          radius,
        );
        context.stroke();
        // The note the playhead is sitting on is the one to play now: ring it and glow it, so it
        // stands out from the notes yet to come. A judged note is never "required".
        if (
          verdict === undefined &&
          note.start_ms <= positionMs &&
          cappedEndMs >= positionMs
        ) {
          context.save();
          context.strokeStyle = COLORS.playhead;
          context.shadowColor = COLORS.playhead;
          context.shadowBlur = 8;
          context.lineWidth = 2;
          roundedNotePath(
            context,
            noteX + 1,
            top + 1,
            noteWidth - 2,
            noteHeight - 2,
            radius,
          );
          context.stroke();
          context.restore();
        }
        if (verdict === "Correct" || verdict === "Wrong") {
          context.strokeStyle =
            verdict === "Correct" ? COLORS.correct : COLORS.wrong;
          context.lineWidth = 2;
          roundedNotePath(
            context,
            noteX + 1,
            top + 1,
            noteWidth - 2,
            noteHeight - 2,
            radius,
          );
          context.stroke();
          if (verdict === "Wrong") {
            context.beginPath();
            context.moveTo(noteX, top);
            context.lineTo(noteX + noteWidth, top + noteHeight);
            context.moveTo(noteX + noteWidth, top);
            context.lineTo(noteX, top + noteHeight);
            context.stroke();
          }
        }
        if (labelNotes && bottom - top > 12 && rect.width > 12) {
          context.fillStyle = COLORS.label;
          context.font = LABEL_FONT;
          context.textAlign = "center";
          context.textBaseline = "middle";
          context.fillText(
            pitchName(note.pitch),
            rect.x + rect.width / 2,
            (top + bottom) / 2,
          );
        }
      }

      // Keyboard: white keys first, black keys over them. The realistic look gathers its
      // depth from shading ramps rather than separate borders.
      const isPressed = (pitch: number) => pressed.includes(pitch);
      // A key just played wrong flashes red, overriding its normal colour until the flash ages.
      const now = performance.now();
      const wrongKeys = new Set(
        wrongFlashes
          .filter((flash) => now - flash.at < WRONG_FLASH_MS)
          .map((flash) => flash.pitch),
      );
      if (shade) {
        context.fillStyle = shade.keyboardShadow;
        context.fillRect(
          0,
          keyboardTop - KEYBOARD_SHADOW_PX,
          width,
          KEYBOARD_SHADOW_PX,
        );
        context.fillStyle = FELT_COLOR;
        context.fillRect(0, keyboardTop - 3, width, 3);
      }
      for (const pitch of whites) {
        const rect = keyRect(pitch);
        context.fillStyle = wrongKeys.has(pitch)
          ? COLORS.wrong
          : isPressed(pitch)
            ? COLORS.pressedWhite
            : sounding.has(pitch)
              ? COLORS.soundingKey
              : COLORS.white;
        if (shade) {
          const top = keyboardTop + (isPressed(pitch) ? KEY_PRESS_DEPTH : 0);
          const keyHeight = height - top;
          roundBottomEdge(
            context,
            rect.x,
            top,
            rect.width,
            keyHeight,
            KEY_CORNER_RADIUS,
          );
          context.fill();
          context.fillStyle = shade.whiteBack;
          roundBottomEdge(
            context,
            rect.x,
            top,
            rect.width,
            keyHeight,
            KEY_CORNER_RADIUS,
          );
          context.fill();
          // Just a separator between keys; the shading supplies the rest of the depth.
          context.strokeStyle = COLORS.keyBorder;
          context.lineWidth = 1;
          context.beginPath();
          context.moveTo(rect.x + rect.width, top);
          context.lineTo(rect.x + rect.width, height);
          context.stroke();
          continue;
        }
        context.fillRect(rect.x, keyboardTop, rect.width, height - keyboardTop);
        context.strokeStyle = COLORS.keyBorder;
        context.strokeRect(
          rect.x,
          keyboardTop,
          rect.width,
          height - keyboardTop,
        );
      }
      const blackHeight = (height - keyboardTop) * BLACK_KEY_SHARE;
      for (let pitch = LOWEST_KEY; pitch <= HIGHEST_KEY; pitch++) {
        if (!BLACK_CLASSES.has(pitch % 12)) {
          continue;
        }
        const rect = keyRect(pitch);
        context.fillStyle = wrongKeys.has(pitch)
          ? COLORS.wrong
          : isPressed(pitch)
            ? COLORS.pressedBlack
            : sounding.has(pitch)
              ? COLORS.soundingKey
              : COLORS.black;
        if (shade) {
          const lit =
            wrongKeys.has(pitch) || isPressed(pitch) || sounding.has(pitch);
          const top = keyboardTop + (isPressed(pitch) ? KEY_PRESS_DEPTH : 0);
          roundBottomEdge(
            context,
            rect.x,
            top,
            rect.width,
            blackHeight,
            KEY_CORNER_RADIUS,
          );
          context.fill();
          // The gloss is skipped on a lit key so its pressed/sounding colour reads clearly.
          if (!lit) {
            context.fillStyle = shade.blackGloss;
            roundBottomEdge(
              context,
              rect.x,
              top,
              rect.width,
              blackHeight,
              KEY_CORNER_RADIUS,
            );
            context.fill();
          }
          // A thin shadow to the right casts the black key onto the white keys.
          context.fillStyle = "rgba(0, 0, 0, 0.25)";
          context.fillRect(rect.x + rect.width, top, 1, blackHeight);
          continue;
        }
        context.fillRect(
          rect.x,
          keyboardTop,
          rect.width,
          (height - keyboardTop) * 0.62,
        );
      }

      // Note names on the keyboard, when the labels are on. Every white key when they are wide
      // enough, otherwise just the Cs, so the names never vanish entirely on a narrow window.
      if (labelKeys) {
        const labelEveryKey = whiteWidth >= 14;
        context.fillStyle = COLORS.black;
        context.font = LABEL_FONT;
        context.textAlign = "center";
        context.textBaseline = "bottom";
        for (const pitch of whites) {
          if (!labelEveryKey && pitch % 12 !== 0) {
            continue;
          }
          const rect = keyRect(pitch);
          context.fillText(
            pitchName(pitch),
            rect.x + rect.width / 2,
            height - 4,
          );
        }
      }

      context.strokeStyle = COLORS.playhead;
      context.beginPath();
      context.moveTo(0, keyboardTop);
      context.lineTo(width, keyboardTop);
      context.stroke();
    };

    const loop = () => {
      frame = requestAnimationFrame(loop);
      draw(useSession.getState().positionMs);
    };
    frame = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frame);
  }, [
    piece,
    zoom,
    soloHand,
    byHand,
    version,
    noteLabels,
    noteBackdrop,
    pianoLook,
  ]);

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
