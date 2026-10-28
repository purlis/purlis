// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
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
import { forgetYourEditor, setYourEditor } from "./yourEditor";
import { showTheExplorer, stripNamed } from "./test-strips";
import { SETTINGS_LINK, type SettingsLinkAsk } from "./settings/links";
import { findResult, theResult } from "./actionResult.testkit";

/**
 * **A file or folder row's actions, against the whole window** (FM-10, #1113): each row of its
 * menu reaches the core as the branch and the path inside it — never a directory the window
 * joined — and what the core answers is said. The rows themselves are `actions.test.ts`'s; what
 * the core does with the path is `piecefiles.rs`'s and `files::place`'s.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

type Asked = { cmd: string; args: Record<string, unknown> };

/** A chat open in the workspace, for the rows that hand a reference to one. */
const CHAT = {
  session: 1,
  name: "1",
  cwd: ALPHA,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: "steward",
};

/** One workspace with one clone and one branch, holding `src/` and `README.md`. */
function core({ refuse, chat = false }: { refuse?: string; chat?: boolean } = {}) {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats")
      return chat ? [{ ...CHAT, unreported: null, guessed: null, pinned: false }] : [];
    if (cmd === "reference_into_chat") return { kind: "typed", text: "@README.md" };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: [
          { name: "alpha", path: ALPHA, vision: "", todos: [], chats: chat ? [CHAT] : [] },
        ],
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
      return [
        {
          piece: "one",
          path: `${ALPHA}/.worktrees/svc/one`,
          branch: "one",
          wired: true,
          stale: false,
        },
      ];
    if (cmd === "branch_tree")
      return a.folder === "" && a.piece === "one"
        ? {
            entries: [
              { name: "src", kind: "folder", ignored: false, refused: null },
              { name: "README.md", kind: "file", ignored: false, refused: null },
            ],
            more: 0,
          }
        : { entries: [], more: 0 };
    if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
    if (cmd === "copy_branch_path" || cmd === "reveal_branch_path") {
      if (refuse !== undefined) throw refuse;
      return null;
    }
    if (cmd === "open_shell_in_branch") return 41;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
    return null;
  });
  return asked;
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  forgetYourEditor();
});

const ONE = { workspace: "alpha", repo: "svc", piece: "one" };
const asks = (asked: Asked[], cmd: string) =>
  asked.filter((one) => one.cmd === cmd).map((one) => one.args);

/** Picks the branch, so Explorer's Files section draws its files (#1677), then opens the menu
 *  on `name`'s row and presses `row` in it. */
async function fromTheMenu(name: string, row: string | RegExp) {
  await showTheExplorer();
  // Picked in the Repos view, the Panel's since #1737, which is what Files lists.
  const repos = await screen.findByRole("navigation", { name: "Repos" });
  await userEvent.click(await within(repos).findByRole("row", { name: /^one in svc/ }));
  const tree = await screen.findByRole("treegrid", { name: "Files of one in svc" });
  fireEvent.contextMenu(await within(tree).findByRole("row", { name: new RegExp(`^${name}`) }));
  const menu = await screen.findByRole("menu");
  await userEvent.click(within(menu).getByRole("menuitem", { name: row }));
}

describe("a file or folder row's actions (FM-10)", () => {
  it("copies a path through the core, by branch and path, and says so", async () => {
    const asked = core();
    render(<App />);

    await fromTheMenu("README.md", "Copy absolute path");

    await waitFor(() =>
      expect(asks(asked, "copy_branch_path")).toEqual([
        { plane: PLANE, ...ONE, path: "README.md", absolute: true },
      ]),
    );
    expect(await findResult("Copied the absolute path of README.md.")).toBeInTheDocument();
  });

  it("says the core's refusal in its own words", async () => {
    core({ refuse: "'README.md' is not in the branch's folder any more" });
    render(<App />);

    await fromTheMenu("README.md", "Copy relative path");

    expect(
      await findResult("'README.md' is not in the branch's folder any more"),
    ).toBeInTheDocument();
  });

  it("reveals a folder by branch and path", async () => {
    const asked = core();
    render(<App />);

    await fromTheMenu("src", /^Reveal in /);

    await waitFor(() =>
      expect(asks(asked, "reveal_branch_path")).toEqual([{ plane: PLANE, ...ONE, path: "src" }]),
    );
  });

  it("opens a shell tab in a folder the core resolves, and draws its tab", async () => {
    const asked = core();
    render(<App />);

    await fromTheMenu("src", "Open a shell tab here");

    await waitFor(() =>
      expect(asks(asked, "open_shell_in_branch")).toEqual([
        expect.objectContaining({ plane: PLANE, ...ONE, folder: "src", name: "shell 1" }),
      ]),
    );
    expect(asks(asked, "open_session")).toEqual([]);
    expect(
      await within(stripNamed("Tabs")).findByRole("tab", {
        name: /shell 1/,
      }),
    ).toBeInTheDocument();
  });

  it("opens a file in your editor at its first line, or asks for an editor first", async () => {
    const asked = core();
    render(<App />);

    await fromTheMenu("README.md", "Open in your editor");
    expect(await findResult("Choose your editor in Settings first.")).toBeInTheDocument();

    setYourEditor("zed");
    await fromTheMenu("README.md", "Open in your editor");
    await waitFor(() =>
      expect(asks(asked, "open_in_your_editor")).toEqual([
        { plane: PLANE, ...ONE, path: "README.md", line: 1, editor: "zed" },
      ]),
    );
  });

  it("links the no-editor refusal to Settings › You › Editor (#1201)", async () => {
    core();
    const links: SettingsLinkAsk[] = [];
    const heard = (event: Event) => links.push((event as CustomEvent<SettingsLinkAsk>).detail);
    window.addEventListener(SETTINGS_LINK, heard);
    try {
      render(<App />);

      await fromTheMenu("README.md", "Open in your editor");
      const words = await findResult("Choose your editor in Settings first.");
      // A closable notice in the trouble tone, said once as a polite status (#3344).
      expect(words.closest('[role="status"]')).not.toBeNull();
      const said = theResult();
      await userEvent.click(within(said).getByRole("button", { name: "Choose your editor" }));

      expect(links).toEqual([
        { plane: PLANE, link: { group: "you.editor", setting: "you.editor.yours" } },
      ]);
    } finally {
      window.removeEventListener(SETTINGS_LINK, heard);
    }
  });

  it("adds a file to an open chat's context from its row, through the chat picker (#1151)", async () => {
    const asked = core({ chat: true });
    render(<App />);

    await fromTheMenu("README.md", "Add to a chat's context");

    const picker = await screen.findByRole("dialog", {
      name: "Add README.md to a chat's context",
    });
    await userEvent.click(within(picker).getByRole("option", { name: /steward/ }));

    await waitFor(() =>
      expect(asks(asked, "reference_into_chat")).toEqual([
        expect.objectContaining({
          plane: PLANE,
          ...ONE,
          path: "README.md",
          lines: null,
          session: 1,
        }),
      ]),
    );
    expect(await findResult(/Typed @README\.md into .*Nothing was sent\./)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says no chat is open, and types nothing, where the project has none", async () => {
    const asked = core();
    render(<App />);

    await fromTheMenu("src", "Add to a chat's context");

    const picker = await screen.findByRole("dialog", { name: "Add src/ to a chat's context" });
    expect(within(picker).getByRole("status")).toHaveTextContent(/No chat is open/);
    await userEvent.click(within(picker).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(asks(asked, "reference_into_chat")).toEqual([]);
  });
});
