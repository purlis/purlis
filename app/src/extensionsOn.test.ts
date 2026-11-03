// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  EXTENSIONS_ON_ASKED_AGAIN_AFTER,
  extensionsChanged,
  forgetExtensionsOn,
  useExtensionsOn,
  useExtensionsOnUnread,
} from "./extensionsOn";

/**
 * What each project, and each of its workspaces, has on (charter-app#253, #280): asked per
 * project AND workspace, and asked again after a save — including where the last question
 * failed.
 */

afterEach(() => {
  forgetExtensionsOn();
  clearMocks();
});

const PLANE = "/home/dev/plane";

describe("what a workspace has on", () => {
  it("is asked for the project and the workspace, and kept apart from the project's", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "extensions_on") return undefined;
      asked.push(args);
      const { workspace } = args as { workspace: string | null };
      return workspace === "alpha" ? [] : ["stats"];
    });
    const project = renderHook(() => useExtensionsOn(PLANE));
    const alpha = renderHook(() => useExtensionsOn(PLANE, "alpha"));

    await waitFor(() => expect(project.result.current).toEqual(new Set(["stats"])));
    await waitFor(() => expect(alpha.result.current).toEqual(new Set()));
    expect(asked).toEqual([
      { plane: PLANE, workspace: null },
      { plane: PLANE, workspace: "alpha" },
    ]);
  });

  it("is asked again after a save even where the last question failed", async () => {
    let fail = true;
    const asked = vi.fn();
    mockIPC((cmd, args) => {
      if (cmd !== "extensions_on") return undefined;
      asked(args);
      if (fail) throw new Error("could not read");
      return ["stats"];
    });
    const alpha = renderHook(() => useExtensionsOn(PLANE, "alpha"));
    await waitFor(() => expect(asked).toHaveBeenCalledTimes(1));
    expect(alpha.result.current).toBeUndefined();

    fail = false;
    extensionsChanged(PLANE);

    await waitFor(() => expect(alpha.result.current).toEqual(new Set(["stats"])));
    expect(asked).toHaveBeenLastCalledWith({ plane: PLANE, workspace: "alpha" });
  });

  it("asks once more after a question that failed, and keeps that answer (#4822)", async () => {
    vi.useFakeTimers();
    try {
      let failures = 1;
      const asked = vi.fn();
      mockIPC((cmd, args) => {
        if (cmd !== "extensions_on") return undefined;
        asked(args);
        if (failures-- > 0) throw new Error("could not read");
        return ["acme"];
      });
      const project = renderHook(() => useExtensionsOn(PLANE));
      const unread = renderHook(() => useExtensionsOnUnread(PLANE));

      await vi.advanceTimersByTimeAsync(0);
      expect(asked).toHaveBeenCalledTimes(1);
      expect(project.result.current).toBeUndefined();
      await vi.advanceTimersByTimeAsync(EXTENSIONS_ON_ASKED_AGAIN_AFTER);
      expect(asked).toHaveBeenCalledTimes(2);
      expect(project.result.current).toEqual(new Set(["acme"]));
      expect(unread.result.current).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not ask once more when a newer question overtook the one that failed (#4822)", async () => {
    vi.useFakeTimers();
    try {
      let fail = true;
      const asked = vi.fn();
      mockIPC((cmd, args) => {
        if (cmd !== "extensions_on") return undefined;
        asked(args);
        if (fail) throw new Error("could not read");
        return ["acme"];
      });
      const project = renderHook(() => useExtensionsOn(PLANE));
      const unread = renderHook(() => useExtensionsOnUnread(PLANE));
      await vi.advanceTimersByTimeAsync(0);
      expect(asked).toHaveBeenCalledTimes(1);

      // A save while the failed one waits to be asked again: its answer is the one kept.
      fail = false;
      extensionsChanged(PLANE);
      await vi.advanceTimersByTimeAsync(0);
      expect(asked).toHaveBeenCalledTimes(2);
      expect(project.result.current).toEqual(new Set(["acme"]));

      // And the wait ends asking nothing, and taking nothing back.
      fail = true;
      await vi.advanceTimersByTimeAsync(EXTENSIONS_ON_ASKED_AGAIN_AFTER * 2);
      expect(asked).toHaveBeenCalledTimes(2);
      expect(project.result.current).toEqual(new Set(["acme"]));
      expect(unread.result.current).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps a project's failure to that project when the window switches to another (#4822)", async () => {
    vi.useFakeTimers();
    try {
      const OTHER = "/home/dev/other";
      mockIPC((cmd, args) => {
        if (cmd !== "extensions_on") return undefined;
        if ((args as { plane: string }).plane === PLANE) throw new Error("could not read");
        return ["acme"];
      });
      const inFront = renderHook(
        ({ plane }) => ({ on: useExtensionsOn(plane), unread: useExtensionsOnUnread(plane) }),
        { initialProps: { plane: PLANE } },
      );
      await vi.advanceTimersByTimeAsync(0);

      // Switched while the first project's question waits to be asked again.
      inFront.rerender({ plane: OTHER });
      await vi.advanceTimersByTimeAsync(0);
      expect(inFront.result.current.on).toEqual(new Set(["acme"]));

      await vi.advanceTimersByTimeAsync(EXTENSIONS_ON_ASKED_AGAIN_AFTER);
      expect(inFront.result.current.on).toEqual(new Set(["acme"]));
      expect(inFront.result.current.unread).toBeUndefined();
      // The first project's failure is still its own, said when it is in front again.
      inFront.rerender({ plane: PLANE });
      expect(inFront.result.current.unread).toContain("could not read");
    } finally {
      vi.useRealTimers();
    }
  });

  it("says why once the second question failed too, and asks no more until a save (#4822)", async () => {
    vi.useFakeTimers();
    try {
      let fail = true;
      const asked = vi.fn();
      mockIPC((cmd, args) => {
        if (cmd !== "extensions_on") return undefined;
        asked(args);
        if (fail) throw new Error("the record is unreadable");
        return ["acme"];
      });
      const project = renderHook(() => useExtensionsOn(PLANE, "alpha"));
      const unread = renderHook(() => useExtensionsOnUnread(PLANE, "alpha"));

      await vi.advanceTimersByTimeAsync(0);
      expect(asked).toHaveBeenCalledTimes(1);
      expect(unread.result.current).toBeUndefined();
      await vi.advanceTimersByTimeAsync(EXTENSIONS_ON_ASKED_AGAIN_AFTER);
      expect(asked).toHaveBeenCalledTimes(2);
      expect(unread.result.current).toContain("the record is unreadable");
      expect(project.result.current).toBeUndefined();
      await vi.advanceTimersByTimeAsync(EXTENSIONS_ON_ASKED_AGAIN_AFTER * 10);
      expect(asked).toHaveBeenCalledTimes(2);

      fail = false;
      extensionsChanged(PLANE);
      await vi.advanceTimersByTimeAsync(0);
      expect(project.result.current).toEqual(new Set(["acme"]));
      expect(unread.result.current).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });
});
