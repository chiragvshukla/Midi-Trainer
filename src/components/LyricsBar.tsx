import type { LyricAt } from "../lib/api";
import { useSession } from "../state/sessionStore";

/** A shared empty list, so a missing piece does not make a new array each read. */
const NO_LYRICS: LyricAt[] = [];

/** A position inside this many milliseconds is close enough for a text highlight. */
const TICK_MS = 50;

/** One syllable with the time it is sung. */
interface Syllable {
  time_ms: number;
  text: string;
}

/** Karaoke line separators: `\` starts a new line, `/` a new paragraph. */
const LINE_BREAK = /[\\/]/;

/**
 * Groups the syllable stream into lines, splitting anywhere a separator appears, including
 * inside a syllable, since real `.kar` files put them in either place.
 */
export function toLines(lyrics: LyricAt[]): Syllable[][] {
  const lines: Syllable[][] = [];
  let line: Syllable[] = [];
  for (const lyric of lyrics) {
    lyric.text.split(LINE_BREAK).forEach((part, index) => {
      if (index > 0) {
        lines.push(line);
        line = [];
      }
      if (part.length > 0) {
        line.push({ time_ms: lyric.time_ms, text: part });
      }
    });
  }
  if (line.length > 0) {
    lines.push(line);
  }
  return lines;
}

/** Index of the line being sung at `now`, or -1 before the first line starts. */
export function lineAt(lines: Syllable[][], now: number): number {
  let found = -1;
  for (let index = 0; index < lines.length; index += 1) {
    if (lines[index][0].time_ms <= now) {
      found = index;
    } else {
      break;
    }
  }
  return found;
}

/**
 * The karaoke lyric bar: the line being sung, with the syllables up to the playhead lit and the
 * rest dimmed. Rendering only — the lyrics and the position come from the engine.
 */
export function LyricsBar() {
  const lyrics = useSession((state) => state.piece?.lyrics ?? NO_LYRICS);
  // Re-render only as the playhead crosses a 50 ms tick, so the bar follows the music without
  // repainting every animation frame.
  const now = useSession(
    (state) => Math.floor(state.positionMs / TICK_MS) * TICK_MS,
  );

  if (lyrics.length === 0) {
    return null;
  }
  const lines = toLines(lyrics);
  const index = lineAt(lines, now);
  if (index < 0) {
    return null;
  }
  const line = lines[index];
  const sung = line.reduce(
    (last, syllable, at) => (syllable.time_ms <= now ? at : last),
    -1,
  );

  return (
    <div className="lyrics" aria-live="off">
      {line.map((syllable, at) => (
        <span key={at} className={at <= sung ? "sung" : ""}>
          {syllable.text}
        </span>
      ))}
    </div>
  );
}
