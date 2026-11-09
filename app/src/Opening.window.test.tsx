// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidNode, LaidOut } from "./bindings";
import { stripNamed } from "./test-strips";
import { findTree, rowsOf } from "./ui/tree/tree.testkit";

/**
 * **Where things open, against the whole window** (#5289; spec #5288 stories 66 to 71). The rule
 * is `tabs.opens.test.ts`; this says the window's paths use it: the chat list's New chat, the
 * same button by the keyboard, the palette's row, an empty pane's own New chat, and a pressed
 * row. The prototype's "Opening a chat" scenarios are the list.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" data-session={session}>
      session {session}
    </div>
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
const STARTED = 20;

const chat = (session: number) => ({
  session,
  name: String(session),
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: session === 4,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: "steward",
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
  sandbox: null,
  sandbox_why: null,
  browser_sites: null,
});

/** Chats 4, 5 and 6 work in alpha. */
const CHATS = [chat(4), chat(5), chat(6)];

const FIRST = {
  profiles: [
    {
      name: "work",
      kind: "claude",
      shown: "claude",
      source: "charter.local.toml",
      is_default: true,
      approval: null,
      ready_to_type: true,
      harness: null,
      sandbox: null,
      model: "the harness's default",
      billed_to: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  ignore_fix_id: null,
  declares_none: false,
};

const laidOut = (tree: LaidNode): LaidOut => ({
  strip: "alpha",
  tabs: [{ workspace: "alpha", pinned: false, window: 0, front: true, focused: "", tree }],
});

const split = (a: LaidNode, b: LaidNode): LaidNode => ({
  kind: "split",
  direction: "row",
  share: 500,
  a,
  b,
});

const ONE_PANE = laidOut({ kind: "chat", chat: 4 });
const SPLIT = laidOut(split({ kind: "chat", chat: 4 }, { kind: "chat", chat: 5 }));
const WITH_EMPTY = laidOut(split({ kind: "chat", chat: 4 }, { kind: "empty" }));
const TWO_EMPTY = laidOut(split({ kind: "empty" }, { kind: "empty" }));

function core(laid: LaidOut) {
  mockIPC(
    (cmd) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return CHATS;
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: "steward",
          unfiled: [],
          workspaces: [
            { name: "alpha", path: ALPHA, vision: "", todos: [], chats: CHATS, colour: null },
          ],
        };
      if (cmd === "reopened_layout") return laid;
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks: [] };
      if (cmd === "mcp_failures") return [];
      if (cmd === "smart_closing") return [];
      if (cmd === "start_options_first") return FIRST;
      if (cmd === "start_details")
        return [{ name: "work", approval: null, sandbox: null, billed_to: null }];
      if (cmd === "start_chat") return { session: STARTED, label: null };
      return null;
    },
    { shouldMockEvents: true },
  );
}

/** The sessions the panes on screen show, left to right. */
const onScreen = () =>
  screen.queryAllByTestId("pane").map((pane) => Number(pane.getAttribute("data-session")));

const tabCount = () => within(stripNamed("Tabs")).queryAllByRole("tab").length;

async function drawn(laid: LaidOut, panes: number[]) {
  core(laid);
  render(<App />);
  await waitFor(() => expect(onScreen()).toEqual(panes));
  return findTree("Chats of this project");
}

const row = (tree: HTMLElement, name: string) => {
  const found = rowsOf(tree).find((one) => one.querySelector(".session")?.textContent === name);
  if (found === undefined) throw new Error(`no row is named ${name}`);
  return found;
};

/** The chat list's own New chat, the button at the top of the list. */
const listNewChat = async () => {
  const buttons = await screen.findAllByRole("button", { name: "New chat" });
  const found = buttons.find((one) => one.closest('[data-testid="empty-pane"]') === null);
  if (found === undefined) throw new Error("the chat list has no New chat");
  return found;
};

/** Answers the picker that New chat opens. */
async function startIt() {
  const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
  await userEvent.click(await within(dialog).findByRole("button", { name: /^Start chat/ }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull());
}

/** The ways of asking for a new chat, each of which must use the one rule. */
const WAYS: [string, () => Promise<void>][] = [
  ["the chat list's button", async () => userEvent.click(await listNewChat())],
  [
    "the keyboard, on that button",
    async () => {
      (await listNewChat()).focus();
      await userEvent.keyboard("{Enter}");
    },
  ],
  [
    "the keyboard, through the palette",
    async () => {
      await userEvent.keyboard("{F2}");
      await screen.findByRole("dialog", { name: "Command palette" });
      await userEvent.keyboard("New chat");
      await screen.findByRole("option", { name: /^New chat(?! at| in)/ });
      await userEvent.keyboard("{Enter}");
    },
  ],
  [
    "the palette",
    async () => {
      await userEvent.keyboard("{F2}");
      await userEvent.click(await screen.findByRole("option", { name: /^New chat(?! at| in)/ }));
    },
  ],
];

describe("a new chat (#5289)", () => {
  describe.each(WAYS)("by %s", (_, ask) => {
    it("in a split, leaves both panes alone and opens a tab of its own", async () => {
      await drawn(SPLIT, [4, 5]);
      expect(tabCount()).toBe(1);

      await ask();
      await startIt();

      await waitFor(() => expect(onScreen()).toEqual([STARTED]));
      expect(tabCount()).toBe(2);
      await userEvent.click(within(stripNamed("Tabs")).getAllByRole("tab")[0]);
      await waitFor(() => expect(onScreen()).toEqual([4, 5]));
    });

    it("in one pane, opens a tab of its own and replaces nothing", async () => {
      await drawn(ONE_PANE, [4]);

      await ask();
      await startIt();

      await waitFor(() => expect(onScreen()).toEqual([STARTED]));
      expect(tabCount()).toBe(2);
      await userEvent.click(within(stripNamed("Tabs")).getAllByRole("tab")[0]);
      await waitFor(() => expect(onScreen()).toEqual([4]));
    });

    it("fills an empty pane of the tab, and adds no tab", async () => {
      await drawn(WITH_EMPTY, [4]);

      await ask();
      await startIt();

      await waitFor(() => expect(onScreen()).toEqual([4, STARTED]));
      expect(tabCount()).toBe(1);
    });
  });

  it("fills the empty pane whose own New chat was pressed, of two", async () => {
    await drawn(TWO_EMPTY, []);
    const [left, right] = screen.getAllByTestId("empty-pane");

    await userEvent.click(within(right).getByRole("button", { name: "New chat" }));
    await startIt();

    await waitFor(() => expect(onScreen()).toEqual([STARTED]));
    expect(tabCount()).toBe(1);
    expect(left.isConnected).toBe(true);
    expect(screen.getAllByTestId("empty-pane")).toEqual([left]);
  });
});

describe("a pressed chat (#5289)", () => {
  it("that's on screen goes to its pane and adds nothing", async () => {
    const tree = await drawn(SPLIT, [4, 5]);

    await userEvent.click(row(tree, "steward 4"));

    expect(onScreen()).toEqual([4, 5]);
    expect(tabCount()).toBe(1);
    await waitFor(() => expect(row(tree, "steward 4").getAttribute("aria-current")).toBe("true"));
  });

  it("off screen, in one pane, shows there, and the old chat stays in the list", async () => {
    const tree = await drawn(ONE_PANE, [4]);

    await userEvent.click(row(tree, "steward 6"));

    await waitFor(() => expect(onScreen()).toEqual([6]));
    expect(tabCount()).toBe(1);
    expect(row(tree, "steward 4").getAttribute("aria-current")).toBeNull();
  });

  it("off screen, in a split, opens a tab, and the split survives", async () => {
    const tree = await drawn(SPLIT, [4, 5]);

    await userEvent.click(row(tree, "steward 6"));

    await waitFor(() => expect(onScreen()).toEqual([6]));
    expect(tabCount()).toBe(2);
    await userEvent.click(within(stripNamed("Tabs")).getAllByRole("tab")[0]);
    await waitFor(() => expect(onScreen()).toEqual([4, 5]));
  });

  it("off screen, with an empty pane in the tab, fills it", async () => {
    const tree = await drawn(WITH_EMPTY, [4]);

    await userEvent.click(row(tree, "steward 6"));

    await waitFor(() => expect(onScreen()).toEqual([4, 6]));
    expect(tabCount()).toBe(1);
  });
});
