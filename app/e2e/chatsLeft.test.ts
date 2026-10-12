// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { leftOver, leftOverSaid, specOf, workspacesInTurn } from "./chatsLeft.js";

describe("the chats a spec file left open", () => {
  it("are the ones running now that were not running when the file began", () => {
    expect(leftOver({ "/p": [1, 2] }, { "/p": [1, 2, 5, 7] })).toEqual([
      { plane: "/p", session: 5 },
      { plane: "/p", session: 7 },
    ]);
  });

  it("count none when the file ended what it started", () => {
    expect(leftOver({ "/p": [1] }, { "/p": [1] })).toEqual([]);
    expect(leftOver({ "/p": [] }, { "/p": [] })).toEqual([]);
  });

  it("are never a chat that was running before the file began and is gone now", () => {
    expect(leftOver({ "/p": [1, 2] }, { "/p": [] })).toEqual([]);
  });

  it("are every chat of a project the file opened and left open", () => {
    expect(leftOver({ "/p": [] }, { "/p": [], "/q": [3, 4] })).toEqual([
      { plane: "/q", session: 3 },
      { plane: "/q", session: 4 },
    ]);
  });

  it("are none of a project the file closed again", () => {
    expect(leftOver({ "/p": [], "/q": [3] }, { "/p": [] })).toEqual([]);
  });
});

describe("what the guard says", () => {
  it("names the spec file and how many chats it left, and says it ended them", () => {
    const said = leftOverSaid("/repo/app/e2e/specs/panes.e2e.ts", [
      { plane: "/p", session: 5 },
      { plane: "/p", session: 7 },
    ]);
    expect(said).toContain("specs/panes.e2e.ts");
    expect(said).toContain("left 2 chats running");
    expect(said).toContain("5, 7");
    expect(said).toMatch(/ended them/);
  });

  it("says one chat in the singular", () => {
    expect(leftOverSaid("specs/one.e2e.ts", [{ plane: "/p", session: 9 }])).toContain(
      "left 1 chat running",
    );
  });
});

describe("the spec file the root hook ran for", () => {
  it("is the file of the first suite under the root", () => {
    expect(specOf({ suites: [{ file: "/x/specs/a.e2e.ts" }] })).toBe("/x/specs/a.e2e.ts");
  });

  it("is said as unknown when mocha names none", () => {
    expect(specOf(undefined)).toBe("(a spec file mocha did not name)");
    expect(specOf({ suites: [] })).toBe("(a spec file mocha did not name)");
  });
});

describe("the workspaces a spec's cleanup looks in", () => {
  it("are the one in front first, then the others left to right", () => {
    expect(
      workspacesInTurn([{ selected: false }, { selected: false }, { selected: true }]),
    ).toEqual([2, 0, 1]);
  });

  it("are every one in order when none is in front", () => {
    expect(workspacesInTurn([{ selected: false }, { selected: false }])).toEqual([0, 1]);
  });

  it("are none on an empty strip", () => {
    expect(workspacesInTurn([])).toEqual([]);
  });
});
