// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Said } from "./actions";
import { nextSaid, type SaidSeen } from "./latestSaid";

/**
 * **The window's line says the newest answer** (#1669): a window sentence said after a project's
 * answer is not hidden by it, and the other way round.
 */

const said = (from: string): Said => ({ from, refused: false, words: from });
const copied = said("file.copy");
const forge = said("repo.forge");
const pinned = said("workspace.pin");

/** Each step's answers in turn, from nothing drawn; what the line draws after each. */
function drawn(steps: [string | undefined, Said | undefined, Said | undefined][]) {
  let seen: SaidSeen | undefined;
  return steps.map(([front, project, window]) => {
    seen = nextSaid(seen, front, project, window);
    return seen.latest?.from;
  });
}

describe("the window's line", () => {
  it("draws the project's answer, or the window's where the project has none", () => {
    expect(drawn([["a", copied, forge]])).toEqual(["file.copy"]);
    expect(drawn([["a", undefined, forge]])).toEqual(["repo.forge"]);
    expect(drawn([[undefined, undefined, undefined]])).toEqual([undefined]);
  });

  it("draws a window sentence said after the project in front had answered", () => {
    expect(
      drawn([
        ["a", copied, undefined],
        ["a", copied, forge],
      ]),
    ).toEqual(["file.copy", "repo.forge"]);
  });

  it("draws the project's next answer over an older window sentence", () => {
    expect(
      drawn([
        ["a", copied, undefined],
        ["a", copied, forge],
        ["a", pinned, forge],
      ]),
    ).toEqual(["file.copy", "repo.forge", "workspace.pin"]);
  });

  it("keeps what it drew while nothing new is said", () => {
    expect(
      drawn([
        ["a", copied, forge],
        ["a", copied, forge],
      ]),
    ).toEqual(["file.copy", "file.copy"]);
  });

  it("draws the project brought to the front's own answer, or the window's", () => {
    expect(
      drawn([
        ["a", undefined, forge],
        ["b", pinned, forge],
        ["c", undefined, forge],
      ]),
    ).toEqual(["repo.forge", "workspace.pin", "repo.forge"]);
  });

  it("goes back to the project's answer when the window's is taken back", () => {
    expect(
      drawn([
        ["a", copied, undefined],
        ["a", copied, forge],
        ["a", copied, undefined],
      ]),
    ).toEqual(["file.copy", "repo.forge", "file.copy"]);
  });

  it("draws a window sentence said as another project comes to the front", () => {
    expect(
      drawn([
        ["a", copied, undefined],
        ["b", pinned, forge],
      ]),
    ).toEqual(["file.copy", "repo.forge"]);
  });
});
