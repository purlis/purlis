// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "../App";
import type { BrowserLimitShown, BrowserLimitsShown } from "../bindings";
import { forgetThisLaunch } from "../regions";
import { GLOBAL } from "../windowprefs";

/**
 * **Settings › You › This machine › Browsers** (#2023), in the whole window: each limit shows
 * what is in force, a value typed is written once the field is left, Reset puts the default
 * back, and a value out of its bounds is refused in its row and written nowhere.
 */

vi.mock("../SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const shown = (value: number, deflt: number, most: number, least = 1): BrowserLimitShown => ({
  value,
  default: deflt,
  set: value !== deflt,
  least,
  most,
});

let limits: BrowserLimitsShown;
let sent: { limit: unknown; to: unknown }[];

beforeEach(() => {
  forgetThisLaunch();
  (globalThis as Record<string, unknown>)[GLOBAL] = {
    layout: { path: "/home/op/.config/charter/layout.json", found: false, document: null },
    theme: { path: "", found: false, document: null, trouble: null },
  };
  limits = {
    per_project: shown(3, 3, 16),
    on_this_device: shown(6, 8, 64),
    idle_minutes: shown(10, 10, 1440, 0),
  };
  sent = [];
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "plane_at_launch") return { plane: null, from: null, why: "no plane here" };
      if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
      if (cmd === "planes_to_restore") return { windows: [], dropped: [], gone: [] };
      if (cmd === "this_machine") return { projects: [], dropped: [], forgetful: null };
      if (cmd === "update_channel") return "stable";
      if (cmd === "browser_limits") return structuredClone(limits);
      if (cmd === "set_browser_limit") {
        sent.push({ limit: a.limit, to: a.to });
        const key = a.limit as keyof BrowserLimitsShown;
        const one = limits[key];
        const to = a.to === null ? one.default : (a.to as number);
        limits = { ...limits, [key]: { ...one, value: to, set: a.to !== null } };
        return null;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  Reflect.deleteProperty(globalThis, GLOBAL);
});

async function browsersGroup() {
  render(<App />);
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard("Settings…");
  await userEvent.keyboard("{Enter}");
  const nav = await screen.findByRole("navigation", { name: "Groups" });
  await userEvent.click(within(nav).getByRole("button", { name: "Browsers" }));
  return screen.findByRole("region", { name: "Browsers" });
}

describe("You › This machine › Browsers", () => {
  it("shows each limit in force, and writes a value typed once the field is left", async () => {
    const group = await browsersGroup();
    const device = await within(group).findByRole("textbox", { name: "Browsers on this machine" });
    await waitFor(() => expect(device).toHaveValue("6"));

    const idle = within(group).getByRole("textbox", {
      name: "Minutes before an unused browser stops",
    });
    await userEvent.clear(idle);
    await userEvent.type(idle, "0");
    await userEvent.tab();

    await waitFor(() => expect(sent).toEqual([{ limit: "idle_minutes", to: 0 }]));
    await waitFor(() => expect(idle).toHaveValue("0"));
  });

  it("puts a limit back to its default with Reset", async () => {
    const group = await browsersGroup();
    await waitFor(() =>
      expect(within(group).getByRole("textbox", { name: "Browsers on this machine" })).toHaveValue(
        "6",
      ),
    );

    await userEvent.click(within(group).getByRole("button", { name: "Reset to 8" }));

    await waitFor(() => expect(sent).toEqual([{ limit: "on_this_device", to: null }]));
  });

  it("puts a field left blank back to its default, never to 0", async () => {
    limits = { ...limits, idle_minutes: shown(25, 10, 1440, 0) };
    const group = await browsersGroup();
    const idle = await within(group).findByRole("textbox", {
      name: "Minutes before an unused browser stops",
    });
    await waitFor(() => expect(idle).toHaveValue("25"));

    await userEvent.clear(idle);
    await userEvent.tab();

    await waitFor(() => expect(sent).toEqual([{ limit: "idle_minutes", to: null }]));
    await waitFor(() => expect(idle).toHaveValue("10"));
  });

  it("refuses a value out of its bounds in its row, and writes nothing", async () => {
    const group = await browsersGroup();
    const project = await within(group).findByRole("textbox", { name: "Browsers per project" });
    await waitFor(() => expect(project).toHaveValue("3"));

    await userEvent.clear(project);
    await userEvent.type(project, "40");
    await userEvent.tab();

    expect(await within(group).findByText(/set between 1 and 16/)).toBeInTheDocument();
    expect(sent).toEqual([]);
  });
});
