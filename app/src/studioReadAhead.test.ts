// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { READ_AHEAD_FRESH_MS, readStudioAhead, studioOpeningRead } from "./studioReadAhead";

let now = 0;
let reads: unknown[] = [];

beforeEach(() => {
  now = 1_000_000;
  reads = [];
  vi.spyOn(performance, "now").mockImplementation(() => now);
  mockIPC((cmd, args) => {
    if (cmd !== "skill_studio") return null;
    reads.push((args as { workspace: unknown }).workspace);
    return { read: reads.length };
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  clearMocks();
});

/** Which read an answer was: the first the core was asked, the second, … */
const which = async (answer: Promise<unknown>) =>
  ((await answer) as { data: { read: number } }).data.read;

describe("the studio's skills asked for as it is opened (#4351)", () => {
  it("hands the tab the read asked as it was opened, as often as the opening asks", async () => {
    readStudioAhead("/plane", "alpha", null);
    const first = studioOpeningRead("/plane", "alpha", null);
    const again = studioOpeningRead("/plane", "alpha", null);

    expect(await which(first)).toBe(1);
    expect(await which(again)).toBe(1);
    expect(reads).toEqual(["alpha"]);
  });

  it("reads afresh for another scope's skills", async () => {
    readStudioAhead("/plane", "alpha", null);

    expect(await which(studioOpeningRead("/plane", "beta", null))).toBe(2);
    expect(await which(studioOpeningRead("/other", "alpha", null))).toBe(3);
    expect(reads).toEqual(["alpha", "beta", "alpha"]);
  });

  it("reads afresh once the read asked as it was opened is no longer fresh", async () => {
    readStudioAhead("/plane", "alpha", null);
    now += READ_AHEAD_FRESH_MS;

    expect(await which(studioOpeningRead("/plane", "alpha", null))).toBe(2);
  });

  it("reads for itself when nothing was asked as it was opened", async () => {
    expect(await which(studioOpeningRead("/plane", null, "steward"))).toBe(1);
    expect(reads).toEqual([null]);
  });
});
