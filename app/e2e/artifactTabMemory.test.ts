// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  type TabMemoryLook,
  artifactTabBudgetKb,
  artifactTabVerdict,
  tabMemoryLook,
} from "./artifactTabMemory.js";
import type { WebAndHarnessMemory } from "./processes.js";

const MB = 1024;

function web(over: Partial<WebAndHarnessMemory>): WebAndHarnessMemory {
  return {
    webContentKb: 0,
    webContentProcesses: 1,
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

const BEFORE: TabMemoryLook = {
  appKb: 300 * MB,
  webContentKb: 900 * MB,
  webContentProcesses: 1,
  webContentEach: [],
};

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

describe("artifactTabVerdict", () => {
  it("is within the budget when the webview's process and the window's growth fit in it", () => {
    const after = {
      appKb: 310 * MB,
      webContentKb: 960 * MB,
      webContentProcesses: 2,
      webContentEach: [],
    };
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
      webContentEach: [],
    };
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain(`${MACOS_KB / MB + 1} MB`);
    expect(verdict.said).toContain(`${MACOS_KB / MB} MB`);
  });

  it("holds to the budget it is given", () => {
    const after = {
      appKb: 300 * MB,
      webContentKb: 902 * MB,
      webContentProcesses: 2,
      webContentEach: [],
    };
    expect(artifactTabVerdict(BEFORE, after, 1 * MB).within).toBe(false);
    expect(artifactTabVerdict(BEFORE, after, 2 * MB).within).toBe(true);
  });

  it("fails a page drawn in no web content process of its own, which measured nothing", () => {
    const after = {
      appKb: 301 * MB,
      webContentKb: 901 * MB,
      webContentProcesses: 1,
      webContentEach: [],
    };
    const verdict = artifactTabVerdict(BEFORE, after, MACOS_KB);
    expect(verdict.within).toBe(false);
    expect(verdict.said).toContain("no web content process of its own");
  });
});
