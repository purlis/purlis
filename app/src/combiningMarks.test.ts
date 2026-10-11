// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { MOST_MARKS, capMarks } from "./combiningMarks";

/**
 * **A run of stacked combining marks is cut to a few** (#1146): the marks a line keeps on each
 * character, and the line said whole where no run is long.
 */

const ACUTE = "́";

describe("capping combining marks", () => {
  it("keeps a few marks on a character, so a macOS path in NFD reads as written", () => {
    const path = "/Users/dev/Tiếng Việt/café.md".normalize("NFD");
    expect(capMarks(path)).toEqual({ text: path, capped: false });
  });

  it("keeps exactly the most a character may carry, and says nothing was cut", () => {
    const four = `e${ACUTE.repeat(4)}`;
    expect(MOST_MARKS).toBe(4);
    expect(capMarks(four)).toEqual({ text: four, capped: false });
  });

  it("cuts a long run to the first few marks, and says it cut", () => {
    expect(capMarks(`ls h${ACUTE.repeat(30)}i`)).toEqual({
      text: `ls h${ACUTE.repeat(4)}i`,
      capped: true,
    });
  });

  it("cuts each run on its own", () => {
    const stacked = `a${"̀".repeat(9)}b̂c${"̈".repeat(6)}`;
    expect(capMarks(stacked).text).toBe(`a${"̀".repeat(4)}b̂c${"̈".repeat(4)}`);
  });

  it("counts enclosing marks, and a mark outside the BMP as one", () => {
    // U+20DD COMBINING ENCLOSING CIRCLE (Me); U+1D167 MUSICAL SYMBOL COMBINING TREMOLO-1 (Mn).
    const run = `x${"⃝".repeat(3)}${"\u{1D167}".repeat(3)}`;
    expect(capMarks(run).text).toBe(`x${"⃝".repeat(3)}\u{1D167}`);
  });

  it("leaves spacing marks alone: they take room of their own and draw over nothing", () => {
    // Devanagari vowel sign I (U+093F) and AA (U+093E) are Mc.
    const spacing = `क${"िा".repeat(4)}`;
    expect(capMarks(spacing)).toEqual({ text: spacing, capped: false });
  });

  it("answers a line with no marks as it is", () => {
    expect(capMarks("Run a command: npm test")).toEqual({
      text: "Run a command: npm test",
      capped: false,
    });
  });
});
