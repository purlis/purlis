// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { LaidNode, LaidOut } from "./bindings";
import { stripNamed } from "./test-strips";

/**
 * **The strip's `+`, its list and the keys, against the whole window** (#5292; spec #5288
 * stories 73 to 89, the prototype's "The + button" scenarios). How the list opens and closes
 * under the pointer is `NewTabButton.test.tsx`'s; this is what each way of opening leaves: a
 * press is an empty tab, each row of the list a new tab of its own, ⌘T and ⌘N the keys, and the
 * empty pane's own buttons fill it.
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
const SHELL = 30;

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

const CHATS = [chat(4), chat(5)];

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

const ONE_PANE = laidOut({ kind: "chat", chat: 4 });
const WITH_EMPTY = laidOut({
  kind: "split",
  direction: "row",
  share: 500,
  a: { kind: "chat", chat: 4 },
  b: { kind: "empty" },
});

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
      if (cmd === "open_session") return SHELL;
      return null;
    },
    { shouldMockEvents: true },
  );
}

/** The sessions the panes on screen show, left to right. */
const onScreen = () =>
  screen.queryAllByTestId("pane").map((pane) => Number(pane.getAttribute("data-session")));
const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".session")?.textContent ?? tab.textContent);
const tabCount = () => within(stripNamed("Tabs")).queryAllByRole("tab").length;
const plus = () => screen.getByRole("button", { name: "New tab" });

async function drawn(laid: LaidOut, panes: number[]) {
  core(laid);
  render(<App />);
  await waitFor(() => expect(onScreen()).toEqual(panes));
}

/** Answers the picker a New chat opens. */
async function startIt() {
  const dialog = await screen.findByRole("dialog", { name: "Start a chat" });
  await userEvent.click(await within(dialog).findByRole("button", { name: /^Start chat/ }));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull());
}

/** Opens the `+` list from the keyboard, as Down on `+` does. */
async function theList() {
  plus().focus();
  await userEvent.keyboard("{ArrowDown}");
  return screen.findByRole("menu", { name: "Open in a new tab" });
}

async function choose(row: RegExp) {
  const list = await theList();
  await userEvent.click(within(list).getByRole("menuitem", { name: row }));
}

describe("pressing + (#5292; stories 73, 86 and 88)", () => {
  it("opens an empty tab that asks what goes there, and starts no chat", async () => {
    await drawn(ONE_PANE, [4]);

    await userEvent.click(plus());

    await waitFor(() => expect(tabCount()).toBe(2));
    expect(onScreen()).toEqual([]);
    expect(screen.getByText("What goes here?")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Start a chat" })).toBeNull();
  });

  it("names the tab Empty tab until a chat fills it", async () => {
    await drawn(ONE_PANE, [4]);
    await userEvent.click(plus());
    await waitFor(() => expect(tabNames()).toContain("Empty tab"));

    await userEvent.click(
      within(screen.getByTestId("empty-pane")).getByRole("button", { name: "New chat" }),
    );
    await startIt();

    await waitFor(() => expect(onScreen()).toEqual([STARTED]));
    expect(tabNames()).not.toContain("Empty tab");
  });

  it("closes an empty tab without asking", async () => {
    await drawn(ONE_PANE, [4]);
    await userEvent.click(plus());
    await waitFor(() => expect(tabCount()).toBe(2));

    await userEvent.click(
      within(stripNamed("Tabs")).getByRole("button", { name: "Close Empty tab" }),
    );

    await waitFor(() => expect(tabCount()).toBe(1));
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
});

describe("the + list's rows, each a new tab (#5292, the call from #5297)", () => {
  it("Empty tab is what + itself opens", async () => {
    await drawn(ONE_PANE, [4]);

    await choose(/Empty tab/);

    await waitFor(() => expect(tabCount()).toBe(2));
    expect(screen.getByText("What goes here?")).toBeInTheDocument();
  });

  it("New chat gets a tab of its own, even with an empty pane in front", async () => {
    await drawn(WITH_EMPTY, [4]);

    await choose(/New chat/);
    await startIt();

    await waitFor(() => expect(onScreen()).toEqual([STARTED]));
    expect(tabCount()).toBe(2);
    await userEvent.click(within(stripNamed("Tabs")).getAllByRole("tab")[0]);
    await waitFor(() => expect(onScreen()).toEqual([4]));
    expect(screen.getByTestId("empty-pane")).toBeInTheDocument();
  });

  it("New shell gets a tab of its own, even with an empty pane in front", async () => {
    await drawn(WITH_EMPTY, [4]);

    await choose(/New shell/);

    await waitFor(() => expect(onScreen()).toEqual([SHELL]));
    expect(tabCount()).toBe(2);
  });

  it("a view opens in a tab of its own", async () => {
    await drawn(ONE_PANE, [4]);

    await choose(/Keyboard shortcuts/);

    await waitFor(() => expect(tabNames()).toContain("Keyboard shortcuts"));
    expect(tabCount()).toBe(2);
  });
});

describe("the keys (#5292; stories 82 and 83)", () => {
  it("⌘T opens an empty tab, from a chat's pane too", async () => {
    vi.stubGlobal("navigator", { ...navigator, platform: "MacIntel" });
    await drawn(ONE_PANE, [4]);

    fireEvent.keyDown(screen.getByTestId("pane"), { key: "t", metaKey: true });

    await waitFor(() => expect(tabCount()).toBe(2));
    expect(screen.getByText("What goes here?")).toBeInTheDocument();
  });

  it("⌘N asks for a new chat, which fills an empty pane first", async () => {
    vi.stubGlobal("navigator", { ...navigator, platform: "MacIntel" });
    await drawn(WITH_EMPTY, [4]);

    fireEvent.keyDown(window, { key: "n", metaKey: true });
    await startIt();

    await waitFor(() => expect(onScreen()).toEqual([4, STARTED]));
    expect(tabCount()).toBe(1);
  });
});

describe("an empty pane's own buttons (#5292; stories 84 and 89)", () => {
  it("New shell fills that pane", async () => {
    await drawn(WITH_EMPTY, [4]);

    await userEvent.click(
      within(screen.getByTestId("empty-pane")).getByRole("button", { name: "New shell" }),
    );

    await waitFor(() => expect(onScreen()).toEqual([4, SHELL]));
    expect(tabCount()).toBe(1);
  });
});
