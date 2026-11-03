// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useProfileNames } from "./profileNames";

/**
 * **The project's profiles, for the palette's rows to their pages** (#1201): read when the
 * palette opens over this project, never on a render of its own, and named as the Settings pages
 * name them. What the hook is told is how many times the palette has opened, not whether it is
 * open: its closing changes nothing the project's view is drawn from (#4351).
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

const entry = (collection: string, label: string, name?: string) => ({
  collection,
  id: `${collection}:${label}`,
  label,
  keys: [],
  values: name === undefined ? [] : [{ field: "name", value: name }],
});

function core(entries: unknown[], parsed = true) {
  const asked: string[] = [];
  mockIPC((cmd) => {
    asked.push(cmd);
    if (cmd === "project_settings")
      return {
        shared: { which: "shared", parsed: true, entries: [] },
        local: { which: "local", parsed, entries },
      };
    return null;
  });
  return asked;
}

describe("the project's profile names", () => {
  it("reads nothing until the palette opens, then names each profile the Local file lists", async () => {
    const asked = core([
      entry("profiles", "claude", "claude"),
      entry("myHosts", "api.example.com"),
      entry("profiles", "codex fast", "fast"),
    ]);
    const { result, rerender } = renderHook(
      ({ openings }) => useProfileNames(PLANE, openings, true),
      { initialProps: { openings: 0 } },
    );
    expect(result.current).toBeUndefined();
    expect(asked).not.toContain("project_settings");

    rerender({ openings: 1 });

    await waitFor(() => expect(result.current).toEqual(["claude", "fast"]));
  });

  it("reads again each time the palette opens, and not while it stays open", async () => {
    const asked = core([entry("profiles", "claude", "claude")]);
    const { result, rerender } = renderHook(
      ({ openings }) => useProfileNames(PLANE, openings, true),
      { initialProps: { openings: 1 } },
    );
    await waitFor(() => expect(result.current).toEqual(["claude"]));
    rerender({ openings: 1 });
    rerender({ openings: 2 });

    await waitFor(() => expect(asked.filter((one) => one === "project_settings")).toHaveLength(2));
  });

  it("reads nothing for an opening over another project, nor as this one comes to front", async () => {
    const asked = core([entry("profiles", "claude", "claude")]);
    const { rerender } = renderHook(
      ({ openings, here }) => useProfileNames(PLANE, openings, here),
      { initialProps: { openings: 0, here: false } },
    );
    rerender({ openings: 1, here: false });
    rerender({ openings: 1, here: true });
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
    expect(asked).not.toContain("project_settings");

    rerender({ openings: 2, here: true });
    await waitFor(() => expect(asked.filter((one) => one === "project_settings")).toHaveLength(1));
  });

  it("keeps the very list when an opening reads the same names again", async () => {
    const asked = core([entry("profiles", "claude", "claude")]);
    const { result, rerender } = renderHook(
      ({ openings }) => useProfileNames(PLANE, openings, true),
      { initialProps: { openings: 1 } },
    );
    await waitFor(() => expect(result.current).toEqual(["claude"]));
    const first = result.current;
    rerender({ openings: 2 });
    await waitFor(() => expect(asked.filter((one) => one === "project_settings")).toHaveLength(2));
    // The second read has answered once the core has been asked: let its answer land.
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

    expect(result.current).toBe(first);
  });

  it("names none from a file the core could not parse", async () => {
    core([entry("profiles", "claude", "claude")], false);
    const { result } = renderHook(() => useProfileNames(PLANE, 1, true));

    await waitFor(() => expect(result.current).toEqual([]));
  });
});
