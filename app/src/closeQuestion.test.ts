// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import type { PlaneId } from "./bindings";
import { useCloseQuestion } from "./closeQuestion";

/**
 * **A project is marked as asking while a close question is up, and only then** (#2845): the
 * mark a project's view gives the window (`onAsking`), read by a link's close (#2761). A close
 * that settles with no render in between must still clear it.
 */

afterEach(cleanup);

const PLANE = "/home/dev/plane" as unknown as PlaneId;

/** The window's side of the mark: the projects marked as asking now. */
function window() {
  const marked = new Set<PlaneId>();
  const onAsking = (plane: PlaneId, up: boolean) => {
    if (up) marked.add(plane);
    else marked.delete(plane);
  };
  return { marked, onAsking };
}

describe("a project's close question mark", () => {
  it("is cleared by a close that settles with no render in between", async () => {
    const { marked, onAsking } = window();
    const { result } = renderHook(() => useCloseQuestion(PLANE, false, onAsking));
    await act(() => result.current.counted(async () => "closed"));
    expect(result.current.up).toBe(false);
    expect(marked.has(PLANE)).toBe(false);
  });

  it("is cleared by a quiet close under StrictMode too", async () => {
    const { marked, onAsking } = window();
    const { result } = renderHook(() => useCloseQuestion(PLANE, false, onAsking), {
      wrapper: StrictMode,
    });
    expect(marked.has(PLANE)).toBe(false);
    await act(() => result.current.counted(async () => "closed"));
    expect(result.current.up).toBe(false);
    expect(marked.has(PLANE)).toBe(false);
  });

  it("is cleared by a close that throws, which still throws", async () => {
    const { marked, onAsking } = window();
    const { result } = renderHook(() => useCloseQuestion(PLANE, false, onAsking));
    await act(async () => {
      await expect(
        result.current.counted(async () => {
          throw new Error("the core refused");
        }),
      ).rejects.toThrow("the core refused");
    });
    expect(result.current.up).toBe(false);
    expect(marked.has(PLANE)).toBe(false);
  });

  it("is up from a close's first step, before anything renders", async () => {
    const { marked, onAsking } = window();
    const { result } = renderHook(() => useCloseQuestion(PLANE, false, onAsking));
    let settle = () => {};
    const held = new Promise<void>((resolve) => (settle = resolve));
    let closing: Promise<unknown> = Promise.resolve();
    act(() => {
      closing = result.current.counted(() => held);
      expect(marked.has(PLANE)).toBe(true);
    });
    expect(result.current.up).toBe(true);
    await act(async () => {
      settle();
      await closing;
    });
    expect(marked.has(PLANE)).toBe(false);
  });

  it("stays up while a question the close drew is on screen", async () => {
    const { marked, onAsking } = window();
    let drawn = false;
    const { result, rerender } = renderHook(() => useCloseQuestion(PLANE, drawn, onAsking));
    await act(() =>
      result.current.counted(async () => {
        drawn = true;
      }),
    );
    rerender();
    expect(result.current.up).toBe(true);
    expect(marked.has(PLANE)).toBe(true);
    drawn = false;
    rerender();
    expect(marked.has(PLANE)).toBe(false);
  });

  it("stays up while another close is still being asked about", async () => {
    const { marked, onAsking } = window();
    const { result } = renderHook(() => useCloseQuestion(PLANE, false, onAsking));
    let settle = () => {};
    const held = new Promise<void>((resolve) => (settle = resolve));
    let slow: Promise<unknown> = Promise.resolve();
    act(() => {
      slow = result.current.counted(() => held);
    });
    await act(() => result.current.counted(async () => "closed"));
    expect(marked.has(PLANE)).toBe(true);
    await act(async () => {
      settle();
      await slow;
    });
    expect(marked.has(PLANE)).toBe(false);
  });

  it("is cleared when the project's view goes", () => {
    const { marked, onAsking } = window();
    const { unmount } = renderHook(() => useCloseQuestion(PLANE, true, onAsking));
    expect(marked.has(PLANE)).toBe(true);
    unmount();
    expect(marked.has(PLANE)).toBe(false);
  });
});
