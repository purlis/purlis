// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { samePlace, type Strip, tabsLeftOver, tabsLeftSaid } from "./tabsLeft.js";

const strip = (tabs: string[], project = "/p", workspace = ""): Strip => ({
  project,
  workspace,
  tabs,
});

describe("the tabs a spec file left open", () => {
  it("are the ones on the strip now that were not on it when the file began", () => {
    expect(
      tabsLeftOver(strip(["steward 1"]), strip(["steward 1", "Settings", "Activity"])),
    ).toEqual(["Settings", "Activity"]);
  });

  it("count none when the file closed what it opened", () => {
    expect(tabsLeftOver(strip(["steward 1"]), strip(["steward 1"]))).toEqual([]);
    expect(tabsLeftOver(strip([]), strip([]))).toEqual([]);
  });

  it("are never a tab that was there before the file began and is gone now", () => {
    expect(tabsLeftOver(strip(["Settings", "steward 1"]), strip([]))).toEqual([]);
  });

  it("count a second tab of a name the strip already had", () => {
    expect(tabsLeftOver(strip(["Settings"]), strip(["Settings", "Settings"]))).toEqual([
      "Settings",
    ]);
  });

  it("are never the strip's kept empty tab (#4756)", () => {
    expect(tabsLeftOver(strip(["Settings"]), strip(["New chat"]))).toEqual([]);
  });

  it("are none when another project or workspace is in front: that strip says nothing of this one", () => {
    expect(tabsLeftOver(strip([]), strip(["Settings"], "/q"))).toEqual([]);
    expect(tabsLeftOver(strip([]), strip(["Settings"], "/p", "talk"))).toEqual([]);
  });
});

describe("where a spec file ended", () => {
  it("is the strip it began on when its project and workspace are the same, whatever its tabs", () => {
    expect(samePlace(strip(["a"]), strip(["b"]))).toBe(true);
  });

  it("is another strip when another project or workspace is in front", () => {
    expect(samePlace(strip([]), strip([], "/q"))).toBe(false);
    expect(samePlace(strip([]), strip([], "/p", "talk"))).toBe(false);
    expect(samePlace(strip([], "/p", "talk"), strip([]))).toBe(false);
  });

  it("judges the strip a file ended on, away from where it began, as if it held nothing", () => {
    const ended = strip(["New chat", "Settings"], "/p", "alpha");
    expect(tabsLeftOver({ ...ended, tabs: [] }, ended)).toEqual(["Settings"]);
  });

  it("says nothing of the project when the rail did not draw one", () => {
    expect(samePlace(strip([]), strip([], ""))).toBe(true);
    expect(samePlace(strip([], ""), strip([], "/p"))).toBe(true);
    expect(samePlace(strip([], ""), strip([], "", "talk"))).toBe(false);
  });
});

describe("what the tab guard says", () => {
  it("names the spec file and the tabs it left, and says it closed them", () => {
    const said = tabsLeftSaid(
      "/repo/app/e2e/specs/settings-row.e2e.ts",
      ["Settings", "Activity"],
      strip([], "/p", "alpha"),
    );
    expect(said).toContain("specs/settings-row.e2e.ts");
    expect(said).toContain("left 2 tabs open on /p, alpha");
    expect(said).toContain("Settings, Activity");
    expect(said).toMatch(/closed them/);
  });

  it("says one tab in the singular", () => {
    expect(tabsLeftSaid("specs/one.e2e.ts", ["Settings"], strip([]))).toContain("left 1 tab open");
  });
});
