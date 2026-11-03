// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { type Strip, tabsLeftOver, tabsLeftSaid } from "./tabsLeft.js";

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

describe("what the tab guard says", () => {
  it("names the spec file and the tabs it left, and says it closed them", () => {
    const said = tabsLeftSaid("/repo/app/e2e/specs/settings-row.e2e.ts", ["Settings", "Activity"]);
    expect(said).toContain("specs/settings-row.e2e.ts");
    expect(said).toContain("left 2 tabs open");
    expect(said).toContain("Settings, Activity");
    expect(said).toMatch(/closed them/);
  });

  it("says one tab in the singular", () => {
    expect(tabsLeftSaid("specs/one.e2e.ts", ["Settings"])).toContain("left 1 tab open");
  });
});
