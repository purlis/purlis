// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { McpFailed, Shown } from "./bindings";
import { forgetInboxOpen, inboxAwayAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";

/**
 * **A pane's notices, minimised and expanded, against the whole window** (#4754, spec #4750
 * stories 33 to 39, the accepted prototype `prototype/pane-notices`).
 *
 * One chat on screen, whose pane holds asks (a workspace it asks to have removed) and notes (an
 * MCP server that failed, in trouble; a sandbox block of its own work). Minimised, the default,
 * the notes fold into the bell's count in the pane's header and the asks stand; expanded, every
 * notice stands under a head row with Dismiss all; the choice is one for every pane, written to
 * the layout file. The cap of two asks on a pane, and its line about the rest waiting in the
 * Inbox, are gone.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

// A pane's Notice is listed in the Inbox too: these tests are about the pane's, so the side
// opens on another view.
// A new machine's layout file, which says nothing of the notices: minimised, the window's
// default (`paneNotices.ts`); a test's machine is otherwise handed them expanded (`test-inbox.ts`).
beforeEach(() => inboxAwayAtLaunch("minimised"));
afterEach(forgetInboxOpen);
afterEach(forgetInbox);
afterEach(() => {
  cleanup();
  clearMocks();
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
};

/** Chat 4 asks to have `workspace` removed (qw356): an ask, as the registry lists it. */
const removal = (workspace: string, since: number): Shown => ({
  session: 4,
  ask: `removal:4:${workspace}`,
  says: `Removing workspace ${workspace} deletes its memory, its todos and its clones`,
  options: [
    { id: "remove", label: "Remove…", allows: false },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: ["claude 4"],
  answer: { via: "workspace-removal", workspace, memories: 0, todos: 0 },
  since,
});

/** An MCP server of chat 4's that could not start: a note, in trouble. */
const FAILED: McpFailed = {
  plane: PLANE,
  session: 4,
  server: "figma",
  persona: null,
  error: "exited with code 1",
};

/** A block of chat 4's own work, which asks nothing: a note, in trouble. */
const BLOCKED = {
  plane: PLANE,
  session: 4,
  operation: "write",
  kind: "toolchain-cache",
  ours: false,
  harness: "claude",
  said: "a write to a toolchain's package cache",
  offer: "none",
  target: null,
  route: null,
  levels: [],
  held: false,
  ruled: null,
};

type Core = { asks: Shown[]; failed: McpFailed[] };

function core(now: Core) {
  const layouts: string[] = [];
  mockIPC(
    (cmd, args) => {
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
      if (cmd === "asks_waiting") return { plane: PLANE, asks: now.asks };
      if (cmd === "mcp_failures") return now.failed;
      if (cmd === "put_away_mcp_failure") {
        now.failed = [];
        return null;
      }
      if (cmd === "write_layout") layouts.push((args as { text: string }).text);
      return null;
    },
    { shouldMockEvents: true },
  );
  return { layouts };
}

async function aPane(now: Core) {
  const said = core(now);
  render(<App />);
  await screen.findByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return said;
}

/** The pane's frame, its header and its bell. */
const frame = () => screen.getByTestId("pane").closest(".pane-frame") as HTMLElement;
const bell = () => within(frame()).getByRole("button", { name: /^Pane notices:/ });
/** The Notices a person can see in the pane, top to bottom, by their names. */
const seen = () =>
  within(frame())
    .queryAllByRole("status")
    .map((line) => line.getAttribute("aria-label"))
    .filter((name) => name !== null);

describe("a pane's notices, minimised (the default)", () => {
  it("folds the notes into the bell's count and stands the asks as full cards", async () => {
    await aPane({ asks: [removal("beta", 10)], failed: [FAILED] });
    await act(() => emit("chat-mcp-failed", FAILED));

    await waitFor(() =>
      expect(bell()).toHaveAccessibleName("Pane notices: 1 notice, 1 in trouble, 1 asking"),
    );
    expect(bell()).toHaveTextContent(/^1$/);
    expect(bell()).toHaveAttribute("aria-expanded", "false");
    expect(bell()).toHaveAttribute("data-trouble");
    // The ask keeps its words and its answers, and has no Dismiss.
    expect(seen()).toEqual(["Remove workspace"]);
    const ask = within(frame()).getByRole("status", { name: "Remove workspace" });
    expect(within(ask).queryByRole("button", { name: "Dismiss" })).toBeNull();
    expect(within(frame()).queryByRole("button", { name: /Dismiss all/ })).toBeNull();
  });

  it("puts the bell in the header, before split and close, and the band under the header", async () => {
    await aPane({ asks: [removal("beta", 10)], failed: [] });
    await waitFor(() => expect(seen()).toEqual(["Remove workspace"]));

    const top = frame().querySelector(".pane-top") as HTMLElement;
    expect([...top.children].map((one) => one.getAttribute("data-scope") ?? one.className)).toEqual(
      ["pane-head", "notice-row"],
    );
    const doing = [...frame().querySelectorAll(".pane-doing > button")].map(
      (one) => one.getAttribute("data-way") ?? one.getAttribute("aria-label"),
    );
    // A fixed slot, right before the splits and the close.
    expect(doing.indexOf("bell")).toBeGreaterThanOrEqual(0);
    expect(doing.slice(doing.indexOf("bell") + 1, doing.indexOf("bell") + 3)).toEqual([
      "Split right",
      "Split down",
    ]);
    expect(doing).toHaveLength(doing.indexOf("bell") + 4);
  });

  it("keeps the bell's slot with no count when there is nothing, and draws no band", async () => {
    await aPane({ asks: [], failed: [] });
    await waitFor(() => expect(bell()).toHaveAccessibleName("Pane notices: No notices"));
    expect(bell()).toHaveTextContent(/^$/);
    expect(bell()).not.toHaveAttribute("data-trouble");
    expect(within(frame()).queryByRole("region", { name: "Pane notices" })).toBeNull();
  });

  it("never opens the band or takes the focus when a note arrives, and says the count politely", async () => {
    await aPane({ asks: [removal("beta", 10)], failed: [] });
    await waitFor(() => expect(seen()).toEqual(["Remove workspace"]));
    const keep = within(frame()).getByRole("button", { name: "Keep" });
    keep.focus();

    await act(() => emit("chat-sandbox-blocked", BLOCKED));

    await waitFor(() =>
      expect(bell()).toHaveAccessibleName("Pane notices: 1 notice, 1 in trouble, 1 asking"),
    );
    expect(seen()).toEqual(["Remove workspace"]);
    expect(bell()).toHaveAttribute("aria-expanded", "false");
    expect(keep).toHaveFocus();
    const said = frame().querySelectorAll('.pane-doing [aria-live="polite"]');
    expect([...said].map((one) => one.textContent)).toContain("1 notice, 1 in trouble, 1 asking");
  });
});

describe("a pane's notices, expanded", () => {
  it("shows every notice under a head row, the bell's count the same, and writes the choice", async () => {
    const { layouts } = await aPane({ asks: [removal("beta", 10)], failed: [FAILED] });
    await act(() => emit("chat-mcp-failed", FAILED));
    await waitFor(() => expect(bell()).toHaveTextContent(/^1$/));

    await userEvent.click(bell());

    await waitFor(() => expect(seen()).toEqual(["Remove workspace", "MCP server"]));
    expect(bell()).toHaveAttribute("aria-expanded", "true");
    expect(bell()).toHaveTextContent(/^1$/);
    const band = within(frame()).getByRole("region", { name: "Pane notices" });
    expect(within(band).getByText("2 notices, 1 waiting on you")).toBeTruthy();
    await waitFor(() =>
      expect(layouts.map((text) => JSON.parse(text) as { notices?: string }).pop()?.notices).toBe(
        "expanded",
      ),
    );
  });

  it("clears the notes with Dismiss all, counting what it clears, and leaves the ask", async () => {
    await aPane({ asks: [removal("beta", 10)], failed: [FAILED] });
    await act(() => emit("chat-mcp-failed", FAILED));
    await act(() => emit("chat-sandbox-blocked", BLOCKED));
    await waitFor(() => expect(bell()).toHaveTextContent(/^2$/));
    await userEvent.click(bell());

    await userEvent.click(await within(frame()).findByRole("button", { name: "Dismiss all 2" }));

    await waitFor(() => expect(seen()).toEqual(["Remove workspace"]));
    expect(bell()).toHaveAccessibleName("Pane notices: No notices, 1 asking");
    expect(within(frame()).queryByRole("button", { name: /Dismiss all/ })).toBeNull();
  });

  it("minimises again from the band's head row, for every pane", async () => {
    await aPane({ asks: [removal("beta", 10)], failed: [FAILED] });
    await act(() => emit("chat-mcp-failed", FAILED));
    await userEvent.click(bell());
    await waitFor(() => expect(seen()).toEqual(["Remove workspace", "MCP server"]));

    await userEvent.click(within(frame()).getByRole("button", { name: "Minimise" }));

    await waitFor(() => expect(seen()).toEqual(["Remove workspace"]));
    expect(bell()).toHaveAttribute("aria-expanded", "false");
  });
});

describe("a full band", () => {
  it("stands every ask, with no cap of two and no line about the rest in the Inbox", async () => {
    const asks = ["beta", "gamma", "delta", "epsilon"].map((one, at) => removal(one, 10 + at));
    await aPane({ asks, failed: [FAILED] });
    await act(() => emit("chat-mcp-failed", FAILED));
    await act(() => emit("chat-sandbox-blocked", BLOCKED));

    await waitFor(() => expect(seen()).toHaveLength(4));
    expect(seen()).toEqual(Array.from({ length: 4 }, () => "Remove workspace"));
    expect(within(frame()).queryByText(/wait in the Inbox|waits in the Inbox/)).toBeNull();
    expect(within(frame()).queryByRole("button", { name: /more/ })).toBeNull();
    await waitFor(() =>
      expect(bell()).toHaveAccessibleName("Pane notices: 2 notices, 2 in trouble, 4 asking"),
    );

    await userEvent.click(bell());
    await waitFor(() => expect(seen()).toHaveLength(6));
    const band = within(frame()).getByRole("region", { name: "Pane notices" });
    expect(within(band).getByText("6 notices, 4 waiting on you")).toBeTruthy();
    expect(within(band).getByRole("button", { name: "Dismiss all 2" })).toBeTruthy();
  });
});
