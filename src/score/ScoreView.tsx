import { useEffect, useRef, useState } from "react";
import {
  Accidental,
  Dot,
  Formatter,
  Renderer,
  Stave,
  StaveConnector,
  StaveNote,
  StaveTie,
  Tuplet,
  Voice as VfVoice,
} from "vexflow";
import {
  playbackPosition,
  type Base,
  type Chord,
  type Duration,
  type Score,
  type Voice,
  type VoiceEvent,
} from "../lib/api";
import { useSession, verdictKey } from "../state/sessionStore";
import { scoreColors, useAppearance } from "../lib/theme";

/** The theme-derived colours the sheet draws with. */
type ScoreColors = ReturnType<typeof scoreColors>;

const PITCH_NAMES = [
  "c",
  "c#",
  "d",
  "d#",
  "e",
  "f",
  "f#",
  "g",
  "g#",
  "a",
  "a#",
  "b",
];

const BASE_DURATION: Record<Base, string> = {
  Whole: "w",
  Half: "h",
  Quarter: "q",
  Eighth: "8",
  Sixteenth: "16",
  ThirtySecond: "32",
};

const MEASURE_WIDTH = 340;
const STAFF_GAP = 130;
/** Vertical distance between one system's top and the next system's top. */
const SYSTEM_HEIGHT = 230;
/** Padding around the sheet, in SVG units. */
const MARGIN = 12;

/** VexFlow key names by fifths (index 0 = Cb, index 7 = C, index 14 = C#). */
const MAJOR_KEYS = [
  "Cb",
  "Gb",
  "Db",
  "Ab",
  "Eb",
  "Bb",
  "F",
  "C",
  "G",
  "D",
  "A",
  "E",
  "B",
  "F#",
  "C#",
];
const MINOR_KEYS = [
  "Abm",
  "Ebm",
  "Bbm",
  "Fm",
  "Cm",
  "Gm",
  "Dm",
  "Am",
  "Em",
  "Bm",
  "F#m",
  "C#m",
  "G#m",
  "D#m",
  "A#m",
];

/** The VexFlow name for a key signature, e.g. 2 fifths -> "D". */
function keyName(signature: { fifths: number; minor: boolean }): string {
  const keys = signature.minor ? MINOR_KEYS : MAJOR_KEYS;
  return keys[Math.min(14, Math.max(0, signature.fifths + 7))];
}

/** VexFlow key for a MIDI pitch, e.g. 60 -> "c/4". */
function keyOf(pitch: number): string {
  return `${PITCH_NAMES[pitch % 12]}/${Math.floor(pitch / 12) - 1}`;
}

function durationText(duration: Duration, rest: boolean): string {
  return `${BASE_DURATION[duration.base]}${rest ? "r" : ""}`;
}

/** One drawable event of a staff voice, plus the VexFlow note that renders it. */
interface DrawnEvent {
  note: StaveNote;
  duration: Duration;
  chord: Chord | null;
}

/** Where a measure sits on the rendered sheet, in SVG units. */
interface MeasureRange {
  x: number;
  y: number;
  width: number;
  height: number;
  startMs: number;
  endMs: number;
}

/**
 * The sheet-music view: a thin renderer over the engine's score. Measures wrap into grand-
 * staff systems that fit the window, each starting with clef, key and time signature. A
 * cursor sweeps the score from the playback position, and notes are colored by their
 * playback verdicts. Scroll position and zoom live in the DOM, so they survive toggling
 * between the two views. The whole sheet is redrawn when verdicts change; for very long
 * pieces this redraw is the cost center to optimize first.
 */
export function ScoreView() {
  const hostRef = useRef<HTMLDivElement>(null);
  const sheetRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLDivElement>(null);
  const rangesRef = useRef<MeasureRange[]>([]);
  const piece = useSession((state) => state.piece);
  const verdicts = useSession((state) => state.verdicts);
  const version = useAppearance((state) => state.version);
  const [zoom, setZoom] = useState(1);
  const [width, setWidth] = useState(0);
  const score = piece?.score;

  // The measures-per-system wrap depends on how much width the window gives us.
  useEffect(() => {
    const sheet = sheetRef.current;
    if (!sheet) {
      return;
    }
    const observer = new ResizeObserver(() => setWidth(sheet.clientWidth));
    observer.observe(sheet);
    setWidth(sheet.clientWidth);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    host.replaceChildren();
    rangesRef.current = [];
    if (!score || score.measures.length === 0) {
      return;
    }
    render(score, piece.duration_ms, verdicts, zoom, width, host, rangesRef);
  }, [score, verdicts, zoom, width, version]);

  useEffect(() => {
    let frame = 0;
    const loop = () => {
      frame = requestAnimationFrame(loop);
      playbackPosition()
        .then((positionMs) => {
          moveCursor(
            positionMs,
            rangesRef.current,
            zoom,
            sheetRef.current,
            cursorRef.current,
          );
        })
        .catch(() => {});
    };
    frame = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frame);
  }, [zoom]);

  return (
    <div className="sheet">
      <div className="sheet-tools">
        <button
          type="button"
          onClick={() => setZoom((current) => Math.max(0.5, current - 0.25))}
        >
          −
        </button>
        <span>{Math.round(zoom * 100)}%</span>
        <button
          type="button"
          onClick={() => setZoom((current) => Math.min(3, current + 0.25))}
        >
          +
        </button>
      </div>
      <div className="sheet-scroll" ref={sheetRef}>
        <div className="sheet-cursor" ref={cursorRef} />
        <div ref={hostRef} />
        {!score || score.measures.length === 0 ? (
          <p className="sheet-empty">Nothing to notate in this piece.</p>
        ) : null}
      </div>
    </div>
  );
}

function render(
  score: Score,
  durationMs: number,
  verdicts: Record<string, string>,
  zoom: number,
  viewWidth: number,
  host: HTMLDivElement,
  rangesRef: React.RefObject<MeasureRange[]>,
): void {
  const measures = score.measures;
  const colors = scoreColors();
  // Layout is in SVG units; the context is scaled by zoom, so a CSS pixel of view width
  // is 1/zoom SVG units.
  const available = Math.max(MEASURE_WIDTH, viewWidth / zoom - 2 * MARGIN);
  const perSystem = Math.max(1, Math.floor(available / MEASURE_WIDTH));
  const systems = Math.ceil(measures.length / perSystem);
  const sheetWidth =
    2 * MARGIN + Math.min(perSystem, measures.length) * MEASURE_WIDTH;
  const sheetHeight = 2 * MARGIN + Math.max(1, systems) * SYSTEM_HEIGHT;

  const renderer = new Renderer(host, Renderer.Backends.SVG);
  renderer.resize(Math.ceil(sheetWidth * zoom), Math.ceil(sheetHeight * zoom));
  const context = renderer.getContext();
  context.scale(zoom, zoom);

  const signatureChanged = (index: number): boolean =>
    index > 0 &&
    (measures[index].time_signature.numerator !==
      measures[index - 1].time_signature.numerator ||
      measures[index].time_signature.denominator !==
        measures[index - 1].time_signature.denominator);

  // (staff, measure, voice index) -> drawn events, so ties can cross measure boundaries.
  const voiceNotes = new Map<string, DrawnEvent[]>();
  const ranges: MeasureRange[] = [];

  measures.forEach((measure, index) => {
    const inSystem = index % perSystem;
    const systemTop = MARGIN + Math.floor(index / perSystem) * SYSTEM_HEIGHT;
    const x = MARGIN + inSystem * MEASURE_WIDTH;
    const innerWidth = MEASURE_WIDTH - 20;
    const treble = new Stave(x, systemTop, innerWidth);
    const bass = new Stave(x, systemTop + STAFF_GAP, innerWidth);
    const atSystemStart = inSystem === 0;
    if (atSystemStart) {
      treble.addClef("treble");
      bass.addClef("bass");
      if (score.key_signature) {
        const key = keyName(score.key_signature);
        treble.addKeySignature(key);
        bass.addKeySignature(key);
      }
    }
    if (atSystemStart || signatureChanged(index)) {
      const signature = `${measure.time_signature.numerator}/${measure.time_signature.denominator}`;
      treble.addTimeSignature(signature);
      bass.addTimeSignature(signature);
    }
    treble.setContext(context).draw();
    bass.setContext(context).draw();
    new StaveConnector(treble, bass)
      .setType(StaveConnector.type.SINGLE_LEFT)
      .setContext(context)
      .draw();
    if (atSystemStart) {
      new StaveConnector(treble, bass)
        .setType(StaveConnector.type.BRACE)
        .setContext(context)
        .draw();
    }

    ranges.push({
      x: treble.getX(),
      y: systemTop,
      width: innerWidth,
      height: SYSTEM_HEIGHT,
      startMs: measure.start_ms,
      endMs: measures[index + 1]?.start_ms ?? durationMs,
    });

    drawStaff(
      measure.treble,
      measure.number,
      treble,
      context,
      verdicts,
      voiceNotes,
      "treble",
      index,
      colors,
    );
    drawStaff(
      measure.bass,
      measure.number,
      bass,
      context,
      verdicts,
      voiceNotes,
      "bass",
      index,
      colors,
    );
  });

  drawTies(voiceNotes, context);
  rangesRef.current = ranges;
}

function drawStaff(
  voices: Voice[],
  measureNumber: number,
  stave: Stave,
  context: ReturnType<Renderer["getContext"]>,
  verdicts: Record<string, string>,
  voiceNotes: Map<string, DrawnEvent[]>,
  staff: string,
  measureIndex: number,
  colors: ScoreColors,
): void {
  voices.forEach((voice, voiceIndex) => {
    const drawn = voice.events.map((event) => drawnOf(event, verdicts, colors));

    // Consecutive triplet events of one base value share a tuplet bracket.
    const tuplets: Tuplet[] = [];
    let run: StaveNote[] = [];
    let runBase: Base | null = null;
    for (const { note, duration } of drawn) {
      const isTriplet = duration.type === "Triplet";
      if (isTriplet && duration.base === runBase) {
        run.push(note);
        continue;
      }
      if (run.length > 1) {
        tuplets.push(new Tuplet(run, { ratioed: false }));
      }
      run = isTriplet ? [note] : [];
      runBase = isTriplet ? duration.base : null;
    }
    if (run.length > 1) {
      tuplets.push(new Tuplet(run, { ratioed: false }));
    }

    const vfVoice = new VfVoice({ numBeats: 4, beatValue: 4 })
      .setMode(VfVoice.Mode.SOFT)
      .addTickables(drawn.map(({ note }) => note));
    new Formatter()
      .joinVoices([vfVoice])
      .format([vfVoice], stave.getWidth() - 30);
    vfVoice.draw(context, stave);
    for (const tuplet of tuplets) {
      tuplet.setContext(context).draw();
    }

    voiceNotes.set(`${staff}:${measureIndex}:${voiceIndex}`, drawn);
  });

  if (measureNumber > 0) {
    stave.setMeasure(measureNumber);
  }
}

function drawnOf(
  event: VoiceEvent,
  verdicts: Record<string, string>,
  colors: ScoreColors,
): DrawnEvent {
  const rest = event.type === "Rest";
  const duration = event.duration;
  const chord = rest ? null : event;
  const keys = chord ? chord.notes.map((note) => keyOf(note.pitch)) : ["b/4"];
  const note = new StaveNote({ keys, duration: durationText(duration, rest) });
  if (!chord) {
    return { note, duration, chord: null };
  }

  colorChord(note, chord, verdicts, colors);
  chord.notes.forEach((chordNote, pitchIndex) => {
    if (PITCH_NAMES[chordNote.pitch % 12].includes("#")) {
      note.addModifier(new Accidental("#"), pitchIndex);
    }
  });
  if (duration.type === "Straight" && duration.dotted) {
    Dot.buildAndAttach([note], { all: true });
  }
  return { note, duration, chord };
}

/** Colors a chord's notehead by the verdict of its first matching played note. */
function colorChord(
  note: StaveNote,
  chord: Chord,
  verdicts: Record<string, string>,
  colors: ScoreColors,
): void {
  for (const chordNote of chord.notes) {
    const verdict = verdicts[verdictKey(chordNote.pitch, chordNote.onset_ms)];
    if (verdict === "Correct" || verdict === "Wrong") {
      const color = verdict === "Correct" ? colors.correct : colors.wrong;
      note.setStyle({ fillStyle: color, strokeStyle: color });
      return;
    }
  }
}

/** Draws ties between consecutive chords of a voice, one per shared tied pitch. */
function drawTies(
  voiceNotes: Map<string, DrawnEvent[]>,
  context: ReturnType<Renderer["getContext"]>,
): void {
  for (const [key, events] of voiceNotes) {
    const [staff, measureText, voiceText] = key.split(":");
    for (let index = 0; index < events.length; index++) {
      const current = events[index];
      if (!current.chord) {
        continue;
      }
      const next =
        events[index + 1] ??
        voiceNotes.get(`${staff}:${Number(measureText) + 1}:${voiceText}`)?.[0];
      if (!next?.chord) {
        continue;
      }
      const firstIndexes: number[] = [];
      const lastIndexes: number[] = [];
      current.chord.notes.forEach((chordNote, pitchIndex) => {
        if (!chordNote.tied_to_next) {
          return;
        }
        const target = next.chord!.notes.findIndex(
          (candidate) =>
            candidate.pitch === chordNote.pitch && candidate.tied_from_previous,
        );
        if (target >= 0) {
          firstIndexes.push(pitchIndex);
          lastIndexes.push(target);
        }
      });
      if (firstIndexes.length > 0) {
        new StaveTie({
          firstNote: current.note,
          lastNote: next.note,
          firstIndexes,
          lastIndexes,
        })
          .setContext(context)
          .draw();
      }
    }
  }
}

/** Moves the cursor to the playback position and keeps the active system in view. */
function moveCursor(
  positionMs: number,
  ranges: MeasureRange[],
  zoom: number,
  sheet: HTMLDivElement | null,
  cursor: HTMLDivElement | null,
): void {
  if (!sheet || !cursor || ranges.length === 0) {
    return;
  }
  let active = ranges[ranges.length - 1];
  for (const range of ranges) {
    if (positionMs >= range.startMs) {
      active = range;
    } else {
      break;
    }
  }
  const span = Math.max(1, active.endMs - active.startMs);
  const progress = Math.min(
    1,
    Math.max(0, (positionMs - active.startMs) / span),
  );
  const x = (active.x + progress * active.width) * zoom;
  const top = active.y * zoom;
  const height = active.height * zoom;
  cursor.style.left = `${x}px`;
  cursor.style.top = `${top}px`;
  cursor.style.height = `${height}px`;
  cursor.style.display = "block";

  // Follow the active system vertically; only nudge sideways if it is off-screen.
  const bottom = top + height;
  if (top < sheet.scrollTop || bottom > sheet.scrollTop + sheet.clientHeight) {
    sheet.scrollTop = Math.max(0, top - 20);
  }
  const view = sheet.clientWidth;
  const target = x - view / 3;
  if (x < sheet.scrollLeft || x > sheet.scrollLeft + view) {
    sheet.scrollLeft = Math.max(0, target);
  }
}
