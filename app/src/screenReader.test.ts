// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { forgetThisLaunch } from "./regions";
import {
  forgetScreenReaderMode,
  loadScreenReaderMode,
  onScreenReaderMode,
  screenReaderMode,
  setScreenReaderMode,
  useScreenReaderMode,
} from "./screenReader";
import {
  aboutThisMachine,
  GLOBAL,
  sayAboutThisMachine,
  usingTheDefaultLayout,
} from "./windowprefs";

const PATH = "/home/op/.config/purlis/layout.json";

const handed = (document: unknown) => {
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: PATH, found: true, document, trouble: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
};

let sent: { cmd: string; args: unknown }[] = [];
const written = () =>
  sent
    .filter((one) => one.cmd === "write_layout")
    .map((one) => JSON.parse((one.args as { text: string }).text) as Record<string, unknown>);

beforeEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  forgetThisLaunch();
  sayAboutThisMachine("screenReader", undefined);
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    return null;
  });
});
afterEach(() => {
  clearMocks();
  forgetScreenReaderMode();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

describe("screen-reader mode, kept in the layout file (#629)", () => {
  it("is followed by a render: turned on or off, what reads it is drawn again", () => {
    handed({ version: 2, regions: [] });
    const { result } = renderHook(() => useScreenReaderMode());
    expect(result.current).toBe(false);

    act(() => setScreenReaderMode(true));
    expect(result.current).toBe(true);
    act(() => setScreenReaderMode(false));
    expect(result.current).toBe(false);
  });

  it("is off where the file says nothing of it", () => {
    expect(loadScreenReaderMode({ version: 2, regions: [] })).toEqual({ on: false, said: [] });
    handed({ version: 2, regions: [] });

    expect(screenReaderMode()).toBe(false);
  });

  it("starts on when the file the window was handed says it is on", () => {
    handed({ version: 2, regions: [], screenReader: true });

    expect(screenReaderMode()).toBe(true);
  });

  it("is off when the file holds something else there, and the Inbox says where", () => {
    handed({ version: 2, regions: [], screenReader: "yes" });

    expect(screenReaderMode()).toBe(false);
    const [said] = aboutThisMachine();
    expect(said.subject).toBe("screenReader");
    expect(said.detail).toContain(PATH);
    expect(said.detail).toContain('"yes"');
  });

  it("is written to the file when it is turned on, and left out once it is off again", async () => {
    setScreenReaderMode(true);

    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].screenReader).toBe(true);

    setScreenReaderMode(false);

    await vi.waitFor(() => expect(written()).toHaveLength(2));
    expect(written()[1]).not.toHaveProperty("screenReader");
  });

  it("stays on after Use the default layout, and the new file keeps it for the next launch", async () => {
    handed({ version: 2, regions: [], screenReader: true });
    expect(screenReaderMode()).toBe(true);

    usingTheDefaultLayout();

    expect(screenReaderMode()).toBe(true);
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].screenReader).toBe(true);
  });

  it("writes no file after Use the default layout while it is off", async () => {
    handed({ version: 2, regions: [] });
    expect(screenReaderMode()).toBe(false);

    usingTheDefaultLayout();
    await Promise.resolve();

    expect(written()).toHaveLength(0);
  });

  it("tells whoever listens, once per change", () => {
    const heard: boolean[] = [];
    const stop = onScreenReaderMode((on) => heard.push(on));

    setScreenReaderMode(true);
    setScreenReaderMode(true);
    setScreenReaderMode(false);
    stop();
    setScreenReaderMode(true);

    expect(heard).toEqual([true, false]);
  });
});
