// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { commands } from "./bindings";
import {
  LATE_MS,
  WAITS_BY_DESIGN,
  forgetLateAnswers,
  saidOfLate,
  timed,
  useLateAnswers,
} from "./answerWatch";

/**
 * **A command the core has not answered for a while is written down, and said** (`answerWatch.ts`,
 * #1949): a dialog that waits on an answer that never comes is no longer a silent nothing.
 */

afterEach(() => {
  vi.useRealTimers();
  act(() => forgetLateAnswers());
});

describe("an answer", () => {
  it("still not come past the bound is written down, and its time once it comes", async () => {
    vi.useFakeTimers();
    let answer: (said: string) => void = () => {};
    const ask = timed("createProject", () => new Promise<string>((said) => (answer = said)));
    const { result } = renderHook(() => useLateAnswers());

    const asked = ask("/home/dev/made");
    act(() => vi.advanceTimersByTime(LATE_MS - 1));
    expect(result.current).toEqual([]);
    act(() => vi.advanceTimersByTime(1));
    expect(result.current.map((one) => [one.command, one.ms])).toEqual([
      ["createProject", undefined],
    ]);

    await act(async () => {
      vi.advanceTimersByTime(5_000);
      answer("made");
      await asked;
    });
    expect(result.current[0].ms).toBeGreaterThanOrEqual(LATE_MS);
  });

  it("that comes in time is not written down at all", async () => {
    vi.useFakeTimers();
    const ask = timed("sandboxState", () => Promise.resolve("ok"));
    const { result } = renderHook(() => useLateAnswers());
    await act(async () => {
      await ask("/p");
      vi.advanceTimersByTime(LATE_MS * 2);
    });
    expect(result.current).toEqual([]);
  });

  it("is said in words that name the command and that it may still come", () => {
    expect(saidOfLate({ command: "createProject", at: 0, ms: undefined })).toBe(
      "purlis has not answered the window for 20 s (createProject). It may still be working; " +
        "what you asked for appears when it answers.",
    );
  });
});

describe("the commands that wait by design", () => {
  it("are each one the bindings have", () => {
    const asks = commands as unknown as Record<string, unknown>;
    for (const name of WAITS_BY_DESIGN) expect(typeof asks[name], name).toBe("function");
  });
});
