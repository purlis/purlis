// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  type TabMemoryLook,
  artifactTabBudgetKb,
  artifactTabVerdict,
  tabMemoryLook,
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
    footprintError: null,
    harnessKb: 0,
    harnessLargestKb: 0,
    ...over,
  };
}

const MACOS_KB = artifactTabBudgetKb("darwin");

const BEFORE: TabMemoryLook = { appKb: 300 * MB, webContentKb: 900 * MB, webContentProcesses: 1 };

describe("tabMemoryLook", () => {
  it("counts web content by footprint on macOS, where resident memory reads far low", () => {
    const look = tabMemoryLook(
      200 * MB,
      web({ webContentKb: 80 * MB, webContentFootprintKb: 700 * MB, webContentProcesses: 2 }),
      "darwin",
    );
    expect(look).toEqual({ appKb: 200 * MB, webContentKb: 700 * MB, webContentProcesses: 2 });
  });

  it("counts web content by resident memory on Linux", () => {
    const look = tabMemoryLook(200 * MB, web({ webContentKb: 80 * MB }), "linux");
    expect(look).toEqual({ appKb: 200 * MB, webContentKb: 80 * MB, webContentProcesses: 1 });
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

describe("artifactTabVerdict", () => {
  it("is within the budget when the webview's process and the window's growth fit in it", () => {
    const after = { appKb: 310 * MB, webContentKb: 960 * MB, webContentProcesses: 2 };
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.addedKb).toBe(70 * MB);
    expect(verdict.within).toBe(true);
    expect(verdict.said).toContain("70 MB");
  });

  it("fails an open tab that adds more than the budget, naming both sizes", () => {
    const after = {
      appKb: 300 * MB,
      webContentKb: 900 * MB + MACOS_KB + MB,
      webContentProcesses: 2,
    };
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain(`${MACOS_KB / MB + 1} MB`);
    expect(verdict.said).toContain(`${MACOS_KB / MB} MB`);
  });

  it("holds to the budget it is given", () => {
    const after = { appKb: 300 * MB, webContentKb: 902 * MB, webContentProcesses: 2 };
    expect(artifactTabVerdict(BEFORE, after, 1 * MB).within).toBe(false);
    expect(artifactTabVerdict(BEFORE, after, 2 * MB).within).toBe(true);
  });

  it("fails a page drawn in no web content process of its own, which measured nothing", () => {
    const after = { appKb: 301 * MB, webContentKb: 901 * MB, webContentProcesses: 1 };
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain("no web content process of its own");
  });
});

describe("what a look that never settled says it saw", () => {
  const seen = (processes: number, pids: number[], webMb: number) => ({
    look: { appKb: 100 * MB, webContentKb: webMb * MB, webContentProcesses: processes },
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
