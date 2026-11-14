// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  DEFAULT_NOTIFICATIONS,
  loadNotifications,
  notificationPrefs,
  setExpiringSound,
  setFinishedAfter,
  setQuietHours,
  setRemind,
  setSourceOn,
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
      prefs: {
        quiet: { on: false, from: "22:00", to: "07:00" },
        finishedAfter: 5,
        off: [],
        expiringSound: false,
        remind: "gently",
      },
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

describe("how long a task works before its finish notifies (#6005, ADR 0121 decision 3)", () => {
  it("is five minutes until the person changes it", () => {
    expect(DEFAULT_NOTIFICATIONS.finishedAfter).toBe(5);
    expect(notificationPrefs().finishedAfter).toBe(5);
  });

  it("is read from the file as whole minutes, beside the quiet hours", () => {
    handed({ version: 2, regions: [], notifications: { finishedAfter: 15 } });
    expect(notificationPrefs()).toEqual({
      quiet: { on: false, from: "22:00", to: "07:00" },
      finishedAfter: 15,
      off: [],
      expiringSound: false,
      remind: "gently",
    });
  });

  it("puts right a value that is not a whole number of minutes above none", () => {
    for (const not of [0, -2, 2.5, "10"]) {
      const { prefs, said } = loadNotifications({ notifications: { finishedAfter: not } });
      expect(prefs.finishedAfter).toBe(5);
      expect(said.join(" ")).toContain("notifications.finishedAfter");
    }
  });

  it("is written when changed, and left out of the file at five", async () => {
    setFinishedAfter(10);
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].notifications).toEqual({ finishedAfter: 10 });
    setFinishedAfter(5);
    await vi.waitFor(() => expect(written()).toHaveLength(2));
    expect(written()[1].notifications).toBeUndefined();
  });
});

describe("notifications per source (#6019, ADR 0121 decision 22)", () => {
  it("are on for a failed task and a long finish until the person turns one off", () => {
    expect(notificationPrefs().off).toEqual([]);
  });

  it("are read off from the file the window was handed", () => {
    handed({ version: 2, regions: [], notifications: { off: ["task-failed"] } });
    expect(notificationPrefs().off).toEqual(["task-failed"]);
  });

  it("keep no kind that is no source, and say what was left out", () => {
    const { prefs, said } = loadNotifications({
      notifications: { off: ["task-done", "ask", 3] },
    });
    expect(prefs.off).toEqual(["task-done"]);
    expect(said.join(" ")).toContain("notifications.off");
    expect(loadNotifications({ notifications: { off: "task-done" } }).prefs.off).toEqual([]);
  });

  it("are written as the kinds turned off, and left out while none is", async () => {
    setSourceOn("task-done", false);
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].notifications).toEqual({ off: ["task-done"] });
    setSourceOn("task-done", true);
    await vi.waitFor(() => expect(written()).toHaveLength(2));
    expect(written()[1].notifications).toBeUndefined();
  });
});

describe("the Expiring sound (ADR 0121 decision 12)", () => {
  it("is off until the person turns it on, and written only while on", async () => {
    expect(notificationPrefs().expiringSound).toBe(false);
    setExpiringSound(true);
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].notifications).toEqual({ expiringSound: true });
    expect(loadNotifications({ notifications: { expiringSound: "yes" } }).said.join(" ")).toContain(
      "notifications.expiringSound",
    );
  });
});

describe("Remind me (#6344, ADR 0121 amended 2026-11-12)", () => {
  it("is Gently until the person changes it, and left out of the file at Gently", async () => {
    expect(notificationPrefs().remind).toBe("gently");
    setRemind("often");
    await vi.waitFor(() => expect(written()).toHaveLength(1));
    expect(written()[0].notifications).toEqual({ remind: "often" });
    setRemind("gently");
    await vi.waitFor(() => expect(written()).toHaveLength(2));
    expect(written()[1].notifications).toBeUndefined();
  });

  it("is read from the file the window was handed, so it survives a restart", () => {
    handed({ version: 2, regions: [], notifications: { remind: "off" } });
    expect(notificationPrefs().remind).toBe("off");
  });

  it("reads a value that is no pace as Gently, and says so", () => {
    const { prefs, said } = loadNotifications({ notifications: { remind: "loudly" } });
    expect(prefs.remind).toBe("gently");
    expect(said.join(" ")).toContain("notifications.remind");
  });
});
