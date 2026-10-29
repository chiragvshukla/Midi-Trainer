import { describe, expect, it } from "vitest";
import { nearestBarMs, nextBarMs, previousBarMs } from "./sessionStore";
import type { Piece } from "../lib/api";

/** A piece whose measures start at the given times, so the bar maths is easy to read. */
function pieceWithBars(starts: number[], durationMs: number): Piece {
  return {
    notes: [],
    tracks: [],
    lyrics: [],
    recent_files: [],
    duration_ms: durationMs,
    score: {
      key_signature: null,
      measures: starts.map((start_ms) => ({
        start_ms,
        number: 1,
        time_signature: { numerator: 4, denominator: 4 },
        treble: [],
        bass: [],
      })),
    },
  };
}

describe("bar helpers", () => {
  const piece = pieceWithBars([0, 1000, 2000, 3000], 4000);

  it("snaps to the nearest bar line", () => {
    expect(nearestBarMs(piece, 1400)).toBe(1000);
    expect(nearestBarMs(piece, 1600)).toBe(2000);
  });

  it("finds the previous bar line, or the piece's start", () => {
    expect(previousBarMs(piece, 2500)).toBe(2000);
    expect(previousBarMs(piece, 500)).toBe(0);
  });

  it("finds the next bar line, or the piece's end", () => {
    expect(nextBarMs(piece, 2500)).toBe(3000);
    expect(nextBarMs(piece, 3500)).toBe(4000);
  });

  it("falls back sensibly with no piece", () => {
    expect(nearestBarMs(null, 123)).toBe(123);
    expect(previousBarMs(null, 123)).toBe(0);
    expect(nextBarMs(null, 123)).toBe(123);
  });
});
