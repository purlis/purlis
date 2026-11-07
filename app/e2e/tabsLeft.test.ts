// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  arrivedAt,
  samePlace,
  type Strip,
  stripsToJudge,
  tabsLeftOver,
  tabsLeftSaid,
} from "./tabsLeft.js";

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

  it("are a second unnamed tab, past the strip's kept empty one (#5586)", () => {
    expect(tabsLeftOver(strip([]), strip(["New chat", "New chat"]))).toEqual(["New chat"]);
    expect(tabsLeftOver(strip(["New chat"]), strip(["New chat", "steward 1", "New chat"]))).toEqual(
      ["steward 1", "New chat"],
    );
    expect(tabsLeftOver(strip(["New chat", "New chat"]), strip(["New chat", "New chat"]))).toEqual(
      [],
    );
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

describe("the strips a spec file visited (#5269)", () => {
  it("are written down once each, with what each held when the file first came to it", () => {
    let seen: Strip[] = [];
    seen = arrivedAt(seen, strip(["steward 1"]));
    seen = arrivedAt(seen, strip(["New chat"], "/p", "alpha"));
    seen = arrivedAt(seen, strip(["steward 1", "Settings"]));
    expect(seen).toEqual([strip(["steward 1"]), strip(["New chat"], "/p", "alpha")]);
  });

  it("judge a strip the file only passed through against what it held when the file came to it", () => {
    const began = strip(["steward 1"]);
    const passed = strip(["Activity"], "/p", "alpha");
    const ended = strip([], "/p", "beta");
    expect(stripsToJudge(began, ended, [began, passed, ended])).toEqual([ended, passed, began]);
  });

  it("judge the strip it ended on first, then every other it visited, then the one it began on", () => {
    const began = strip([]);
    const visits = [strip([], "/p", "a"), strip([], "/q", "b"), strip([], "/p", "c")];
    const ended = visits[2];
    expect(stripsToJudge(began, ended, [began, ...visits])).toEqual([
      ended,
      visits[0],
      visits[1],
      began,
    ]);
  });

  it("judge the strip it began on against what it held as the file began, once, when it ended there", () => {
    const began = strip(["steward 1"]);
    const passed = strip([], "/p", "alpha");
    expect(stripsToJudge(began, strip(["steward 1", "Settings"]), [began, passed])).toEqual([
      began,
      passed,
    ]);
  });

  it("judge a strip it ended on but never saw arrive at as holding nothing of its own", () => {
    const ended = strip(["New chat", "Settings"], "/p", "alpha");
    expect(stripsToJudge(strip([]), ended, [])).toEqual([{ ...ended, tabs: [] }, strip([])]);
  });
});
