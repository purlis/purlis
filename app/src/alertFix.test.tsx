// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetAlertFixes, useAlertFix } from "./alertFix";

/**
 * **An alert's fix, one per project and fix** (#3930's review): the Inbox's row and a workspace
 * chip's card press the same fix, so they share what it is doing and what it answered, and never
 * run it twice at once. Another project's answer is never said on this one's line.
 */

afterEach(() => {
  clearMocks();
  forgetAlertFixes();
});

/** A core whose fix waits until `land` is called, counting each ask. */
function held() {
  let land: (answer: unknown) => void = () => undefined;
  let asked = 0;
  mockIPC((cmd) => {
    if (cmd !== "plane_doctor_fix") return null;
    asked += 1;
    return new Promise((done) => {
      land = done;
    });
  });
  return { land: (answer: unknown) => land(answer), asked: () => asked };
}

const LABEL = "Update workspace layout";

describe("an alert's fix, pressed from two places", () => {
  it("is busy in both while it runs, and runs once", async () => {
    const core = held();
    const reread = vi.fn();
    const row = renderHook(() => useAlertFix("/p/a", "workspace-reinit", LABEL, reread));
    const chip = renderHook(() => useAlertFix("/p/a", "workspace-reinit", LABEL, reread));
    act(() => chip.result.current.apply("workspace-reinit"));
    await waitFor(() => expect(row.result.current.busy).toBe(true));
    // The second press, from the Inbox's row while the chip's runs: nothing more is sent.
    act(() => row.result.current.apply("workspace-reinit"));
    expect(core.asked()).toBe(1);
    act(() =>
      core.land({
        fix: "workspace-reinit",
        refused: "a workspace is locked",
        said: [],
        complete: false,
      }),
    );
    await waitFor(() => expect(row.result.current.busy).toBe(false));
    // What it answered is said in both places.
    expect(row.result.current.said).toBe(
      "purlis could not update workspace layout: a workspace is locked",
    );
    expect(chip.result.current.said).toBe(row.result.current.said);
    expect(reread).toHaveBeenCalled();
  });

  it("says nothing on another project's line", async () => {
    const core = held();
    const here = renderHook(() => useAlertFix("/p/a", "workspace-reinit", LABEL, () => undefined));
    act(() => here.result.current.apply("workspace-reinit"));
    act(() => core.land({ fix: "workspace-reinit", refused: "locked", said: [], complete: false }));
    await waitFor(() => expect(here.result.current.said).toBeDefined());
    const there = renderHook(() => useAlertFix("/p/b", "workspace-reinit", LABEL, () => undefined));
    expect(there.result.current.said).toBeUndefined();
    expect(there.result.current.busy).toBe(false);
  });
});
