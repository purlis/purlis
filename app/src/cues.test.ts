// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * **A cue no theme can erase** (#3295): the `cues` layer, after `parts`, which no theme writes
 * into. Its first cue was the close that ended a chat; a close ends nothing since #4756, so no
 * close wears the danger colour anywhere, under a theme or not.
 */

/** A stylesheet's name, in parts: the stylesheet guard reads a quoted path ending in the
 *  extension as an import. */
const sheet = (name: string) => `${name}.${"css"}`;

const read = (file: string) =>
  readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");

const LAYERS = /@layer ([a-z, ]+);/;

describe("the cues layer (#3295)", () => {
  it("is drawn in a layer after the theme's parts", () => {
    const styles = read(sheet("styles"));
    const order = (LAYERS.exec(styles)?.[1] ?? "").split(/,\s*/);
    expect(order).toContain("cues");
    expect(order.indexOf("cues")).toBeGreaterThan(order.indexOf("parts"));
    expect(styles).toContain(`@import "./${sheet("cues")}" layer(cues);`);
  });
});

describe("a close (#4756)", () => {
  it("wears no danger cue, since it ends nothing", () => {
    for (const file of [
      sheet("cues"),
      sheet("paneFrame"),
      sheet("App"),
      `ui/tabs/${sheet("tabs")}`,
    ])
      expect(read(file), file).not.toMatch(/ends-a-chat/);
  });
});
