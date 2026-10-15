// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  DEFAULT_NOTIFICATIONS,
  loadNotifications,
  notificationPrefs,
  setQuietHours,
} from "./notificationPrefs";
import { forgetThisLaunch } from "./regions";
import { aboutThisMachine, GLOBAL, sayAboutThisMachine } from "./windowprefs";

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
  forgetThisLaunch();
  sayAboutThisMachine("notifications", undefined);
  sent = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args });
    return null;
  });
});
afterEach(() => {
  Reflect.deleteProperty(globalThis, GLOBAL);
  clearMocks();
});

describe("quiet hours, kept in the layout file (#1989)", () => {
  it("are off until the person turns them on, from 22:00 to 07:00", () => {
    expect(loadNotifications({ version: 2, regions: [] })).toEqual({
      prefs: { quiet: { on: false, from: "22:00", to: "07:00" } },
      said: [],
    });
    expect(notificationPrefs()).toEqual(DEFAULT_NOTIFICATIONS);
  });

  it("are read from the file the window was handed", () => {
    handed({
      version: 2,
      regions: [],
      notifications: { quiet: { on: true, from: "23:30", to: "06:00" } },
    });
    expect(notificationPrefs().quiet).toEqual({ on: true, from: "23:30", to: "06:00" });
  });

  it("put right each field that is not one, and the Inbox says where", () => {
    handed({
      version: 2,
      regions: [],
      notifications: { quiet: { on: "yes", from: "25:00", to: "06:00" } },
    });
    expect(notificationPrefs().quiet).toEqual({ on: false, from: "22:00", to: "06:00" });
    const [said] = aboutThisMachine();
    expect(said.subject).toBe("notifications");
    expect(said.detail).toContain(PATH);
    expect(said.detail).toContain('"25:00"');
    expect(said.detail).toContain('"yes"');
  });

  it("are written when changed, and left out of the file at the defaults", async () => {
    setQuietHours({ on: true });
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].notifications).toEqual({
      quiet: { on: true, from: "22:00", to: "07:00" },
    });
    setQuietHours({ on: false });
    await vi.waitFor(() => expect(written()).toHaveLength(2));
    expect(written()[1].notifications).toBeUndefined();
  });
});
