import { describe, expect, it } from "vitest";
import { keyName, keyOf } from "./ScoreView";
import type { ChordNote } from "../lib/api";

function note(letter: number, alter: number, pitch: number): ChordNote {
  return {
    letter,
    alter,
    pitch,
    accidental: false,
    onset_ms: 0,
    tied_from_previous: false,
    tied_to_next: false,
  };
}

describe("key signatures", () => {
  it("names major and minor keys by their fifths", () => {
    expect(keyName({ fifths: 2, minor: false })).toBe("D");
    expect(keyName({ fifths: -3, minor: false })).toBe("Eb");
    expect(keyName({ fifths: 0, minor: true })).toBe("Am");
  });
});

describe("note spelling", () => {
  it("writes the letter, accidental and octave", () => {
    expect(keyOf(note(6, -1, 58))).toBe("bb/3"); // B-flat below middle C
    expect(keyOf(note(0, 0, 60))).toBe("c/4"); // middle C
    expect(keyOf(note(6, 0, 59))).toBe("b/3"); // B below middle C
  });
});
