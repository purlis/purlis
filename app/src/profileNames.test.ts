// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useProfileNames } from "./profileNames";

/**
 * **The projects' profiles, for the palette's rows to their pages** (#1201): read for a project
 * when the palette opens over it, never on a render of its own, and named as the Settings pages
 * name them. **Held by the window, a list per project, the same list while the names are**
 * (#4351): the palette opening or closing changes nothing a project's view is drawn from, and a
 * project with no profiles is given no list at all.
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
  mockIPC((cmd, args) => {
    asked.push(`${cmd} ${String((args as { plane?: string } | undefined)?.plane)}`);
    if (cmd === "project_settings")
      return {
        shared: { which: "shared", parsed: true, entries: [] },
        local: { which: "local", parsed, entries },
      };
    return null;
  });
  return asked;
}

describe("the projects' profile names", () => {
  it("reads nothing until asked, then names each profile the Local file lists", async () => {
    const asked = core([
      entry("profiles", "claude", "claude"),
      entry("myHosts", "api.example.com"),
      entry("profiles", "codex fast", "fast"),
    ]);
    const { result } = renderHook(() => useProfileNames());
    expect(result.current.of[PLANE]).toBeUndefined();
    expect(asked).toEqual([]);

    act(() => result.current.read(PLANE));

    await waitFor(() => expect(result.current.of[PLANE]).toEqual(["claude", "fast"]));
    expect(asked).toEqual([`project_settings ${PLANE}`]);
  });

  it("keeps the very lists when a read finds the same names", async () => {
    const asked = core([entry("profiles", "claude", "claude")]);
    const { result } = renderHook(() => useProfileNames());
    act(() => result.current.read(PLANE));
    await waitFor(() => expect(result.current.of[PLANE]).toEqual(["claude"]));
    const first = result.current.of;

    act(() => result.current.read(PLANE));
    await waitFor(() => expect(asked).toHaveLength(2));
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

    expect(result.current.of).toBe(first);
  });

  it("gives a project with no profiles no list, so reading it changes nothing", async () => {
    const asked = core([entry("myHosts", "api.example.com")]);
    const { result } = renderHook(() => useProfileNames());
    const before = result.current.of;

    act(() => result.current.read(PLANE));
    await waitFor(() => expect(asked).toHaveLength(1));
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

    expect(result.current.of).toBe(before);
    expect(result.current.of[PLANE]).toBeUndefined();
  });

  it("names none from a file the core could not parse", async () => {
    const asked = core([entry("profiles", "claude", "claude")], false);
    const { result } = renderHook(() => useProfileNames());

    act(() => result.current.read(PLANE));
    await waitFor(() => expect(asked).toHaveLength(1));
    await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

    expect(result.current.of[PLANE]).toBeUndefined();
  });
});
