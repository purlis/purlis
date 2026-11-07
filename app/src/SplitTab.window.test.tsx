// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidOut, LaidTab, Moved, OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";

/**
 * **A split tab, against the whole window** (#5342, spec #5339; `prototype/tab-faces`, its
 * scenarios "A split tab", "A pane needs you, but another is focused" and "Splitting doesn't
 * rename the tab"): the tab keeps its first chat's name whichever pane has the keyboard, takes
 * the next chat's when its first closes (#5308), and wears its most urgent pane's mark.
 */

vi.mock("./SessionPane", async (original) => {
  const real = await original<typeof import("./SessionPane")>();
  const { CHAT_KEYBOARD } = await import("./actions");
  return {
    ...real,
    SessionPane: ({ session, focused }: { session: number; focused?: boolean }) => (
      <div
        className={focused ? "pane focused" : "pane"}
        data-testid="pane"
        data-session={session}
        {...{ [CHAT_KEYBOARD]: "" }}
      >
        session {session}
      </div>
    ),
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);
const PLANE = "/home/dev/plane";

function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: `${PLANE}/workspaces/ide`,
    harness: null,
    in_front: false,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/** What the board says of a chat: `waiting` with `needs_you` is its turn ended on the person. */
function moved(session: number, state: string, needsYou = false): Moved {
  return {
    plane: PLANE,
    session,
    state,
    needs_you: needsYou,
    queue: needsYou ? [session] : [],
    sequence: 1,
    moved_at: 1,
    reports: [],
    refusals: [],
    children: [],
  } as unknown as Moved;
}

function core(open: OpenChat[], laid: LaidOut, states: Moved[] = []) {
  mockIPC((cmd) => {
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open;
    // Each chat has its row, as the plane lists it: what a tab is named from.
    if (cmd === "plane_sidebar")
      return { root: PLANE, personas: [], persona: null, unfiled: open, workspaces: [] };
    if (cmd === "reopened_layout") return laid;
    if (cmd === "chat_states") return states;
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
}

const tab = (tree: LaidTab["tree"], focused: string): LaidTab => ({
  workspace: null,
  pinned: false,
  window: 0,
  front: true,
  focused,
  tree,
});

/** 1 on the left, 2 on the right, as a split tab is recorded; `focused` is a pane's place. */
const twoPanes = (focused: string): LaidOut => ({
  strip: null,
  tabs: [
    tab(
      {
        kind: "split",
        direction: "row",
        share: 500,
        a: { kind: "chat", chat: 1 },
        b: { kind: "chat", chat: 2 },
      },
      focused,
    ),
  ],
});

const theTab = () => within(stripNamed("Tabs")).getAllByRole("tab")[0];
const tabName = () => theTab().querySelector(".tab-name")?.textContent;
/** The state the tab's mark wears (`ui/chat-mark`'s `data-variant`). */
const tabMark = () =>
  theTab()
    .querySelector('[data-scope="chat-mark"][data-part="root"]')
    ?.getAttribute("data-variant");
const focusedSession = () => document.querySelector(".pane.focused")?.getAttribute("data-session");

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a split tab's name (#5342)", () => {
  it("is its first chat's while the second pane has the keyboard", async () => {
    core([chat(1), chat(2)], twoPanes("b"));
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("2"));

    expect(tabName()).toBe("ide.1");
  });

  it("stays its first chat's as the keyboard moves between the panes", async () => {
    core([chat(1), chat(2)], twoPanes("a"));
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));
    expect(tabName()).toBe("ide.1");

    await userEvent.keyboard("{Alt>}{ArrowRight}{/Alt}");
    await waitFor(() => expect(focusedSession()).toBe("2"));
    expect(tabName()).toBe("ide.1");
    await userEvent.keyboard("{Alt>}{ArrowLeft}{/Alt}");
    await waitFor(() => expect(focusedSession()).toBe("1"));
    expect(tabName()).toBe("ide.1");
  });

  it("is the next chat's once its first pane closes, never the closed chat's (#5308)", async () => {
    core([chat(1), chat(2)], twoPanes("a"));
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    const first = screen.getByRole("region", { name: "Left pane" });
    await userEvent.click(within(first).getByRole("button", { name: "Close this pane" }));

    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(1));
    expect(tabName()).toBe("ide.2");
    // Its pane's head names the chat it shows, as the tab does.
    expect(document.querySelector(".pane-head .pane-name")).toHaveTextContent("ide.2");
  });
});

describe("a split tab's mark (#5342)", () => {
  it("says the second pane needs you while the first, idle, has the keyboard", async () => {
    core([chat(1), chat(2)], twoPanes("a"), [moved(1, "waiting"), moved(2, "waiting", true)]);
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    await waitFor(() => expect(tabMark()).toBe("needs"));
    // The name stays the first chat's: the mark, not the name, says which pane asks.
    expect(tabName()).toBe("ide.1");
  });

  it("is the focused pane's own where no other pane outranks it", async () => {
    core([chat(1), chat(2)], twoPanes("b"), [moved(1, "waiting"), moved(2, "running")]);
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("2"));

    await waitFor(() => expect(tabMark()).toBe("working"));
  });
});

describe("a split tab's picture (#5342)", () => {
  /** The lines the tab's picture draws between its panes, each as `x1 y1 x2 y2`. */
  /** The tiles the tab's picture draws, one for each pane, each as `x y width height`. */
  const pictureLines = () =>
    [...theTab().querySelectorAll('[data-scope="tabs"][data-part="picture"] rect')].map((rect) =>
      ["x", "y", "width", "height"].map((at) => rect.getAttribute(at)).join(" "),
    );

  it("is drawn after the name in place of +1: two tiles side by side for two side by side", async () => {
    core([chat(1), chat(2)], twoPanes("a"));
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    expect(pictureLines()).toEqual(["2 2 5.25 12", "8.75 2 5.25 12"]);
    expect(theTab()).not.toHaveTextContent("+1");
    // To a screen reader the picture is words: how many panes the tab holds.
    expect(theTab()).toHaveTextContent("ide.1, split, 2 panes");
  });

  it("is gone once the tab is down to one pane", async () => {
    core([chat(1), chat(2)], twoPanes("a"));
    render(<App />);
    await waitFor(() => expect(focusedSession()).toBe("1"));

    const second = screen.getByRole("region", { name: "Right pane" });
    await userEvent.click(within(second).getByRole("button", { name: "Close this pane" }));

    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(1));
    expect(pictureLines()).toEqual([]);
    expect(theTab()).not.toHaveTextContent("split");
  });
});
