// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import { CLOSE_HANDED, type WindowDraft } from "./bindings";
import { FROM_A_LINK_SAYS } from "./EndingChat";

/**
 * **A close that keeps a chat's theme drafts, as the window asks it** (#2332, #2708, #2761): who
 * draws each kept theme is asked in turn, and a link's close of a chat with drafts is asked here.
 * The window reading what a mocked core answers; the engine's Keep is `drafts.rs`'s.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";

const SIDEBAR = {
  root: PLANE,
  workspaces: [
    {
      name: "alpha",
      path: `${PLANE}/workspaces/alpha`,
      vision: "Ship it",
      todos: [],
      chats: [],
    },
  ],
  personas: ["steward"],
  persona: "steward",
  unfiled: [],
};

const START_OPTIONS = {
  profiles: [
    {
      name: "claude",
      kind: "claude",
      shown: "claude",
      source: "built-in",
      is_default: true,
      approval: null,
    },
  ],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

const draft = (folder: string, session: number): WindowDraft => ({
  folder,
  session,
  chat: `steward ${session}`,
  workspace: null,
  kept_to: "project",
  replaces: false,
  changed: false,
  drawn: true,
  broken: [],
  refused: [],
  review: null,
  kept_refused: false,
});

/** Answers every command; chat 1, the one opened, has the theme drafts `folders`. */
function core(folders: readonly string[]): { asked: { cmd: string; args: unknown }[] } {
  const asked: { cmd: string; args: unknown }[] = [];
  let opened = 0;
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "plane_sidebar") return SIDEBAR;
      if (cmd === "running_sessions") return [];
      if (cmd === "opened_chats") return [];
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "start_options") return START_OPTIONS;
      if (cmd === "start_chat") return { session: ++opened };
      if (cmd === "smart_close_offer") return { available: false, why: null, close_first: true };
      if (cmd === "smart_closing") return [];
      if (cmd === "theme_drafts_of_chat")
        return (args as { session: number }).session === 1
          ? folders.map((folder) => draft(folder, 1))
          : [];
      if (cmd === "theme_draft_keep") return `${(args as { folder: string }).folder} is kept`;
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

async function openAChat() {
  await userEvent.click(await screen.findByRole("button", { name: "New tab" }));
  await userEvent.click(await screen.findByRole("button", { name: "Start" }));
  await screen.findByRole("button", { name: "End chat steward 1" });
}

/** Answers the close dialog on screen: Keep its drafts, then Close. */
async function keepAndClose() {
  const asking = await screen.findByRole("alertdialog");
  await userEvent.click(within(asking).getByRole("radio", { name: /^Keep/ }));
  await userEvent.click(within(asking).getByRole("button", { name: "Close" }));
}

const asked = (log: { cmd: string; args: unknown }[], cmd: string) =>
  log.filter((one) => one.cmd === cmd).map((one) => one.args);

describe("a close that keeps several theme drafts", () => {
  it("asks who draws each kept theme, one after the other", async () => {
    const { asked: log } = core(["dusk", "dawn"]);
    render(<App />);
    await openAChat();
    await userEvent.click(screen.getByRole("button", { name: "End chat steward 1" }));
    await keepAndClose();

    await waitFor(() =>
      expect(
        asked(log, "theme_draft_keep").map((one) => (one as { folder: string }).folder),
      ).toEqual(["dusk", "dawn"]),
    );
    const first = await screen.findByRole("alertdialog", { name: "Who draws dusk?" });
    await userEvent.click(within(first).getByRole("button", { name: "Only for me" }));
    const second = await screen.findByRole("alertdialog", { name: "Who draws dawn?" });
    await userEvent.click(within(second).getByRole("button", { name: "Only for me" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(asked(log, "theme_choose").map((one) => (one as { theme: string }).theme)).toEqual([
      "dusk",
      "dawn",
    ]);
  });
});

describe("a link's close of a chat with theme drafts", () => {
  it("is asked here, saying a link asked, Keep or Revert first", async () => {
    const { asked: log } = core(["dusk"]);
    render(<App />);
    await openAChat();
    await act(() => emit(CLOSE_HANDED, { plane: PLANE, sessions: [1] }));

    const asking = await screen.findByRole("alertdialog", { name: "End chat steward 1?" });
    expect(asking.textContent).toContain(FROM_A_LINK_SAYS);
    // It starts on Cancel: a stray Return closes nothing a link asked for.
    await waitFor(() =>
      expect(within(asking).getByRole("button", { name: "Cancel" })).toHaveFocus(),
    );
    // Nothing closes until the drafts are answered here.
    expect(within(asking).getByRole("button", { name: "Close" })).toBeDisabled();
    expect(asked(log, "close_session")).toEqual([]);
    await keepAndClose();
    await waitFor(() => expect(asked(log, "theme_draft_keep")).toHaveLength(1));
  });

  it("never changes a close question already on screen", async () => {
    core(["dusk"]);
    render(<App />);
    await openAChat();
    await userEvent.click(screen.getByRole("button", { name: "End chat steward 1" }));
    const asking = await screen.findByRole("alertdialog");
    await act(() => emit(CLOSE_HANDED, { plane: PLANE, sessions: [1] }));
    await act(async () => {});
    expect(screen.getAllByRole("alertdialog")).toEqual([asking]);
    expect(asking.textContent).not.toContain(FROM_A_LINK_SAYS);
  });

  it("asks nothing for chats no tab here holds alone, and says to close them here", async () => {
    core(["dusk"]);
    render(<App />);
    await openAChat();
    await act(() => emit(CLOSE_HANDED, { plane: PLANE, sessions: [1, 7] }));
    expect(await screen.findByText(/no tab here holds just those chats/)).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("is not asked in a project this window does not hold", async () => {
    core(["dusk"]);
    render(<App />);
    await openAChat();
    await act(() => emit(CLOSE_HANDED, { plane: "/home/dev/other", sessions: [1] }));
    await act(async () => {});
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });
});
