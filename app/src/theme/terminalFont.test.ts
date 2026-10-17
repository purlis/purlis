// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { drawTerminalFont } from "./terminalFont";

/** A terminal's options that records every family it is set to, as xterm's do. */
function pane(fontFamily: string) {
  const set: string[] = [];
  const options = {
    get fontFamily() {
      return fontFamily;
    },
    set fontFamily(to: string) {
      // xterm measures again only on a change, so a write of the same value is not one.
      if (to !== fontFamily) set.push(to);
      fontFamily = to;
    },
  };
  return { pane: { options }, set };
}

describe("a terminal's family", () => {
  it("is set when it changes", () => {
    const { pane: one, set } = pane("monospace");
    expect(drawTerminalFont(one, '"purlis-terminal-0123", monospace', false)).toBe(true);
    expect(set).toEqual(['"purlis-terminal-0123", monospace']);
  });

  it("is measured again when a face of the same family arrives", () => {
    const family = '"purlis-terminal-0123", monospace';
    const { pane: one, set } = pane(family);
    expect(drawTerminalFont(one, family, true)).toBe(true);
    expect(set).toEqual(["monospace", family]);
    expect(one.options.fontFamily).toBe(family);
  });

  it("is left alone when nothing changed and nothing arrived", () => {
    const { pane: one, set } = pane("monospace");
    expect(drawTerminalFont(one, "monospace", false)).toBe(false);
    expect(set).toEqual([]);
  });
});
