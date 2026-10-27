// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { BrowserFrame } from "./bindings";
import { stripNamed } from "./test-strips";

/**
 * **Inspect: a browser's DevTools, beside it** (#2028): Inspect on a browser view takes the
 * browser over, opens the engine's DevTools in the session host and their view beside the
 * browser, which draws each frame the host sends on a canvas; the person's keys and clicks there
 * go to DevTools alone; and once DevTools close, their view says so. jsdom decodes no image, so
 * what reaches a canvas is read at the one seam that draws (`browserFrames.paint`).
 */

const painted: number[][] = [];
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (_canvas: HTMLCanvasElement, jpeg: Uint8Array) => {
    painted.push(Array.from(jpeg));
  },
}));

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

/** The pool whose runner the browser runs on, as `watch_browser` answers (#2050). */
let onPool: string | null = null;

beforeEach(() => {
  painted.length = 0;
  onPool = null;
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function core() {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let devtools: Channel<BrowserFrame> | undefined;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "plane_sidebar")
        return { root: PLANE, personas: [], persona: null, unfiled: [], workspaces: [] };
      if (cmd === "open_browser") return "B1";
      if (cmd === "watch_browser")
        return { view: 4, width: 1280, height: 800, person_drives: false, pool: onPool };
      if (cmd === "watch_inspector") {
        devtools = (args as { frames: Channel<BrowserFrame> }).frames;
        return { view: 5, width: 1000, height: 700, person_drives: true, pool: null };
      }
      if (cmd === "inspector_input") return { outcome: "taken" };
      if (cmd === "browser_input") return { outcome: "taken" };
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
    order: () => asked.map(({ cmd }) => cmd),
    devtoolsSend: (jpeg: string, aim = "devtools:1") =>
      devtools?.onmessage({ aim, jpeg, reconnection: null, page: null }),
  };
}

async function aBrowser() {
  const host = core();
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  await screen.findByTestId("title-about");
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard("New browser");
  await userEvent.keyboard("{Enter}");
  await screen.findByLabelText("The browser's page");
  return host;
}

const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

describe("Inspect", () => {
  it("takes the browser over, opens its DevTools and draws them beside it", async () => {
    const host = await aBrowser();

    await userEvent.click(await screen.findByRole("button", { name: "Inspect" }));

    await waitFor(() =>
      expect(host.asked("inspect_browser")).toEqual([{ program: "B1", width: 1000, height: 700 }]),
    );
    const order = host.order();
    expect(order.indexOf("take_over_browser")).toBeLessThan(order.indexOf("inspect_browser"));
    await screen.findByTestId("devtools-view");
    await waitFor(() =>
      expect(host.asked("watch_inspector")).toContainEqual(
        expect.objectContaining({ program: "B1" }),
      ),
    );
    // Beside the browser, in its tab: one tab still.
    expect(tabNames()).toEqual(["Browser"]);
    expect(screen.getByLabelText("The browser's page")).toBeInTheDocument();

    host.devtoolsSend("/9j/AA==");
    await waitFor(() => expect(painted).toContainEqual([0xff, 0xd8, 0xff, 0x00]));
  });

  it("sends the person's keys and clicks to DevTools alone, aimed at what they showed", async () => {
    const host = await aBrowser();
    await userEvent.click(await screen.findByRole("button", { name: "Inspect" }));
    const surface = await screen.findByTestId("devtools-surface");
    await waitFor(() => expect(host.asked("watch_inspector").length).toBeGreaterThan(0));
    host.devtoolsSend("/9j/AA==", "devtools:3");
    await waitFor(() => expect(painted.length).toBeGreaterThan(0));
    const before = host.asked("browser_input").length;

    fireEvent.keyDown(surface, { key: "a", code: "KeyA", keyCode: 65 });

    await waitFor(() =>
      expect(host.asked("inspector_input")).toEqual([
        {
          program: "B1",
          input: expect.objectContaining({
            kind: "key",
            aim: "devtools:3",
            action: "down",
            key: "a",
          }),
        },
      ]),
    );
    expect(host.asked("browser_input")).toHaveLength(before);
  });

  it("says DevTools are closed once they close", async () => {
    const host = await aBrowser();
    await userEvent.click(await screen.findByRole("button", { name: "Inspect" }));
    await screen.findByTestId("devtools-view");
    await waitFor(() => expect(host.asked("watch_inspector").length).toBeGreaterThan(0));

    host.devtoolsSend("");

    expect(await screen.findByText("DevTools are closed")).toBeInTheDocument();
  });

  it("is not offered for a browser on a pool's runner", async () => {
    onPool = "qa";
    await aBrowser();
    await screen.findByTestId("browser-chip");
    await waitFor(() => expect(screen.getByTestId("browser-chip")).toHaveTextContent("qa"));

    expect(screen.queryByRole("button", { name: "Inspect" })).toBeNull();
  });
});
