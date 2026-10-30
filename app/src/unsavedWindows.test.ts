// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **What the main window hands to whom, as work goes between windows** (#4028): the go-between's
 * record (`unsavedWindows.ts`), without windows.
 */
import { describe, expect, it } from "vitest";
import { HandOver } from "./unsavedWindows";
import type { HandedWork } from "./unsavedWork";

function handed(key: string, plane: string): HandedWork {
  return { kind: "skill-edit", key, plane, name: key, data: { key } };
}

const DEPLOY = handed("deploy", "p");
const REVIEW = handed("review", "q");

describe("a project moved to another window", () => {
  it("hands its work to the window that claims it, once", () => {
    const record = new HandOver();
    record.claimed("main", "p");
    expect(record.moving("main", "p", [DEPLOY])).toBeUndefined();
    expect(record.claimed("window-1", "p")).toEqual([DEPLOY]);
    // Nothing is handed twice.
    expect(record.claimed("window-1", "p")).toEqual([]);
  });

  it("hands it at once where the window it went to claimed it first", () => {
    const record = new HandOver();
    record.claimed("main", "p");
    record.claimed("window-1", "p");
    expect(record.moving("main", "p", [DEPLOY])).toEqual({ to: "window-1", works: [DEPLOY] });
    expect(record.claimed("window-1", "p")).toEqual([]);
  });

  it("is not handed back to the window it left", () => {
    const record = new HandOver();
    record.moving("window-1", "p", [DEPLOY]);
    expect(record.claimed("window-1", "p")).toEqual([]);
    expect(record.claimed("main", "p")).toEqual([DEPLOY]);
  });

  it("is what the move carried, not what the window it left last said it held", () => {
    const record = new HandOver();
    const older = { ...DEPLOY, data: { key: "older" } };
    record.mirrored("window-1", [older]);
    record.moving("window-1", "p", [DEPLOY]);
    expect(record.claimed("window-2", "p")).toEqual([DEPLOY]);
    // And the left window's record of it is gone with it.
    expect(record.holding(["main", "window-1", "window-2"])).toEqual([]);
  });
});

describe("a split window closed", () => {
  it("hands what it last said it held to the main window, project by project", () => {
    const record = new HandOver();
    record.mirrored("window-1", [DEPLOY, REVIEW]);
    expect(record.claimed("main", "p")).toEqual([DEPLOY]);
    expect(record.claimed("main", "q")).toEqual([REVIEW]);
    expect(record.claimed("main", "p")).toEqual([]);
  });

  it("hands nothing of a project the window did not hold", () => {
    const record = new HandOver();
    record.mirrored("window-1", [REVIEW]);
    expect(record.claimed("main", "p")).toEqual([]);
  });
});

describe("the windows a quit asks in", () => {
  it("are the split windows still open that last said they hold work", () => {
    const record = new HandOver();
    record.mirrored("window-1", [DEPLOY]);
    record.mirrored("window-2", []);
    record.mirrored("window-3", [REVIEW]);
    expect(record.holding(["main", "window-1", "window-2"])).toEqual(["window-1"]);
    record.mirrored("window-1", []);
    expect(record.holding(["main", "window-1", "window-3"])).toEqual(["window-3"]);
  });
});
