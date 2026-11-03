// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved } from "./bindings";

/**
 * **A chat's pane head, against the whole window** (#4760, spec #4750 stories 54, 55, 60 and
 * 61; the accepted prototype `prototype/pane-chat-actions`, variant A).
 *
 * The head shows the chat at a glance (its name, its state, "harness · workspace" and its
 * sandbox) and then, at its end, one state button, ⋯, the bell, Split right, Split down and
 * Close. Its right-click is the chat's list, then the pane's rows. A narrow pane drops its
 * parts in the ruled order (`paneHead.ts`), and the bell, ⋯, Split right and Close never go.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
  sandbox: "floor",
  sandbox_why: null,
  browser_sites: null,
};

/** Chat 4 mid-turn, then waiting for you: what the board says as its turn runs and ends. */
const RUNNING: Moved = {
  plane: PLANE,
  session: 4,
  state: "running",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
} as unknown as Moved;
const WAITING = { ...RUNNING, state: "waiting", moved_at: 2, sequence: 2 } as Moved;

function core() {
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
}

async function aPane() {
  core();
  render(<App />);
  await screen.findByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** The chat's name, as the window shows it: its profile's word before its own. */
const NAME = "claude claude 4";

const frame = () => screen.getByTestId("pane").closest(".pane-frame") as HTMLElement;
const head = () => frame().querySelector(".pane-head") as HTMLElement;
/** The head's controls, start to end, by their names: what a screen reader tabs through. */
const controls = () =>
  [...head().querySelectorAll(".pane-actions button")].map(
    (one) => one.getAttribute("aria-label") ?? one.textContent ?? "",
  );

describe("a chat's pane head", () => {
  it("names the chat, says its harness · workspace and its sandbox", async () => {
    await aPane();

    await waitFor(() => expect(within(head()).getByText(NAME)).toHaveClass("pane-name"));
    expect(head().querySelector(".pane-where")).toHaveTextContent(/^claude · alpha$/);
    // The badge moved from the tab to the head (#4760): named for a screen reader.
    expect(within(head()).getByRole("img", { name: "floor" })).toBeInTheDocument();
  });

  it("ends with ⋯, the bell, Split right, Split down and Close, in that order", async () => {
    await aPane();

    await waitFor(() => expect(controls()[0]).toMatch(/^More for /));
    const [more, bell, right, down, close, ...rest] = controls();
    expect(more).toBe(`More for ${NAME}`);
    expect(bell).toMatch(/^Pane notices:/);
    expect([right, down]).toEqual(["Split right", "Split down"]);
    expect(close).toBeTruthy();
    expect(rest).toEqual([]);
  });

  it("shows Stop while the chat works, before ⋯, and nothing once it is quiet", async () => {
    await aPane();
    await waitFor(() => expect(controls()[0]).toMatch(/^More for /));

    await act(() => emit("chat-moved", RUNNING));
    const stop = await within(head()).findByRole("button", { name: `Stop chat ${NAME}` });
    expect(stop).toHaveTextContent("Stop");
    expect(controls()[0]).toBe(`Stop chat ${NAME}`);
    expect(controls()[1]).toBe(`More for ${NAME}`);

    await act(() => emit("chat-moved", WAITING));
    await waitFor(() =>
      expect(within(head()).queryByRole("button", { name: `Stop chat ${NAME}` })).toBeNull(),
    );
    expect(controls()[0]).toBe(`More for ${NAME}`);
  });

  it("opens the chat's list, then a line, then the pane's rows, on a right-click", async () => {
    await aPane();
    await waitFor(() => expect(controls()[0]).toMatch(/^More for /));

    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    const menu = await screen.findByRole("menu");
    const items = [...menu.querySelectorAll('[role^="menuitem"]')].map(
      (one) => one.getAttribute("aria-label") ?? one.textContent,
    );
    // The chat's own rows first: Rename… is its list's, and End… ends its group.
    const rename = items.findIndex((one) => one?.startsWith("Rename"));
    const end = items.findIndex((one) => one?.startsWith("End…"));
    const split = items.indexOf("Split right");
    expect(rename).toBeGreaterThanOrEqual(0);
    expect(end).toBeGreaterThan(rename);
    expect(split).toBeGreaterThan(end);
    expect(items.slice(split)).toEqual(
      expect.arrayContaining(["Split right", "Split down", "Send F2 to the chat in front"]),
    );
    // Nothing of a tab's and nothing of the window's.
    expect(items).not.toContain("Pinned");
    expect(items).not.toContain("New tab");
  });
});

describe("a narrow pane's head", () => {
  /** Draws the window with every pane `width` pixels wide, as the layout engine would. */
  async function aPaneOf(width: number) {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private readonly told: (entries: unknown[], seen: unknown) => void) {}
        observe() {
          this.told([], this);
        }
        disconnect() {}
        unobserve() {}
      },
    );
    const real = HTMLElement.prototype.getBoundingClientRect;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      return this.classList.contains("pane-frame")
        ? ({
            width,
            height: 400,
            top: 0,
            left: 0,
            right: width,
            bottom: 400,
            x: 0,
            y: 0,
          } as DOMRect)
        : real.call(this);
    });
    await aPane();
    await waitFor(() => expect(controls()[0]).toMatch(/^More for /));
  }

  it("keeps every part in a pane 520px wide", async () => {
    await aPaneOf(520);

    expect(head().querySelector(".pane-where")).not.toBeNull();
    expect(controls()).toContain("Split down");
  });

  it("drops harness · workspace first, and the bell, ⋯, Split right and Close stay", async () => {
    await aPaneOf(500);

    expect(head().querySelector(".pane-where")).toBeNull();
    expect(controls()).toContain("Split down");
  });

  it("drops Split down last, and still keeps the bell, ⋯, Split right and Close", async () => {
    await aPaneOf(280);

    expect(head().querySelector(".pane-where")).toBeNull();
    const [more, bell, right, close, ...rest] = controls();
    expect(more).toBe(`More for ${NAME}`);
    expect(bell).toMatch(/^Pane notices:/);
    expect(right).toBe("Split right");
    expect(close).toBeTruthy();
    expect(rest).toEqual([]);
  });
});
