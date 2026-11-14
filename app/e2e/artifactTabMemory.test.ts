// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  type TabMemoryLook,
  artifactTabBudgetKb,
  artifactTabVerdict,
  tabMemoryLook,
  pageProcesses,
  tabSteady,
  unsettledSaid,
} from "./artifactTabMemory.js";
import type { WebAndHarnessMemory } from "./processes.js";

const MB = 1024;

function web(over: Partial<WebAndHarnessMemory>): WebAndHarnessMemory {
  return {
    webContentKb: 0,
    webContentProcesses: 1,
    webContentPids: [1],
    webContentFootprintKb: null,
    webContentFootprintPeakKb: null,
    webContentEach: [],
    footprintError: null,
    harnessKb: 0,
    harnessLargestKb: 0,
    ...over,
  };
}

const MACOS_KB = artifactTabBudgetKb("darwin");

describe("tabMemoryLook", () => {
  it("counts web content by footprint on macOS, where resident memory reads far low", () => {
    const look = tabMemoryLook(
      200 * MB,
      web({ webContentKb: 80 * MB, webContentFootprintKb: 700 * MB, webContentProcesses: 2 }),
      "darwin",
    );
    expect(look).toEqual({
      appKb: 200 * MB,
      webContentKb: 700 * MB,
      webContentProcesses: 2,
      webContentEach: [],
    });
  });

  it("counts web content by resident memory on Linux", () => {
    const look = tabMemoryLook(200 * MB, web({ webContentKb: 80 * MB }), "linux");
    expect(look).toEqual({
      appKb: 200 * MB,
      webContentKb: 80 * MB,
      webContentProcesses: 1,
      webContentEach: [],
    });
  });

  it("refuses a macOS look footprint gave no reading for, in footprint's own words", () => {
    expect(() =>
      tabMemoryLook(
        200 * MB,
        web({ webContentKb: 80 * MB, footprintError: "footprint: no such pid '42'" }),
        "darwin",
      ),
    ).toThrow("footprint: no such pid '42'");
  });
});

describe("artifactTabBudgetKb", () => {
  it("holds macOS to 192 MB, the row's initial value", () => {
    expect(artifactTabBudgetKb("darwin")).toBe(192 * MB);
  });

  it("holds Linux to 264 MB: its most measured, 238 MB, with about a tenth more", () => {
    expect(artifactTabBudgetKb("linux")).toBe(264 * MB);
  });
});

/** A look of the app at `appMb`, with web content processes `each` as [pid, MB]. */
function lookOf(appMb: number, each: [number, number][]): TabMemoryLook {
  return {
    appKb: appMb * MB,
    webContentKb: each.reduce((sum, [, mb]) => sum + mb * MB, 0),
    webContentProcesses: each.length,
    webContentEach: each.map(([pid, mb]) => ({ pid, kb: mb * MB })),
  };
}

/** The window's web content (pid 10) and a process an earlier page left (pid 20). */
const BEFORE = lookOf(300, [
  [10, 880],
  [20, 20],
]);

describe("artifactTabVerdict", () => {
  it("is within the budget when the webview's process and the window's growth fit in it", () => {
    const after = lookOf(310, [
      [10, 890],
      [20, 20],
      [30, 50],
    ]);
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.addedKb).toBe(70 * MB);
    expect(verdict.processesAdded).toBe(1);
    expect(verdict.within).toBe(true);
    expect(verdict.said).toContain("70 MB");
  });

  it("fails an open tab that adds more than the budget, naming both sizes", () => {
    const after = lookOf(300, [
      [10, 880],
      [20, 20],
      [30, MACOS_KB / MB + 1],
    ]);
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain(`${MACOS_KB / MB + 1} MB`);
    expect(verdict.said).toContain(`${MACOS_KB / MB} MB`);
  });

  it("holds to the budget it is given", () => {
    const after = lookOf(300, [
      [10, 880],
      [20, 20],
      [30, 2],
    ]);
    expect(artifactTabVerdict(BEFORE, after, 1 * MB).within).toBe(false);
    expect(artifactTabVerdict(BEFORE, after, 2 * MB).within).toBe(true);
  });

  // #6589: an earlier page's process WebKit ends while the tab opens is neither the tab's
  // cost nor a saving of it.
  it("counts the page's new process even when an earlier page's process exited meanwhile", () => {
    const after = lookOf(300, [
      [10, 880],
      [30, 16],
    ]);
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.processesAdded).toBe(1);
    expect(verdict.webContentAddedKb).toBe(16 * MB);
    expect(verdict.within).toBe(true);
    expect(verdict.said).toContain("pid 20 exited");
  });

  it("fails a page drawn in no web content process of its own, which measured nothing", () => {
    const after = lookOf(301, [
      [10, 881],
      [20, 20],
    ]);
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain("no web content process of its own");
  });

  it("takes a new pid as the page's even where the count held, an earlier one having exited", () => {
    const after = lookOf(301, [
      [10, 881],
      [21, 20],
    ]);
    // pid 21 is new, so it is the tab's: the count alone would have said nothing was added.
    expect(artifactTabVerdict(BEFORE, after, MACOS_KB).processesAdded).toBe(1);
  });
});

describe("pageProcesses", () => {
  it("is every web content process a look holds that the look before the tab did not", () => {
    const seen = lookOf(300, [
      [10, 880],
      [30, 16],
    ]);
    expect(pageProcesses(BEFORE, seen)).toEqual([{ pid: 30, kb: 16 * MB }]);
    expect(pageProcesses(BEFORE, BEFORE)).toEqual([]);
  });
});

describe("tabSteady", () => {
  const SETTLED = 2 * MB;

  it("holds two looks of the same processes whose app and page agree within the margin", () => {
    const one = lookOf(300, [
      [10, 880],
      [30, 16],
    ]);
    const two = lookOf(301, [
      [10, 880],
      [30, 17],
    ]);
    expect(tabSteady(one, two, BEFORE, SETTLED)).toBe(true);
  });

  // #6589: the window's own web content, the app's interface, moved 3 to 5 MB between looks a
  // second apart in a quarter of the loop's pairs (#6592) with nothing opened or closed.
  it("leaves out the window's own web content, which the tab does not move", () => {
    const one = lookOf(300, [
      [10, 880],
      [30, 16],
    ]);
    const two = lookOf(300, [
      [10, 885],
      [30, 16],
    ]);
    expect(tabSteady(one, two, BEFORE, SETTLED)).toBe(true);
  });

  it("refuses a page still growing", () => {
    const one = lookOf(300, [[30, 16]]);
    const two = lookOf(300, [[30, 19]]);
    expect(tabSteady(one, two, BEFORE, SETTLED)).toBe(false);
  });

  it("refuses an app still growing", () => {
    const one = lookOf(300, [[30, 16]]);
    const two = lookOf(303, [[30, 16]]);
    expect(tabSteady(one, two, BEFORE, SETTLED)).toBe(false);
  });

  it("refuses two looks of different processes: one came or went between them", () => {
    const one = lookOf(300, [
      [10, 880],
      [20, 20],
    ]);
    const two = lookOf(300, [[10, 880]]);
    expect(tabSteady(one, two, null, SETTLED)).toBe(false);
  });

  it("before the tab, holds the app alone to the margin", () => {
    const one = lookOf(300, [[10, 880]]);
    expect(tabSteady(one, lookOf(301, [[10, 884]]), null, SETTLED)).toBe(true);
    expect(tabSteady(one, lookOf(303, [[10, 880]]), null, SETTLED)).toBe(false);
  });
});

describe("what a look that never settled says it saw", () => {
  const seen = (processes: number, pids: number[], webMb: number) => ({
    look: {
      appKb: 100 * MB,
      webContentKb: webMb * MB,
      webContentProcesses: processes,
      webContentEach: [],
    },
    pids,
  });

  it("names the look before the tab and each of the last looks, by pid and memory", () => {
    const said = unsettledSaid(seen(2, [11, 12], 200), [
      seen(2, [11, 13], 220),
      { missed: "no footprint of the web content: footprint gave no reading" },
      seen(2, [11, 13], 221),
    ]);
    expect(said).toBe(
      "before the tab opened: 2 web content processes (pid 11, 12), 300 MB; " +
        "its last 3 looks, a second apart: 2 web content processes (pid 11, 13), 320 MB; " +
        "failed: no footprint of the web content: footprint gave no reading; " +
        "2 web content processes (pid 11, 13), 321 MB",
    );
  });

  it("names only the last five looks", () => {
    const looks = [1, 2, 3, 4, 5, 6, 7].map((n) => seen(1, [n], n));
    const said = unsettledSaid(null, looks);
    expect(
      said.startsWith("its last 5 looks, a second apart: 1 web content processes (pid 3)"),
    ).toBe(true);
    expect(said).not.toContain("pid 2)");
  });

  it("says no look was taken when none was", () => {
    expect(unsettledSaid(null, [])).toBe("no look was taken");
  });
});
