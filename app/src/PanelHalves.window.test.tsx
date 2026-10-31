// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { forgetThisLaunch } from "./regions";
import { forgetTextSizes, setTextSize } from "./textSize";
import { sequenceIn } from "./tabSequence";

/**
 * **The Panel's halves by keyboard, by screen reader and in a narrow window** (spec #1736,
 * P-7, P-13, #1741): F6 from region to region, each half one Tab stop and a named region, and
 * one half at a time below about 900 px.
 */

vi.mock("./SessionPane", async (real) => {
  // The real module's keyboard rule, around a stand-in for xterm, as `Window.keyboard` has it.
  const actual = await real<typeof import("./SessionPane")>();
  return {
    ...actual,
    SessionPane: ({ session, focused }: { session: number; focused: boolean }) => (
      <div
        className={focused ? "pane focused" : "pane"}
        data-testid="pane"
        data-chat-keyboard=""
        onKeyDownCapture={actual.leavesTheChat}
      >
        <textarea aria-label={`Terminal ${session}`} tabIndex={0} />
      </div>
    ),
  };
});

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;
const CUT = `${ALPHA}/.worktrees/svc`;

const chat = (session: number, name: string, inFront = false) => ({
  session,
  name,
  cwd: ALPHA,
  harness: "claude",
  in_front: inFront,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: "steward",
  unreported: null,
  card: {
    name: "claude",
    title: "Claude Code",
    label: "What Claude Code can do here",
    lines: [],
    cannot_type: null,
  },
  guessed: null,
  pinned: false,
});

/** Every command the window sent, in order: what it wrote to layout.json among them. */
let asked: { cmd: string; args: unknown }[] = [];

/** One workspace with a clone, a worktree cut there, and one chat in front. */
function core() {
  const chats = [chat(1, "one", true)];
  asked = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "branch_tree") return { entries: [], more: 0 };
    if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
    if (cmd === "opened_chats") return chats;
    if (cmd === "plane_pins") return { project: false, workspaces: ["alpha"], missing: [] };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats }],
      };
    if (cmd === "workspace_panels")
      return {
        workspace: a.workspace,
        repos: ["svc"],
        paths: { svc: `${ALPHA}/svc` },
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
      };
    if (cmd === "workspace_repos")
      return { workspace: a.workspace, repos: [], cache_refused: null };
    if (cmd === "worktree_list")
      return [{ piece: "one", path: `${CUT}/one`, branch: "one", wired: true, stale: false }];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
}

const WIDE = window.innerWidth;

/** The window `width` CSS pixels wide, as a person resizing it leaves it. */
function sized(width: number) {
  act(() => {
    Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
    window.dispatchEvent(new Event("resize"));
  });
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
  forgetTextSizes();
});
afterEach(() => {
  cleanup();
  clearMocks();
  Object.defineProperty(window, "innerWidth", { configurable: true, value: WIDE });
});

/** A half of the Panel, as a screen reader finds it: a region named for its side. */
const half = (side: "left" | "right") =>
  screen.getByRole("region", { name: `Panel, ${side} half` });
const queryHalf = (side: "left" | "right") =>
  screen.queryByRole("region", { name: `Panel, ${side} half` });
const lowerTab = (side: "left" | "right", name: string) =>
  within(screen.getByRole("tablist", { name: `Panel, ${side} half` })).getByRole("tab", { name });

/** The window launched, its chat's pane and the Repos tree drawn. */
async function launched() {
  core();
  render(<App />);
  await screen.findByLabelText("Terminal 1");
  await screen.findByTestId("piece-svc-one");
}

/** What WebKit would stop at inside `scope`, drawn. */
const stopsIn = (scope: HTMLElement) =>
  sequenceIn(scope).filter((el) => el.closest("[hidden]") === null);

const f6 = (shiftKey = false) =>
  fireEvent.keyDown(document.activeElement ?? document.body, { key: "F6", shiftKey });

describe("each half of the Panel, to a screen reader", () => {
  it("is a region named for its side, holding the view open in it", async () => {
    await launched();

    expect(within(half("left")).getByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
    expect(within(half("right")).getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
  });

  it("is no region while it is away", async () => {
    await launched();

    await userEvent.click(lowerTab("left", "Repos"));

    expect(queryHalf("left")).toBeNull();
    expect(half("right")).toBeInTheDocument();
  });
});

describe("each half of the Panel, to the keyboard", () => {
  it("is one Tab stop, its tree's current row, and the arrows move inside the tree", async () => {
    await launched();
    const stops = stopsIn(half("left"));

    expect(stops).toHaveLength(1);
    expect(stops[0]).toHaveAttribute("role", "row");
    // The chevron on the half's line is the pointer's: its Lower icon and ⌘J are the keyboard's.
    expect(
      within(half("left")).getByRole("button", { name: "Put the panel's left half away" }),
    ).toHaveAttribute("tabindex", "-1");

    stops[0].focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() =>
      expect(document.activeElement).toHaveAccessibleName(expect.stringMatching(/^svc/)),
    );
    expect(half("left")).toContainElement(document.activeElement as HTMLElement);
  });
});

describe("F6 (P-13)", () => {
  /** Which region the keyboard is in, by what holds it. */
  function where(): string {
    const at = document.activeElement as HTMLElement;
    if (queryHalf("left")?.contains(at)) return "bottom-left";
    if (queryHalf("right")?.contains(at)) return "bottom-right";
    if (at.closest(".region-centre")) return "editor";
    if (at.closest(".slot-left")) return "left sidebar";
    if (at.closest(".slot-right")) return "right sidebar";
    return `elsewhere: ${at.tagName}`;
  }

  it("cycles left sidebar, editor, right sidebar, bottom-left, bottom-right, and round", async () => {
    await launched();
    // From outside every region (the left bar's open view), to the first.
    within(screen.getByRole("tablist", { name: "Navigation" }))
      .getByRole("tab", { name: "Chats" })
      .focus();

    const visited: string[] = [];
    for (let press = 0; press < 6; press++) {
      f6();
      visited.push(where());
    }

    expect(visited).toEqual([
      "left sidebar",
      "editor",
      "right sidebar",
      "bottom-left",
      "bottom-right",
      "left sidebar",
    ]);
  });

  it("goes back with Shift", async () => {
    await launched();
    within(screen.getByRole("tablist", { name: "Navigation" }))
      .getByRole("tab", { name: "Chats" })
      .focus();
    f6();

    const visited: string[] = [];
    for (let press = 0; press < 3; press++) {
      f6(true);
      visited.push(where());
    }

    expect(visited).toEqual(["bottom-right", "bottom-left", "right sidebar"]);
  });

  it("lands in a half on its view's Tab stop, and in the editor on the focused pane's controls", async () => {
    await launched();
    screen.getByRole("tab", { name: "Chats" }).focus();

    f6();
    f6();
    expect(document.activeElement).toHaveAccessibleName("What Claude Code can do here");
    f6();
    f6();
    expect(document.activeElement).toHaveAttribute("role", "row");
    expect(document.activeElement).toHaveAttribute("tabindex", "0");
  });

  // Two palette round trips typed key by key: past the default 5 s on a loaded CI runner.
  it(
    "is a palette row each way, which moves the keyboard as the key does (#1751)",
    { timeout: 20_000 },
    async () => {
      await launched();
      // In the left sidebar, as one F6 from the bar leaves it.
      screen.getByRole("tab", { name: "Chats" }).focus();
      f6();
      expect(where()).toBe("left sidebar");

      await userEvent.keyboard("{F2}");
      await screen.findByRole("dialog", { name: "Command palette" });
      await userEvent.keyboard("Focus the next region");
      await userEvent.keyboard("{Enter}");

      await waitFor(() => expect(where()).toBe("editor"), { timeout: 5_000 });

      await userEvent.keyboard("{F2}");
      await screen.findByRole("dialog", { name: "Command palette" });
      await userEvent.keyboard("Focus the previous region");
      await userEvent.keyboard("{Enter}");

      await waitFor(() => expect(where()).toBe("left sidebar"), { timeout: 5_000 });
    },
  );

  it("skips a region that is away", async () => {
    await launched();
    await userEvent.click(lowerTab("left", "Repos"));
    screen.getByRole("tab", { name: "Chats" }).focus();

    const visited: string[] = [];
    for (let press = 0; press < 4; press++) {
      f6();
      visited.push(where());
    }

    expect(visited).toEqual(["left sidebar", "editor", "right sidebar", "bottom-right"]);
  });

  it("is the chat's inside its terminal, which sends it on as a function key", async () => {
    await launched();
    const terminal = screen.getByLabelText("Terminal 1");
    terminal.focus();

    const key = new KeyboardEvent("keydown", { key: "F6", bubbles: true, cancelable: true });
    terminal.dispatchEvent(key);

    expect(key.defaultPrevented).toBe(false);
    expect(terminal).toHaveFocus();
  });
});

describe("a narrow window (P-7)", () => {
  it("draws one half of the Panel at a time, and the other side's Lower icons switch it", async () => {
    sized(800);
    await launched();

    // The left half first: neither has been used yet.
    expect(half("left")).toBeInTheDocument();
    expect(queryHalf("right")).toBeNull();
    expect(lowerTab("right", "Memory")).toHaveAttribute("aria-selected", "false");

    await userEvent.click(lowerTab("right", "Memory"));

    expect(within(half("right")).getByRole("tabpanel", { name: "Memory" })).toBeInTheDocument();
    expect(queryHalf("left")).toBeNull();
    expect(lowerTab("left", "Repos")).toHaveAttribute("aria-selected", "false");

    // And back: the left half was never put away, only not drawn.
    await userEvent.click(lowerTab("left", "Repos"));
    expect(half("left")).toBeInTheDocument();
    expect(queryHalf("right")).toBeNull();
  });

  it("draws the half a view's key opened", async () => {
    sized(800);
    await launched();
    await userEvent.click(lowerTab("right", "Todos"));
    expect(queryHalf("left")).toBeNull();

    // ⌃⇧G: Changes, which lives in the left half (#1739).
    fireEvent.keyDown(document.body, { key: "G", ctrlKey: true, shiftKey: true });

    await waitFor(() =>
      expect(within(half("left")).getByRole("tabpanel", { name: "Changes" })).toBeInTheDocument(),
    );
    expect(queryHalf("right")).toBeNull();
  });

  it("draws both halves again, as they were, once the window is wide", async () => {
    sized(800);
    await launched();
    await userEvent.click(lowerTab("right", "Todos"));

    sized(1280);

    await waitFor(() => expect(half("left")).toBeInTheDocument());
    expect(within(half("left")).getByRole("tabpanel", { name: "Repos" })).toBeInTheDocument();
    expect(within(half("right")).getByRole("tabpanel", { name: "Todos" })).toBeInTheDocument();
  });

  it("writes nothing to layout.json for it: narrowing, switching halves and widening again", async () => {
    await launched();
    const writes = () => asked.filter((one) => one.cmd === "write_layout").length;
    const before = writes();

    sized(800);
    await waitFor(() => expect(queryHalf("right")).toBeNull());
    // The right half's open view: drawn in the left's place, and nothing put away.
    await userEvent.click(lowerTab("right", "Memory"));
    expect(queryHalf("left")).toBeNull();
    sized(1280);
    await waitFor(() => expect(half("left")).toBeInTheDocument());
    expect(half("right")).toBeInTheDocument();

    expect(writes()).toBe(before);
  });

  it("is one at the narrowest window once the window text is big enough (ADR 0054)", async () => {
    // The window is never under 1024 px; at 16 px of window text it lays out as 896 px would.
    sized(1024);
    await launched();
    expect(half("left")).toBeInTheDocument();
    expect(half("right")).toBeInTheDocument();

    act(() => setTextSize("window", 16));
    await waitFor(() => expect(queryHalf("right")).toBeNull());
    expect(half("left")).toBeInTheDocument();

    act(() => setTextSize("window", 14));
    await waitFor(() => expect(half("right")).toBeInTheDocument());
  });

  it("leaves the half it does not draw out of F6", async () => {
    sized(800);
    await launched();
    screen.getByRole("tab", { name: "Chats" }).focus();

    f6(true);

    expect(half("left")).toContainElement(document.activeElement as HTMLElement);
    f6();
    expect(document.activeElement?.closest(".slot-left")).not.toBeNull();
  });
});
