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
  type Base,
  type Chord,
  type ChordNote,
  type Duration,
  type Score,
  type Verdict,
  type Voice,
  type VoiceEvent,
} from "../lib/api";
import { useSession, verdictKey } from "../state/sessionStore";
import { useAppearance } from "../lib/theme";

const LETTER_NAMES = ["c", "d", "e", "f", "g", "a", "b"];

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
export function keyName(signature: { fifths: number; minor: boolean }): string {
  const keys = signature.minor ? MINOR_KEYS : MAJOR_KEYS;
  return keys[Math.min(14, Math.max(0, signature.fifths + 7))];
}

/** VexFlow key for a note's written spelling, e.g. Bb3 -> "bb/3". */
export function keyOf(note: ChordNote): string {
  const accidental = note.alter > 0 ? "#" : note.alter < 0 ? "b" : "";
  const octave = Math.floor((note.pitch - note.alter) / 12) - 1;
  return `${LETTER_NAMES[note.letter]}${accidental}/${octave}`;
}

/** The VexFlow accidental modifier a note needs: sharp, flat, or a natural cancelling the key. */
function accidentalSign(note: ChordNote): string {
  return note.alter > 0 ? "#" : note.alter < 0 ? "b" : "n";
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
 * between the two views. A judged note repaints only its own notehead; the whole sheet is
 * rebuilt only when its layout changes.
 */
export function ScoreView() {
  const hostRef = useRef<HTMLDivElement>(null);
  const sheetRef = useRef<HTMLDivElement>(null);
  const cursorRef = useRef<HTMLDivElement>(null);
  const rangesRef = useRef<MeasureRange[]>([]);
  // Each notehead's rendered note, by verdict key, so a verdict can repaint it without a redraw.
  const noteIndexRef = useRef<Map<string, StaveNote[]>>(new Map());
  // The verdicts already painted, so a change only touches the notes that changed.
  const appliedRef = useRef<Record<string, Verdict>>({});
  // The position of the last frame, so a paused sheet is not yanked back to the cursor and the
  // user can scroll freely; following only resumes once playback moves the cursor again.
  const lastPositionRef = useRef<number | null>(null);
  const piece = useSession((state) => state.piece);
  const verdicts = useSession((state) => state.verdicts);
  const version = useAppearance((state) => state.version);
  // The latest verdicts, so the layout effect can reapply them after a rebuild.
  const verdictsRef = useRef(verdicts);
  verdictsRef.current = verdicts;
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
    noteIndexRef.current = new Map();
    appliedRef.current = {};
    if (!score || score.measures.length === 0) {
      return;
    }
    render(
      score,
      piece.duration_ms,
      zoom,
      width,
      host,
      rangesRef,
      noteIndexRef.current,
    );
    // A fresh sheet carries no verdict marks yet; reapply the ones already known.
    applyVerdicts(
      noteIndexRef.current,
      verdictsRef.current,
      appliedRef.current,
    );
  }, [score, zoom, width, version]);

  useEffect(() => {
    applyVerdicts(noteIndexRef.current, verdicts, appliedRef.current);
  }, [verdicts]);

  useEffect(() => {
    let frame = 0;
    const loop = () => {
      frame = requestAnimationFrame(loop);
      const positionMs = useSession.getState().positionMs;
      const advanced = lastPositionRef.current !== positionMs;
      lastPositionRef.current = positionMs;
      moveCursor(
        positionMs,
        rangesRef.current,
        zoom,
        sheetRef.current,
        cursorRef.current,
        advanced,
      );
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
  zoom: number,
  viewWidth: number,
  host: HTMLDivElement,
  rangesRef: React.RefObject<MeasureRange[]>,
  noteIndex: Map<string, StaveNote[]>,
): void {
  const measures = score.measures;
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
      voiceNotes,
      "treble",
      index,
      noteIndex,
    );
    drawStaff(
      measure.bass,
      measure.number,
      bass,
      context,
      voiceNotes,
      "bass",
      index,
      noteIndex,
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
  voiceNotes: Map<string, DrawnEvent[]>,
  staff: string,
  measureIndex: number,
  noteIndex: Map<string, StaveNote[]>,
): void {
  voices.forEach((voice, voiceIndex) => {
    const drawn = voice.events.map(drawnOf);
    for (const { note, chord } of drawn) {
      for (const chordNote of chord?.notes ?? []) {
        const key = verdictKey(chordNote.pitch, chordNote.onset_ms);
        const notes = noteIndex.get(key) ?? [];
        notes.push(note);
        noteIndex.set(key, notes);
      }
    }

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

function drawnOf(event: VoiceEvent): DrawnEvent {
  const rest = event.type === "Rest";
  const duration = event.duration;
  const chord = rest ? null : event;
  const keys = chord ? chord.notes.map(keyOf) : ["b/4"];
  const note = new StaveNote({ keys, duration: durationText(duration, rest) });
  if (!chord) {
    return { note, duration, chord: null };
  }

  chord.notes.forEach((chordNote, pitchIndex) => {
    if (chordNote.accidental) {
      note.addModifier(new Accidental(accidentalSign(chordNote)), pitchIndex);
    }
  });
  if (duration.type === "Straight" && duration.dotted) {
    Dot.buildAndAttach([note], { all: true });
  }
  return { note, duration, chord };
}

/** Marks a note's rendered SVG by verdict, or clears the mark. The colour itself is CSS. */
function setNoteVerdict(
  notes: StaveNote[] | undefined,
  verdict: Verdict | undefined,
): void {
  const mark =
    verdict === "Correct" ? "correct" : verdict === "Wrong" ? "wrong" : null;
  for (const note of notes ?? []) {
    const element = note.getSVGElement();
    if (!element) {
      continue;
    }
    if (mark) {
      element.setAttribute("data-verdict", mark);
    } else {
      element.removeAttribute("data-verdict");
    }
  }
}

/**
 * Paints only the notes whose verdict changed, so a judged note marks the sheet without
 * rebuilding it. The sheet is rebuilt only when its layout changes; this runs on every verdict.
 */
function applyVerdicts(
  noteIndex: Map<string, StaveNote[]>,
  verdicts: Record<string, Verdict>,
  applied: Record<string, Verdict>,
): void {
  for (const key in verdicts) {
    if (applied[key] !== verdicts[key]) {
      setNoteVerdict(noteIndex.get(key), verdicts[key]);
    }
  }
  for (const key in applied) {
    if (!(key in verdicts)) {
      setNoteVerdict(noteIndex.get(key), undefined);
    }
  }
  for (const key in applied) {
    delete applied[key];
  }
  Object.assign(applied, verdicts);
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

/**
 * Moves the cursor to the playback position. When the position advanced, the active system is
 * kept in view; when it did not (paused or stopped), the scroll is left alone so the user can
 * read elsewhere in the sheet.
 */
function moveCursor(
  positionMs: number,
  ranges: MeasureRange[],
  zoom: number,
  sheet: HTMLDivElement | null,
  cursor: HTMLDivElement | null,
  follow: boolean,
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

  if (!follow) {
    return;
  }

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
