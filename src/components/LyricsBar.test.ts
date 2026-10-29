import { describe, expect, it } from "vitest";
import { lineAt, toLines } from "./LyricsBar";

describe("lyric line splitting", () => {
  it("breaks lines on backslash and slash, even inside a syllable", () => {
    const lines = toLines([
      { time_ms: 0, text: "Hello " },
      { time_ms: 500, text: "world\\next " },
      { time_ms: 1000, text: "line/para" },
    ]);

    expect(lines.map((line) => line.map((s) => s.text).join(""))).toEqual([
      "Hello world",
      "next line",
      "para",
    ]);
    // The syllable that opened a new line keeps its own time.
    expect(lines[1][0].time_ms).toBe(500);
  });

  it("finds the line being sung, or none before the first", () => {
    const lines = toLines([
      { time_ms: 0, text: "one\\" },
      { time_ms: 1000, text: "two" },
    ]);

    expect(lineAt(lines, -1)).toBe(-1);
    expect(lineAt(lines, 0)).toBe(0);
    expect(lineAt(lines, 999)).toBe(0);
    expect(lineAt(lines, 1000)).toBe(1);
  });
});
