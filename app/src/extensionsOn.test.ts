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

    await waitFor(() => expect(project.result.current).toEqual(new Set(["acme"])));
    expect(asked).toHaveBeenCalledTimes(2);
    expect(unread.result.current).toBeUndefined();
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
