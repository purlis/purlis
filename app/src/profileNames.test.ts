import { afterEach, describe, expect, it } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useProfileNames } from "./profileNames";

/**
 * **The project's profiles, for the palette's rows to their pages** (#1201): read when the
 * palette opens, never on a render of its own, and named as the Settings pages name them.
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
    const { result, rerender } = renderHook(({ open }) => useProfileNames(PLANE, open), {
      initialProps: { open: false },
    });
    expect(result.current).toBeUndefined();
    expect(asked).not.toContain("project_settings");

    rerender({ open: true });

    await waitFor(() => expect(result.current).toEqual(["claude", "fast"]));
  });

  it("reads again each time the palette opens, and not while it stays open", async () => {
    const asked = core([entry("profiles", "claude", "claude")]);
    const { result, rerender } = renderHook(({ open }) => useProfileNames(PLANE, open), {
      initialProps: { open: true },
    });
    await waitFor(() => expect(result.current).toEqual(["claude"]));
    rerender({ open: true });
    rerender({ open: false });
    rerender({ open: true });

    await waitFor(() => expect(asked.filter((one) => one === "project_settings")).toHaveLength(2));
  });

  it("names none from a file the core could not parse", async () => {
    core([entry("profiles", "claude", "claude")], false);
    const { result } = renderHook(() => useProfileNames(PLANE, true));

    await waitFor(() => expect(result.current).toEqual([]));
  });
});
