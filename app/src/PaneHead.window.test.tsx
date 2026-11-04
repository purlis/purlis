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
import { onTheWire } from "./chatEvents.testkit";
import { aTabForEachChat } from "./laidOut.testkit";
import { placementFor } from "./ui/tooltip/Tooltip";

/**
 * **A chat's pane head, against the whole window** (#4760, spec #4750 stories 54, 55, 60 and
 * 61; the accepted prototype `prototype/pane-chat-actions`, variant A).
 *
 * The head shows the chat at a glance (its name, its state, "harness · workspace" and its
 * sandbox) and then, at its end, two groups (#5203, spec #5191): the chat's actions, its one
 * state button, the bell and ⋯ last, then, after a thin divider, the pane's controls, Split
 * right, Split down and Close last. Its right-click is the chat's list, then the pane's rows. A
 * narrow pane drops its parts in the ruled order (`paneHead.ts`), and the bell, ⋯, Split right
 * and Close never go.
 */

/** What the stand-in terminal was asked to do from its pane's head (#4760). */
const ran: string[] = [];

vi.mock("./SessionPane", async () => {
  const { useContext, useEffect } = await import("react");
  const { PaneTerminalAt } = await import("./paneTerminal");
  return {
    // A terminal with something selected, which says to its pane's head what it can do.
    SessionPane: ({ session }: { session: number }) => {
      const at = useContext(PaneTerminalAt);
      useEffect(() => {
        if (at === null) return;
        at.current = { selected: () => true, run: (id) => ran.push(id) };
        return () => {
          at.current = null;
        };
      }, [at]);
      return <div data-testid="pane">session {session}</div>;
    },
  };
});

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
      // The chat in a tab of its own on alpha, as the stored layout keeps it (#4758).
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
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
      if (cmd === "smart_closing") return [];
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
    // Led by the chat's persona and state icon, the one its row and tab wear (#4762).
    const line = head().querySelector(".pane-chips") as HTMLElement;
    expect(line.firstElementChild).toHaveClass("pane-persona");
    expect(line.firstElementChild?.querySelector(".chat-persona")).not.toBeNull();
    expect(head().querySelector(".pane-where")).toHaveTextContent(/^claude · alpha$/);
    // The badge moved from the tab to the head (#4760): named for a screen reader.
    expect(within(head()).getByRole("img", { name: "floor" })).toBeInTheDocument();
    const strip = document.querySelector('[data-strip="Tabs"]') as HTMLElement;
    expect(strip.querySelector(".tab-sandbox")).toBeNull();
  });

  it("and its row in the Chats list says the same sandbox (#4760)", async () => {
    await aPane();

    const tree = await screen.findByRole("tree", { name: "Chats of this project" });
    const row = await within(tree).findByRole("treeitem", { name: new RegExp(`^${NAME}`) });
    expect(within(row).getByRole("img", { name: "floor" })).toBeInTheDocument();
  });

  it("ends with the bell and ⋯, a divider, then Split right, Split down and Close (#5203)", async () => {
    await aPane();

    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    const [bell, more, right, down, close, ...rest] = controls();
    expect(bell).toMatch(/^Pane notices:/);
    expect(more).toBe(`More for ${NAME}`);
    expect([right, down]).toEqual(["Split right", "Split down"]);
    expect(close).toBeTruthy();
    expect(rest).toEqual([]);
    // The thin divider parts the chat's actions from the pane's controls, unheard and untabbed.
    const actions = [
      ...head().querySelectorAll(".pane-actions button, .pane-actions [data-scope]"),
    ];
    const line = actions.findIndex((one) => one.getAttribute("data-scope") === "separator");
    expect(actions[line - 1]).toHaveAccessibleName(`More for ${NAME}`);
    expect(actions[line + 1]).toHaveAccessibleName("Split right");
    expect(actions[line]).toHaveAttribute("aria-hidden", "true");
  });

  it("draws the bell as an icon button, and opens every head hint below the head (#5189)", async () => {
    await aPane();
    await waitFor(() => expect(controls()[0]).toMatch(/^Pane notices:/));

    const bell = within(head()).getByRole("button", { name: /^Pane notices:/ });
    // The size of Split right beside it: an icon button, as wide as it is tall.
    expect(bell).toHaveAttribute("data-icon-only");
    expect(bell).toHaveAttribute("data-size", "sm");
    // Below the head: a hint at a control's end stood on the row, over the next control.
    for (const one of within(head()).getAllByRole("button"))
      expect(placementFor(one)).toBe("bottom");
  });

  it("shows Stop while the chat works, first of the chat's actions, and nothing once it is quiet", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

    await act(() => emit(...onTheWire("chat-moved", RUNNING)));
    const stop = await within(head()).findByRole("button", { name: `Stop chat ${NAME}` });
    expect(stop).toHaveTextContent("Stop");
    expect(controls()[0]).toBe(`Stop chat ${NAME}`);
    expect(controls()[1]).toMatch(/^Pane notices:/);
    expect(controls()[2]).toBe(`More for ${NAME}`);

    await act(() => emit(...onTheWire("chat-moved", WAITING)));
    await waitFor(() =>
      expect(within(head()).queryByRole("button", { name: `Stop chat ${NAME}` })).toBeNull(),
    );
    expect(controls()[1]).toBe(`More for ${NAME}`);
  });

  it("opens the chat's list, then a line, then the pane's rows, on a right-click", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

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

  it("puts the terminal's Copy and Paste among the pane's rows, before Send F2, and runs them on it", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
    ran.length = 0;

    fireEvent.contextMenu(head().querySelector(".pane-chips") as HTMLElement);
    const menu = await screen.findByRole("menu");
    const items = [...menu.querySelectorAll('[role^="menuitem"]')].map(
      (one) => one.getAttribute("aria-label") ?? one.textContent,
    );
    const split = items.indexOf("Split right");
    expect(items.slice(split)).toEqual(
      expect.arrayContaining(["Copy", "Paste", "Send F2 to the chat in front"]),
    );
    expect(items.indexOf("Copy")).toBeGreaterThan(items.indexOf("Split down"));
    expect(items.indexOf("Paste")).toBe(items.indexOf("Copy") + 1);
    expect(items.indexOf("Send F2 to the chat in front")).toBeGreaterThan(items.indexOf("Paste"));
    // Not Select all: the terminal's own right-click keeps that.
    expect(items).not.toContain("Select all");

    fireEvent.click(within(menu).getByRole("menuitem", { name: "Copy" }));
    await waitFor(() => expect(ran).toEqual(["terminal.copy"]));
  });
});

describe("a chat wrapping up (#4760)", () => {
  /** The core telling the window a step of chat 4's smart close. */
  const step = (phase: string) =>
    act(() => emit("smart-close", { plane: PLANE, session: 4, phase, record: null }));

  it("keeps its pane, whose head breathes and offers Cancel smart close, and turns no tab into a chip", async () => {
    await aPane();
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));

    await step("sent");

    await waitFor(() =>
      expect(head().querySelector('[data-mark="wrapping-up"]')).toHaveClass("breathing"),
    );
    // The head's one state button, first of its actions, and the pane is still the chat's.
    expect(controls()[0]).toBe(`Cancel smart close of ${NAME}`);
    expect(screen.getByTestId("pane")).toHaveTextContent("session 4");
    // The strip's chip is gone (#4760): the chat's tab stays a tab, with its name.
    const strip = document.querySelector('[data-strip="Tabs"]') as HTMLElement;
    expect(strip.querySelector(".tab[data-chip]")).toBeNull();
    expect([...strip.querySelectorAll(".tab-name")].map((name) => name.textContent)).toContain(
      NAME,
    );

    await step("cancelled");
    await waitFor(() => expect(head().querySelector('[data-mark="wrapping-up"]')).toBeNull());
    expect(controls()[1]).toBe(`More for ${NAME}`);
  });

  it("breathes on its row in the Chats list, and the mark goes when it is cancelled", async () => {
    await aPane();
    const tree = await screen.findByRole("tree", { name: "Chats of this project" });
    const row = await within(tree).findByRole("treeitem", { name: new RegExp(`^${NAME}`) });
    expect(row.querySelector('[data-mark="wrapping-up"]')).toBeNull();

    await step("sent");

    await waitFor(() => expect(row.querySelector('[data-mark="wrapping-up"]')).not.toBeNull());
    expect(row.querySelector('[data-mark="wrapping-up"]')).toHaveClass("breathing");

    await step("cancelled");
    await waitFor(() => expect(row.querySelector('[data-mark="wrapping-up"]')).toBeNull());
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
    await waitFor(() => expect(controls()[1]).toMatch(/^More for /));
  }

  it("keeps every part in a pane 520px wide", async () => {
    await aPaneOf(520);

    expect(head().querySelector(".pane-where")).not.toBeNull();
    expect(within(head()).getByRole("img", { name: "floor" })).toBeInTheDocument();
    expect(controls()).toContain("Split down");
  });

  it("drops harness · workspace first, and the bell, ⋯, Split right and Close stay", async () => {
    await aPaneOf(500);

    expect(head().querySelector(".pane-where")).toBeNull();
    // The sandbox goes with it: it says how the chat runs, as "harness · workspace" does.
    expect(within(head()).queryByRole("img", { name: "floor" })).toBeNull();
    expect(controls()).toContain("Split down");
  });

  it("drops Split down last, and still keeps the bell, ⋯, Split right and Close", async () => {
    await aPaneOf(280);

    expect(head().querySelector(".pane-where")).toBeNull();
    const [bell, more, right, close, ...rest] = controls();
    expect(bell).toMatch(/^Pane notices:/);
    expect(more).toBe(`More for ${NAME}`);
    expect(right).toBe("Split right");
    expect(close).toBeTruthy();
    expect(rest).toEqual([]);
  });
});
