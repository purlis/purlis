// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaneId } from "./bindings";
import { useFailuresSeen } from "./failuresSeen";

const PLANE = "/plane" as PlaneId;
const FAILED = { 1: [{ id: "r-1", task: "build", chat: null }] };

describe("what failed is seen only by a look (#5340)", () => {
  let told: unknown[];
  beforeEach(() => {
    told = [];
    mockIPC((cmd, args) => {
      if (cmd === "task_failure_seen") told.push((args as { id: string }).id);
    });
  });
  afterEach(() => {
    cleanup();
    clearMocks();
    vi.restoreAllMocks();
  });

  it("is seen in the project in front, in a window with the keyboard", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    renderHook(() => useFailuresSeen(PLANE, 1, FAILED, true));
    expect(told).toEqual(["r-1"]);
  });

  it("is not seen in a project kept behind the one in front", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    renderHook(() => useFailuresSeen(PLANE, 1, FAILED, false));
    expect(told).toEqual([]);
  });

  it("is not seen while the window is in the background", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    renderHook(() => useFailuresSeen(PLANE, 1, FAILED, true));
    expect(told).toEqual([]);
  });

  it("is not seen of a chat no pane has in focus", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    renderHook(() => useFailuresSeen(PLANE, 2, FAILED, true));
    expect(told).toEqual([]);
  });
});
