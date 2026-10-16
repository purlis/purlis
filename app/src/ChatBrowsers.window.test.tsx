// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ChatBrowser, OpenChat } from "./bindings";
import { setChatsListPrefs } from "./chatsListPrefs";
import { forgetThisLaunch } from "./regions";
import { stripNamed } from "./test-strips";

/**
 * **A chat's browsers, in the window** (#2024, spec #2008): each is a row under its chat in the
 * Chats list and a live thumbnail on its chat's tab, either of which opens its browser view in
 * a tab of its own; a tab showing it closes only the view, since the browser ends with its chat;
 * the person's setting opens a new one beside its chat; and the view draws the chip that names
 * the device, outside the page. jsdom decodes no image, so what reaches a canvas is read at the
 * one seam that draws (`browserFrames.paint`).
 */

const painted = vi.hoisted(
  () => [] as { canvas: HTMLCanvasElement; jpeg: number[]; across?: number }[],
);
vi.mock("./browserFrames", async (actual) => ({
  ...(await actual<typeof import("./browserFrames")>()),
  paint: async (canvas: HTMLCanvasElement, jpeg: Uint8Array, across?: number) => {
    painted.push({ canvas, jpeg: Array.from(jpeg), across });
  },
}));

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const PLANE = "/home/dev/plane";

function chat(session: number): OpenChat {
  return {
    session,
    name: String(session),
    cwd: `${PLANE}/workspaces/alpha`,
    harness: "claude",
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core, with chats 1 and 2 open in alpha and `browsers` as their browsers. */
function core(browsers: ChatBrowser[]) {
  const asked: Asked[] = [];
  const listeners = new Map<string, number>();
  const views = new Map<string, Channel<string>>();
  let view = 0;
  const open = [chat(1), chat(2)];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "chat_browsers") return browsers;
    if (cmd === "watch_browser") {
      view += 1;
      views.set(`${String(a.program)} ${String(a.role)}`, a.frames as Channel<string>);
      return { view, width: 1280, height: 800 };
    }
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [
          {
            name: "alpha",
            path: `${PLANE}/workspaces/alpha`,
            vision: "",
            todos: [],
            colour: null,
            live: false,
            chats: open,
          },
        ],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: [],
        paths: {},
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
      };
    if (cmd === "workspace_repos")
      return { workspace: a.workspace, repos: [], cache_refused: null };
    if (cmd === "finished_tasks") return [];
    if (cmd === "stopping_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args),
    /** The core says this project's chats' browsers came or went. */
    listed: (now: ChatBrowser[]) => {
      browsers.splice(0, browsers.length, ...now);
      const handler = listeners.get("chat-browsers-listed");
      if (handler === undefined) throw new Error("the window is not listening for browsers");
      act(() => {
        window.__TAURI_INTERNALS__.runCallback(handler, {
          event: "chat-browsers-listed",
          id: 1,
          payload: { plane: PLANE },
        });
      });
    },
    /** A frame on the view of `program` attached as `role`. */
    frame: (program: string, role: "focused" | "thumbnail", base64: string) =>
      views.get(`${program} ${role}`)?.onmessage(base64),
  };
}

const B7: ChatBrowser = { session: 1, name: "b1", program: "B7", unended: false };
const B8: ChatBrowser = { session: 1, name: "b2", program: "B8", unended: false };

const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

const chatsTree = () => screen.findByRole("tree", { name: "Chats of this project" });

async function fromThePalette(row: string) {
  await userEvent.keyboard("{F2}");
  await screen.findByRole("dialog", { name: "Command palette" });
  await userEvent.keyboard(row);
  await userEvent.keyboard("{Enter}");
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
  painted.length = 0;
});
afterEach(() => {
  cleanup();
  clearMocks();
  setChatsListPrefs({ beside: false });
});

const opened = async () => {
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  await screen.findByTestId("title-about");
};

describe("a chat's browser in the Chats list", () => {
  it("is a row under its chat, and its row opens its browser view in a tab of its own", async () => {
    const host = core([B7]);
    await opened();

    const browsers = within(await chatsTree()).getByRole("group", {
      name: "Browsers of steward 1",
    });
    await userEvent.click(within(browsers).getByRole("button", { name: /Browser b1/ }));

    await waitFor(() => expect(tabNames()).toContain("Browser b1 of steward 1"));
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "focused" }),
      ),
    );
  });

  it("comes and goes as the chat's browsers do", async () => {
    const host = core([]);
    await opened();
    const tree = await chatsTree();
    expect(within(tree).queryByRole("group", { name: "Browsers of steward 1" })).toBeNull();

    host.listed([B7, B8]);

    const browsers = await within(tree).findByRole("group", { name: "Browsers of steward 1" });
    expect(
      within(browsers)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual([expect.stringContaining("Browser b1"), expect.stringContaining("Browser b2")]);

    host.listed([]);
    await waitFor(() =>
      expect(within(tree).queryByRole("group", { name: "Browsers of steward 1" })).toBeNull(),
    );
  });
});

describe("a chat's browser on its chat's tab", () => {
  it("is drawn live there, as a thumbnail, and pressed opens its browser view", async () => {
    const host = core([B7]);
    await opened();

    const thumbnail = await screen.findByRole("button", { name: "Open browser b1 of steward 1" });
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "thumbnail" }),
      ),
    );
    host.frame("B7", "thumbnail", "/9j/AA==");
    await waitFor(() =>
      expect(painted.map((one) => one.jpeg)).toContainEqual([0xff, 0xd8, 0xff, 0x00]),
    );
    expect(thumbnail.querySelector("canvas")).not.toBeNull();
    // Drawn from a frame made small, so a thumbnail holds no page-sized picture.
    const { THUMBNAIL_ACROSS } = await import("./browserWatch");
    expect(THUMBNAIL_ACROSS).toBeGreaterThan(0);
    expect(painted.find((one) => one.canvas === thumbnail.querySelector("canvas"))?.across).toBe(
      THUMBNAIL_ACROSS,
    );

    await userEvent.click(thumbnail);

    await waitFor(() => expect(tabNames()).toContain("Browser b1 of steward 1"));
  });

  it("is on no tab of a chat that has no browser", async () => {
    core([B7]);
    await opened();
    await screen.findByRole("button", { name: "Open browser b1 of steward 1" });

    expect(screen.queryByRole("button", { name: /Open browser .* of steward 2/ })).toBeNull();
  });
});

describe("a chat's browsers counted on its tab", () => {
  it("says how many browsers the chat has, in the tab's count style", async () => {
    core([B7, B8]);
    await opened();

    const count = await screen.findByRole("img", { name: "2 browsers" });
    expect(count).toHaveClass("count-pill");
    expect(count.closest(".tab")).not.toBeNull();
  });
});

describe("a chat's browser not ended yet (D-2013-16)", () => {
  it("is a row under its chat that says so, and has no thumbnail on the tab", async () => {
    core([B7, { ...B8, unended: true }]);
    await opened();

    const browsers = within(await chatsTree()).getByRole("group", {
      name: "Browsers of steward 1",
    });
    const row = within(browsers).getByRole("button", { name: /Browser b2/ });
    expect(row).toHaveTextContent("not ended yet");
    await screen.findByRole("button", { name: "Open browser b1 of steward 1" });
    expect(screen.queryByRole("button", { name: "Open browser b2 of steward 1" })).toBeNull();
    expect(screen.getByRole("img", { name: "1 browser" })).toBeInTheDocument();
  });
});

describe("a chat's browser's thumbnail", () => {
  it("closes its view while the window is hidden, and opens it again when seen", async () => {
    const host = core([B7]);
    await opened();
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "thumbnail" }),
      ),
    );
    const watched = () =>
      host.asked("watch_browser").filter((one) => one.role === "thumbnail").length;
    const before = watched();

    let hidden = true;
    Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await waitFor(() =>
      expect(host.asked("unwatch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7" }),
      ),
    );

    hidden = false;
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await waitFor(() => expect(watched()).toBeGreaterThan(before));
    delete (document as unknown as { hidden?: boolean }).hidden;
  });

  it("closes its view when the browser leaves its chat", async () => {
    const host = core([B7]);
    await opened();
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "thumbnail" }),
      ),
    );

    host.listed([]);

    await waitFor(() =>
      expect(host.asked("unwatch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7" }),
      ),
    );
    expect(screen.queryByRole("button", { name: "Open browser b1 of steward 1" })).toBeNull();
  });
});

describe("a tab showing a chat's browser", () => {
  it("closes its view and leaves the browser running: it ends with its chat", async () => {
    const host = core([B7]);
    await opened();
    await userEvent.click(
      await screen.findByRole("button", { name: "Open browser b1 of steward 1" }),
    );
    await waitFor(() => expect(tabNames()).toContain("Browser b1 of steward 1"));
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "focused" }),
      ),
    );

    await fromThePalette("Close this view");

    await waitFor(() => expect(tabNames()).not.toContain("Browser b1 of steward 1"));
    expect(host.asked("close_browser")).toEqual([]);
  });

  it("names the device the browser runs on, outside the page", async () => {
    core([B7]);
    await opened();
    await userEvent.click(
      await screen.findByRole("button", { name: "Open browser b1 of steward 1" }),
    );

    const view = await screen.findByTestId("browser-view");
    expect(within(view).getByTestId("browser-chip").textContent).toMatch(
      /^On this (Mac|computer)$/,
    );
  });
});

describe("the setting that opens a chat's browser beside it", () => {
  it("opens a new browser of a chat beside the chat, in its tab, when it is on", async () => {
    setChatsListPrefs({ beside: true });
    const host = core([]);
    await opened();
    await screen.findByRole("tree", { name: "Chats of this project" });
    const before = tabNames();

    host.listed([B7]);

    await screen.findByLabelText("The browser's page");
    expect(tabNames()).toEqual(before);
  });

  it("opens nothing when it is off: the row and the tab offer it", async () => {
    const host = core([]);
    await opened();
    await screen.findByRole("tree", { name: "Chats of this project" });

    host.listed([B7]);

    await screen.findByRole("button", { name: "Open browser b1 of steward 1" });
    expect(screen.queryByLabelText("The browser's page")).toBeNull();
  });
});
