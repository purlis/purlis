// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { BrowserFrame, ChatBrowser, OpenChat, TimelineMoment } from "./bindings";
import { setChatsListPrefs } from "./chatsListPrefs";
import { forgetThisLaunch } from "./regions";
import { stripNamed } from "./test-strips";
import { eventually, tipOf } from "./ui/tooltip/tooltip.testkit";

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

/** Chat 1's id, where a test gives it one (ADR 0066). */
const CHAT_1 = "01JB0000000000000000000001";

/** Chat 1's browser timeline, as the core keeps it (#2017). */
let timeline: TimelineMoment[] = [];

/** The core, with chats 1 and 2 open in alpha and `browsers` as their browsers. */
function core(browsers: ChatBrowser[]) {
  const asked: Asked[] = [];
  const listeners = new Map<string, number>();
  const views = new Map<string, Channel<BrowserFrame>>();
  let view = 0;
  const open = [{ ...chat(1), id: CHAT_1 }, chat(2)];
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
    if (cmd === "chat_browser_timeline") return a.chat === CHAT_1 ? timeline : [];
    if (cmd === "chat_browser_timeline_shot") return "iVBORw==";
    if (cmd === "watch_browser") {
      view += 1;
      views.set(`${String(a.program)} ${String(a.role)}`, a.frames as Channel<BrowserFrame>);
      return { view, width: 1280, height: 800, person_drives: false };
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
    if (cmd === "browser_input") return { outcome: "taken" };
    if (cmd === "hand_browser_to_chat") return "b2";
    if (cmd === "browsers_at_quit") return atQuit;
    if (cmd === "open_browser") return "B7";
    if (cmd === "go_browser") return { outcome: "went", address: a.address ?? null };
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
    /** The view of `program` attached as `role` is told its page's origin (#2300). */
    page: (program: string, role: "focused" | "thumbnail", origin: string | null) =>
      act(() =>
        views
          .get(`${program} ${role}`)
          ?.onmessage({ aim: "", jpeg: "", reconnection: null, page: { origin } }),
      ),
    /** A frame on the view of `program` attached as `role`. */
    frame: (program: string, role: "focused" | "thumbnail", base64: string) =>
      views
        .get(`${program} ${role}`)
        ?.onmessage({ aim: "p1:1", jpeg: base64, reconnection: null, page: null }),
  };
}

const B7: ChatBrowser = {
  session: 1,
  name: "b1",
  program: "B7",
  unended: false,
  pinned: false,
  was: null,
  login: null,
  driver: null,
};
const B8: ChatBrowser = { ...B7, name: "b2", program: "B8" };
/** A browser pinned by chat 9, which has closed (#2022). */
const B9: ChatBrowser = {
  session: 9,
  name: "b1",
  program: "B9",
  unended: false,
  pinned: true,
  was: "Fix the login",
  login: null,
  driver: null,
};

/** Where the project's browsers were at the last quit, as the core says (#2022). */
let atQuit: string[] = [];

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
  atQuit = [];
  timeline = [];
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

describe("a chat's browser view, opened from its row", () => {
  // jsdom lays nothing out: the page is drawn over 1280 by 800 pixels of the view, at its size.
  const box = { left: 0, top: 0, width: 1280, height: 800, right: 1280, bottom: 800, x: 0, y: 0 };
  beforeEach(() => {
    vi.spyOn(HTMLCanvasElement.prototype, "getBoundingClientRect").mockReturnValue({
      ...box,
      toJSON: () => box,
    } as DOMRect);
  });
  afterEach(() => vi.restoreAllMocks());

  it("is taken over with a click, as any browser view is (#2014)", async () => {
    const host = core([B7]);
    await opened();
    const browsers = within(await chatsTree()).getByRole("group", {
      name: "Browsers of steward 1",
    });
    await userEvent.click(within(browsers).getByRole("button", { name: /Browser b1/ }));
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "focused" }),
      ),
    );
    const view = await screen.findByTestId("browser-view");
    const bar = within(view).getByRole("status");
    await screen.findByLabelText("The browser's page");
    expect(bar).toHaveTextContent("Click or type in the page to take it over.");
    host.frame("B7", "focused", "/9j/AA==");

    fireEvent.pointerDown(screen.getByTestId("browser-surface"), {
      clientX: 640,
      clientY: 400,
      button: 0,
      detail: 1,
      pointerId: 1,
    });

    await waitFor(() => expect(host.asked("take_over_browser")).toEqual([{ program: "B7" }]));
    await waitFor(() => expect(host.asked("browser_input")).toHaveLength(1));
    await waitFor(() => expect(bar).toHaveTextContent("You are driving this browser."));
    expect(within(view).getByRole("button", { name: /Done/ })).toBeInTheDocument();
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

describe("pinning a chat's browser (#2022)", () => {
  it("is a row of the palette and of the browser's menu, and its row says it is pinned", async () => {
    const host = core([B7]);
    await opened();
    await within(await chatsTree()).findByRole("group", { name: "Browsers of steward 1" });

    await fromThePalette("Pin browser b1 of steward 1");

    await waitFor(() =>
      expect(host.asked("pin_chat_browser")).toEqual([
        { plane: PLANE, session: 1, program: "B7", pinned: true },
      ]),
    );
    host.listed([{ ...B7, pinned: true }]);
    const browsers = within(await chatsTree()).getByRole("group", {
      name: "Browsers of steward 1",
    });
    await waitFor(() =>
      expect(within(browsers).getByRole("button", { name: /Browser b1/ })).toHaveTextContent(
        "pinned",
      ),
    );
    await fromThePalette("Unpin browser b1 of steward 1");
    await waitFor(() =>
      expect(host.asked("pin_chat_browser")).toContainEqual({
        plane: PLANE,
        session: 1,
        program: "B7",
        pinned: false,
      }),
    );
  });
});

describe("a pinned browser whose chat closed (#2022)", () => {
  it("is a row of its own under Pinned browsers, by what its chat was called", async () => {
    core([B9]);
    await opened();

    const pinned = within(await chatsTree()).getByRole("group", { name: "Pinned browsers" });
    expect(within(pinned).getByRole("button", { name: /Browser b1/ })).toHaveTextContent(
      "Fix the login",
    );
  });

  it("is handed to an open chat from the palette", async () => {
    const host = core([B9]);
    await opened();
    await within(await chatsTree()).findByRole("group", { name: "Pinned browsers" });

    await fromThePalette("Hand browser b1 of Fix the login to steward 2");

    await waitFor(() =>
      expect(host.asked("hand_browser_to_chat")).toEqual([
        { plane: PLANE, program: "B9", session: 2 },
      ]),
    );
  });

  it("is stopped by the person, and only then", async () => {
    const host = core([B9]);
    await opened();
    const pinned = within(await chatsTree()).getByRole("group", { name: "Pinned browsers" });
    await userEvent.click(within(pinned).getByRole("button", { name: /Browser b1/ }));
    await waitFor(() => expect(tabNames()).toContain("Browser b1 of Fix the login"));
    await fromThePalette("Close this view");
    await waitFor(() => expect(tabNames()).not.toContain("Browser b1 of Fix the login"));
    expect(host.asked("close_browser")).toEqual([]);

    await fromThePalette("Stop browser b1 of Fix the login");

    await waitFor(() => expect(host.asked("close_browser")).toEqual([{ program: "B9" }]));
  });
});

describe("the browsers open when purlis last quit (#2022)", () => {
  it("are offered once, each address a button, and nothing is loaded", async () => {
    atQuit = ["https://staging.example.test/cart", "http://127.0.0.1:5173/"];
    const host = core([]);
    await opened();

    const notice = await screen.findByText(/When purlis last quit/);
    expect(notice).toHaveTextContent("https://staging.example.test/cart");
    expect(screen.getByRole("button", { name: "Open staging.example.test" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open 127.0.0.1:5173" })).toBeInTheDocument();
    expect(host.asked("open_browser")).toEqual([]);
  });

  it("opens a browser of the person's own at an address pressed, which goes there (#2176)", async () => {
    atQuit = ["https://staging.example.test/cart", "http://127.0.0.1:5173/"];
    const host = core([]);
    await opened();
    await screen.findByText(/When purlis last quit/);

    await userEvent.click(screen.getByRole("button", { name: "Open staging.example.test" }));

    await waitFor(() =>
      expect(host.asked("go_browser")).toEqual([
        { plane: PLANE, program: "B7", address: "https://staging.example.test/cart" },
      ]),
    );
    expect(host.asked("open_browser")).toEqual([{ plane: PLANE }]);
    expect(screen.queryByRole("button", { name: "Open staging.example.test" })).toBeNull();
    expect(screen.getByRole("button", { name: "Open 127.0.0.1:5173" })).toBeInTheDocument();
  });

  it("are put away for good by Dismiss", async () => {
    atQuit = ["https://staging.example.test/cart"];
    const host = core([]);
    await opened();
    const notice = (await screen.findByText(/When purlis last quit/)).closest("[data-cause]");
    if (!(notice instanceof HTMLElement)) throw new Error("no Notice");

    await userEvent.click(within(notice).getByRole("button", { name: /Dismiss/ }));

    await waitFor(() =>
      expect(host.asked("put_away_browsers_at_quit")).toEqual([{ plane: PLANE }]),
    );
    expect(screen.queryByText(/When purlis last quit/)).toBeNull();
  });
});

describe("a pinned browser signed in with a login (#2022)", () => {
  it("says so on its row, and its hand-over row names the login", async () => {
    const host = core([{ ...B9, login: "shop-login" }]);
    await opened();
    const pinned = within(await chatsTree()).getByRole("group", { name: "Pinned browsers" });
    expect(within(pinned).getByRole("button", { name: /Browser b1/ })).toHaveTextContent(
      "signed in",
    );

    await fromThePalette(
      "Hand browser b1 of Fix the login, signed in with shop-login, to steward 2",
    );

    await waitFor(() =>
      expect(host.asked("hand_browser_to_chat")).toEqual([
        { plane: PLANE, program: "B9", session: 2 },
      ]),
    );
  });
});

describe("who drives a chat's browser, on its row and its thumbnail (#2300)", () => {
  it("says the person drives it", async () => {
    core([{ ...B7, driver: "person" }]);
    await opened();

    const browsers = within(await chatsTree()).getByRole("group", {
      name: "Browsers of steward 1",
    });
    const row = within(browsers).getByRole("button", { name: /Browser b1/ });
    expect(row.textContent).toContain("you are driving");
    expect(tipOf(row)).toContain("You are driving this browser.");
    const thumbnail = await screen.findByRole("button", {
      name: "Open browser b1 of steward 1, you are driving",
    });
    expect(thumbnail.querySelector(".browser-thumbnail")?.getAttribute("data-driver")).toBe(
      "person",
    );
  });

  it("says purlis drives it while it signs in, and nothing once its agent drives it again", async () => {
    const host = core([{ ...B7, driver: "purlis" }]);
    await opened();
    const tree = await chatsTree();
    const row = () =>
      within(within(tree).getByRole("group", { name: "Browsers of steward 1" })).getByRole(
        "button",
        { name: /Browser b1/ },
      );
    expect(row().textContent).toContain("purlis is signing in");

    host.listed([B7]);

    await waitFor(() => expect(row().textContent).not.toContain("purlis is signing in"));
    expect(row().textContent).not.toContain("you are driving");
  });
});

describe("where a chat's browser's page is (#2300)", () => {
  afterEach(async () => (await import("./browserWatch")).forgetOrigins());

  it("is its origin, on its row's hover and its view's chip, and follows the page", async () => {
    const host = core([B7]);
    await opened();
    await screen.findByRole("button", { name: "Open browser b1 of steward 1" });
    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "thumbnail" }),
      ),
    );
    host.page("B7", "thumbnail", "http://localhost:5173");

    const row = within(
      within(await chatsTree()).getByRole("group", { name: "Browsers of steward 1" }),
    ).getByRole("button", { name: /Browser b1/ });
    await eventually(() => expect(tipOf(row)).toMatch(/localhost:5173 on this (Mac|computer)/));

    await userEvent.click(row);
    const view = await screen.findByTestId("browser-view");
    const chip = within(view).getByTestId("browser-chip");
    expect(chip.textContent).toMatch(/^localhost:5173 on this (Mac|computer)$/);
    expect(tipOf(chip)).toBe("http://localhost:5173");

    await waitFor(() =>
      expect(host.asked("watch_browser")).toContainEqual(
        expect.objectContaining({ program: "B7", role: "focused" }),
      ),
    );
    host.page("B7", "focused", "https://docs.example.test");
    await waitFor(() =>
      expect(chip.textContent).toMatch(/^docs\.example\.test on this (Mac|computer)$/),
    );
    host.page("B7", "focused", null);
    await waitFor(() => expect(chip.textContent).toMatch(/^On this (Mac|computer)$/));
  });
});

describe("a chat's browser timeline (#2017)", () => {
  it("opens from the palette in a tab of its own, listing what the agent did with its screenshots", async () => {
    timeline = [
      {
        at: Date.UTC(2026, 9, 26, 9, 0, 0),
        browser: "b1",
        by: "agent",
        did: "Clicked e12",
        page: "https://shop.example.test/cart",
        shot: null,
        unkept: null,
      },
      {
        at: Date.UTC(2026, 9, 26, 9, 0, 5),
        browser: "b1",
        by: "agent",
        did: "Took a screenshot",
        page: "https://shop.example.test/cart",
        shot: 0,
        unkept: null,
      },
    ];
    const host = core([B7]);
    await opened();
    await within(await chatsTree()).findByRole("group", { name: "Browsers of steward 1" });

    await fromThePalette("Browser timeline of steward 1");

    await waitFor(() => expect(tabNames()).toContain("Browser timeline of steward 1"));
    const list = await screen.findByRole("list", { name: "Browser timeline" });
    const moments = within(list).getAllByRole("listitem");
    // The newest first.
    expect(moments[0]).toHaveTextContent("Took a screenshot");
    expect(moments[1]).toHaveTextContent("Clicked e12");
    await waitFor(() =>
      expect(host.asked("chat_browser_timeline_shot")).toEqual([
        { plane: PLANE, chat: CHAT_1, shot: 0 },
      ]),
    );
    await waitFor(() => expect(painted.length).toBe(1));
  });
});
