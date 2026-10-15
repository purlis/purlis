// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { stripNamed } from "./test-strips";

/**
 * **A browser, in a view tab of the window** (#2012): the palette's New browser starts one in
 * the session host and opens its tab, whose view draws each frame the host sends on a canvas;
 * a refusal (Stop all) is said and opens nothing; and when the last tab showing a browser
 * closes, the browser ends. jsdom decodes no image, so what reaches the canvas is read at the
 * one seam that draws (`browserFrames.paint`).
 */

const painted: number[][] = [];
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (_canvas: HTMLCanvasElement, jpeg: Uint8Array) => {
    painted.push(Array.from(jpeg));
  },
}));

// The palette offers New browser once #2176 turns it on; these tests turn it on here.
vi.mock("./browsers", async (actual) => ({
  ...(await actual<typeof import("./browsers")>()),
  BROWSERS_OFFERED: true,
}));

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

beforeEach(() => {
  painted.length = 0;
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function core(open: () => unknown = () => "B1") {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  let frames: Channel<string> | undefined;
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
      if (cmd === "open_browser") return open();
      if (cmd === "watch_browser") {
        frames = (args as { frames: Channel<string> }).frames;
        return { view: 4, width: 1280, height: 800 };
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
    send: (base64: string) => frames?.onmessage(base64),
  };
}

async function fromThePalette(row: string) {
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard(row);
  await userEvent.keyboard("{Enter}");
}

const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

describe("a browser's view tab", () => {
  it("opens from the palette and draws each frame the host sends, as the focused view", async () => {
    const host = core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");

    await fromThePalette("New browser");

    await waitFor(() => expect(tabNames()).toEqual(["Browser"]));
    expect(host.asked("open_browser")).toEqual([{ plane: PLANE }]);
    await screen.findByLabelText("The browser's page");
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B1", role: "focused" }),
      ),
    );

    host.send("/9j/AA==");
    await waitFor(() => expect(painted).toEqual([[0xff, 0xd8, 0xff, 0x00]]));

    host.send("");
    expect(await screen.findByText("This browser has ended")).toBeInTheDocument();
  });

  it("is refused in words while Stop all holds, and opens nothing", async () => {
    core(() => Promise.reject("Every chat purlis started is stopped."));
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");

    await fromThePalette("New browser");

    expect(await screen.findByText("Every chat purlis started is stopped.")).toBeInTheDocument();
    expect(tabNames()).toEqual([]);
  });

  it("ends its browser when its tab closes, after closing its view", async () => {
    const host = core();
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await screen.findByTestId("title-about");
    await fromThePalette("New browser");
    await waitFor(() => expect(tabNames()).toEqual(["Browser"]));
    await waitFor(() => expect(host.asked("watch_browser").length).toBeGreaterThan(0));

    await fromThePalette("Close this view");

    await waitFor(() => expect(host.asked("close_browser")).toEqual([{ program: "B1" }]));
    expect(host.asked("unwatch_browser")).toContainEqual({ program: "B1", view: 4 });
  });
});

describe("a browser's view nobody can see", () => {
  it("is closed while its project is not in front or the window is hidden, and opened again when seen", async () => {
    const { BrowserView } = await import("./BrowserView");
    const { ProjectShownProvider } = await import("./projectShown");
    const host = core();
    const drawn = (inFront: boolean) => (
      <ProjectShownProvider value={inFront}>
        <BrowserView program="B1" />
      </ProjectShownProvider>
    );
    const { rerender } = render(drawn(false));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(host.asked("watch_browser")).toEqual([]);

    rerender(drawn(true));
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(1));

    let hidden = true;
    Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() =>
      expect(host.asked("unwatch_browser")).toEqual([{ program: "B1", view: 4 }]),
    );

    hidden = false;
    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() => expect(host.asked("watch_browser")).toHaveLength(2));

    rerender(drawn(false));
    await waitFor(() => expect(host.asked("unwatch_browser")).toHaveLength(2));
    delete (document as unknown as { hidden?: boolean }).hidden;
  });
});
