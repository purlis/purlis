// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { notClonedRows } from "./NotCloned";
import type { CloneState } from "./repoClones";

const cloning = (state: CloneState) => new Map([["coming", state]]);

describe("the repos drawn as not cloned here", () => {
  it("are what the workspace names, with nothing cloning", () => {
    expect(notClonedRows(["named"], ["here"])).toEqual(["named"]);
  });

  it("include a repo cloning that the workspace does not name yet", () => {
    expect(notClonedRows(["named"], [], cloning({ state: "cloning", percent: 5 }))).toEqual([
      "named",
      "coming",
    ]);
    expect(notClonedRows([], [], cloning({ state: "waiting" }))).toEqual(["coming"]);
  });

  it("never draw a repo twice, or one that has already landed", () => {
    const now = cloning({ state: "cloning" });
    expect(notClonedRows(["coming"], [], now)).toEqual(["coming"]);
    expect(notClonedRows([], ["coming"], now)).toEqual([]);
  });

  it("leave out a clone that has ended", () => {
    expect(notClonedRows([], [], cloning({ state: "stopped" }))).toEqual([]);
    expect(notClonedRows([], [], cloning({ state: "failed", said: "no" }))).toEqual([]);
  });
});
