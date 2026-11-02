// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Frame } from "../windowprefs";
import { PARTS_ASKED_AGAIN_AFTER, partsLand } from "./partsLater";
import { DEFAULT_THEME, drawIn, inForce } from "./theme";

const PARTS = "@layer parts {}";

/** A heavy theme's first frame: drawn with no parts, and `parts_later` names how to ask. */
function aFrame(parts_later: number | null = 7): Frame {
  return {
    light: false,
    more: false,
    theme: { ...DEFAULT_THEME, name: "Heavy", parts: "" },
    floors: null,
    mine: [],
    parts_later,
  };
}

/** Answers the ask with each of `answers` in turn, the last for ever: a string is the parts, and
 *  an `Error` is thrown. Answers the tickets it was asked with. */
function theCoreAnswers(...answers: readonly (string | Error)[]): number[] {
  const asked: number[] = [];
  mockIPC((cmd, args) => {
    if (cmd !== "first_frame_parts") return null;
    asked.push((args as { ticket: number }).ticket);
    const answer = answers[Math.min(asked.length, answers.length) - 1];
    if (answer instanceof Error) throw answer;
    return answer;
  });
  return asked;
}

describe("a first frame's parts land whatever the restore does (#4410)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    clearMocks();
    drawIn(DEFAULT_THEME);
  });

  it("asks for them at once, and draws the frame with them", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    const asked = theCoreAnswers(PARTS);

    await partsLand(frame);

    expect(asked).toEqual([7]);
    expect(inForce()).toEqual({ ...frame.theme, parts: PARTS });
  });

  it("asks nothing for a frame that has its parts", async () => {
    const frame = aFrame(null);
    drawIn(frame.theme);
    const asked = theCoreAnswers(PARTS);

    await partsLand(frame);

    expect(asked).toEqual([]);
    expect(inForce()).toBe(frame.theme);
  });

  it("asks again after an ask that failed", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    const asked = theCoreAnswers(new Error("the core did not answer"), PARTS);

    const landing = partsLand(frame);
    await vi.runAllTimersAsync();
    await landing;

    expect(asked).toEqual([7, 7]);
    expect(inForce().parts).toBe(PARTS);
  });

  it("asks again after an answer that is a refusal", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    let asks = 0;
    mockIPC((cmd) => {
      if (cmd !== "first_frame_parts") return null;
      asks += 1;
      if (asks === 1) return Promise.reject("not yet");
      return PARTS;
    });

    const landing = partsLand(frame);
    await vi.runAllTimersAsync();
    await landing;

    expect(asks).toBe(2);
    expect(inForce().parts).toBe(PARTS);
  });

  it("stops after its last ask, and the frame stays as it is drawn", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    const asked = theCoreAnswers(new Error("the core did not answer"));

    const landing = partsLand(frame);
    await vi.runAllTimersAsync();
    await landing;

    expect(asked).toHaveLength(1 + PARTS_ASKED_AGAIN_AFTER.length);
    expect(inForce()).toBe(frame.theme);
  });

  it("draws nothing when another theme was drawn while it asked", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    const other = { ...DEFAULT_THEME, name: "The project's" };
    mockIPC((cmd) => {
      if (cmd !== "first_frame_parts") return null;
      drawIn(other);
      return PARTS;
    });

    await partsLand(frame);

    expect(inForce()).toBe(other);
  });

  it("asks no more once another theme is drawn", async () => {
    const frame = aFrame();
    drawIn(frame.theme);
    const other = { ...DEFAULT_THEME, name: "The project's" };
    const asked = theCoreAnswers(new Error("the core did not answer"));

    const landing = partsLand(frame);
    await vi.advanceTimersByTimeAsync(0);
    drawIn(other);
    await vi.runAllTimersAsync();
    await landing;

    expect(asked).toHaveLength(1);
    expect(inForce()).toBe(other);
  });
});
