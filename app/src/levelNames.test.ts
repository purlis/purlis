// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vite/client" />
/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { levelNameFaults } from "./copy";
import { SANDBOX_SAID } from "./SandboxMark";
import type { SandboxState } from "./bindings";
import { LEVELS, LEVEL_SAID, projectLevelSaid, yourLevelControls } from "./settings/yourLevel";
import { uiStrings } from "./uiStrings";

/** The three names, strictest first (R-011, Q122). */
const THE_THREE = ["Full sandbox", "Basic sandbox", "No sandbox"];

/** Every source file of the window, as `copy.test.ts` reads it. */
const SOURCES = Object.keys({
  ...import.meta.glob("./**/*.ts"),
  ...import.meta.glob("./**/*.tsx"),
})
  .filter((path) => !/\.test\.tsx?$|^\.\/(bindings|uiRpc)\.ts$/.test(path))
  .map((path) => `src/${path.slice(2)}`)
  .sort();

describe("the sandbox level's names (R-011)", () => {
  it("the level control shows exactly Full sandbox, Basic sandbox and No sandbox", () => {
    // The control: each level the file writes, under its one name.
    for (const control of yourLevelControls(["scribe"]))
      expect(control.choices?.map((level) => control.labels?.[level])).toEqual(THE_THREE);
    expect(LEVELS.map((level) => LEVEL_SAID[level])).toEqual(THE_THREE);
    // The mark on a chat's tab says the same three, as a screen reader hears them.
    expect(Object.values(SANDBOX_SAID).map((said) => said.toLowerCase())).toEqual(
      THE_THREE.map((name) => name.toLowerCase()),
    );
    // The project's own level, as the control's "not set" row says it.
    const states: Partial<SandboxState>[] = [
      { on: true, level_held: null, team_off: null },
      { on: false, level_held: null, team_off: null },
    ];
    for (const state of states)
      expect(THE_THREE.map((name) => name.toLowerCase())).toContain(
        projectLevelSaid(state as SandboxState),
      );
    // And no window string names a level any other way: not `floor`, not a preset's name.
    const faults = SOURCES.flatMap((path) =>
      uiStrings(readFileSync(join(process.cwd(), path), "utf8")).flatMap(({ text, seen, line }) =>
        levelNameFaults(text, seen).map(
          (fault) => `${path}:${line} ${JSON.stringify(text)}: ${fault}`,
        ),
      ),
    );
    expect(faults).toEqual([]);
  });

  it("reads a level's old name in a sentence, and leaves a config key alone", () => {
    expect(levelNameFaults("Chats here start at the floor.", "shown")).toHaveLength(1);
    expect(levelNameFaults("Floor", "source")).toHaveLength(1);
    expect(levelNameFaults(`A chat runs at the floor: ${"…"}`, "source")).toHaveLength(1);
    expect(levelNameFaults("Pick Balanced for most projects.", "shown")).toHaveLength(1);
    expect(levelNameFaults(`Off for you: ${"…"}.`, "source")).toHaveLength(1);
    // A key the file writes, a code span and an id are not names a person reads.
    expect(levelNameFaults("floor", "source")).toEqual([]);
    expect(levelNameFaults('Set `level = "floor"` in the file.', "shown")).toEqual([]);
    expect(levelNameFaults("sandbox.floor", "source")).toEqual([]);
    expect(levelNameFaults("Chats here start at Basic sandbox.", "shown")).toEqual([]);
  });
});
