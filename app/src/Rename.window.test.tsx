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
import type { LaidOut, LaidTab, OpenChat } from "./bindings";
import { stripNamed } from "./test-strips";
import { rowsOf, treeNamed } from "./ui/tree/tree.testkit";

/**
 * **Rename says what it renames** (#5343, spec #5339), against the whole window: a tab with one
 * chat renames the chat, and the tab, the list and the head change together; "Only this tab"
 * names the tab and leaves the chat's name; a split names the tab only; Rename chat is in each
 * pane's ⋯ and the list row's menu, and opens on that chat's own name, never another's (#5287).
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

function chat(session: number, label: string | null): OpenChat {
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
    label,
    from: null,
  };
}

type Asked = { cmd: string; args: Record<string, unknown> };

/** The core, which holds each chat's name as `rename_chat` leaves it and lists it so. */
function core(sessions: number[], laid: LaidOut): Asked[] {
  const asked: Asked[] = [];
  const labels = new Map<number, string | null>(sessions.map((one) => [one, null]));
  const open = () => sessions.map((one) => chat(one, labels.get(one) ?? null));
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "plugin:event|listen") return 1;
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open();
    if (cmd === "plane_sidebar")
      return { root: PLANE, personas: [], persona: null, unfiled: open(), workspaces: [] };
    if (cmd === "reopened_layout") return laid;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "rename_chat") {
      const { session, label } = args as { session: number; label: string };
      const held = label.trim() === "" ? null : label.trim();
      labels.set(session, held);
      return held;
    }
    return null;
  });
  return asked;
}

const tab = (tree: LaidTab["tree"]): LaidTab => ({
  workspace: null,
  pinned: false,
  window: 0,
  front: true,
  focused: "a",
  tree,
});
const onePane: LaidOut = { strip: null, tabs: [tab({ kind: "chat", chat: 1 })] };
const twoPanes: LaidOut = {
  strip: null,
  tabs: [
    tab({
      kind: "split",
      direction: "row",
      share: 500,
      a: { kind: "chat", chat: 1 },
      b: { kind: "chat", chat: 2 },
    }),
  ],
};

const theTab = () => within(stripNamed("Tabs")).getAllByRole("tab")[0];
const tabName = () => theTab().querySelector(".tab-name")?.textContent;
const heads = () =>
  [...document.querySelectorAll(".pane-head .pane-name")].map((one) => one.textContent);
const list = () => treeNamed("Chats of this project");
const rows = () =>
  rowsOf(list()).map(
    (row) => row.querySelector(".tree-label, .chat-name")?.textContent ?? row.textContent,
  );
const rowOf = (session: number) =>
  list().querySelector<HTMLElement>(`[role="row"][data-session="${session}"]`);
const theRowOf = (session: number): HTMLElement => {
  const row = rowOf(session);
  if (row === null) throw new Error(`no row for chat ${session}`);
  return row;
};
const renamed = (asked: Asked[]) =>
  asked.filter((one) => one.cmd === "rename_chat").map((one) => one.args);
const box = (dialog: HTMLElement) => within(dialog).getByLabelText("New name");

/** Waits for the window, with each chat's row listed. */
async function drawn(count: number) {
  await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(count));
  await waitFor(() => expect(rowOf(1)).not.toBeNull());
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a tab with one chat", () => {
  it("renames the chat, and the tab, the row and the head change together", async () => {
    const asked = core([1], onePane);
    render(<App />);
    await drawn(1);

    await userEvent.dblClick(theTab());
    const dialog = await screen.findByRole("dialog", { name: "Rename ide.1" });
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}planner{Enter}");

    await waitFor(() => expect(tabName()).toBe("planner"));
    expect(renamed(asked)).toEqual([{ plane: PLANE, session: 1, label: "planner" }]);
    await waitFor(() => expect(rowOf(1)).toHaveTextContent("planner"));
    expect(heads()).toEqual(["planner"]);
  });

  it("names only the tab with Only this tab, and the row and the head keep the chat's name", async () => {
    const asked = core([1], onePane);
    render(<App />);
    await drawn(1);

    await userEvent.dblClick(theTab());
    const dialog = await screen.findByRole("dialog", { name: "Rename ide.1" });
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "Only this tab" }));
    await userEvent.clear(box(dialog));
    await userEvent.type(box(dialog), "the build{Enter}");

    await waitFor(() => expect(tabName()).toBe("the build"));
    expect(renamed(asked)).toEqual([]);
    expect(rowOf(1)).toHaveTextContent("ide.1");
    expect(heads()).toEqual(["ide.1"]);
  });
});

describe("a split tab", () => {
  it("names the tab only, says the chats keep their names, and the core is told nothing", async () => {
    const asked = core([1, 2], twoPanes);
    render(<App />);
    await drawn(2);

    await userEvent.dblClick(theTab());
    const dialog = await screen.findByRole("dialog", { name: "Rename tab ide.1" });
    expect(dialog).toHaveTextContent(
      "This tab shows 2 panes, so this names the tab only. The chats keep their names, and each pane's ⋯ renames its own.",
    );
    expect(within(dialog).queryByRole("checkbox")).toBeNull();
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}the build{Enter}");

    await waitFor(() => expect(tabName()).toBe("the build"));
    expect(renamed(asked)).toEqual([]);
    expect(heads()).toEqual(["ide.1", "ide.2"]);
  });

  it("says Rename tab… on its menu", async () => {
    core([1, 2], twoPanes);
    render(<App />);
    await drawn(2);

    fireEvent.contextMenu(theTab());

    expect(await screen.findByRole("menuitem", { name: "Rename tab…" })).toBeInTheDocument();
  });

  it("renames a second pane's chat from its ⋯, on its own name, and the tab keeps the first's", async () => {
    const asked = core([1, 2], twoPanes);
    render(<App />);
    await drawn(2);

    const right = screen.getByRole("region", { name: "Right pane" });
    await userEvent.click(within(right).getByRole("button", { name: "More for ide.2" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Rename chat…" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename chat ide.2" });
    // Never another chat's name (#5287).
    expect(box(dialog)).toHaveValue("ide.2");
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}builder{Enter}");

    await waitFor(() => expect(heads()).toEqual(["ide.1", "builder"]));
    expect(renamed(asked)).toEqual([{ plane: PLANE, session: 2, label: "builder" }]);
    expect(tabName()).toBe("ide.1");
  });

  it("follows its first chat's rename from that pane's ⋯", async () => {
    core([1, 2], twoPanes);
    render(<App />);
    await drawn(2);

    const left = screen.getByRole("region", { name: "Left pane" });
    await userEvent.click(within(left).getByRole("button", { name: "More for ide.1" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Rename chat…" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename chat ide.1" });
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}planner{Enter}");

    await waitFor(() => expect(tabName()).toBe("planner"));
  });

  it("is a one-chat tab again once it closes back to one pane, and keeps a name it was given", async () => {
    core([1, 2], twoPanes);
    render(<App />);
    await drawn(2);
    await userEvent.dblClick(theTab());
    const named = await screen.findByRole("dialog", { name: "Rename tab ide.1" });
    await waitFor(() => expect(box(named)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}the build{Enter}");
    await waitFor(() => expect(tabName()).toBe("the build"));

    const right = screen.getByRole("region", { name: "Right pane" });
    await userEvent.click(within(right).getByRole("button", { name: "Close this pane" }));
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(1));
    expect(tabName()).toBe("the build");

    await userEvent.dblClick(theTab());
    const again = await screen.findByRole("dialog", { name: "Rename the build" });
    expect(again).toHaveTextContent("This tab shows one chat, so this renames the chat");
    expect(within(again).getByRole("checkbox", { name: "Only this tab" })).toBeChecked();
  });
});

describe("a view's tab", () => {
  it("has no rename: a double-click and F2 on it open nothing", async () => {
    const laid: LaidOut = {
      strip: null,
      tabs: [
        tab({ kind: "chat", chat: 1 }),
        {
          ...tab({
            kind: "view",
            view: {
              from: null,
              view: "saving",
              key: "",
              title: "Saving",
              workspace: null,
              at: 1,
              active: false,
              pinned: false,
            },
          }),
          front: false,
        },
      ],
    };
    core([1], laid);
    render(<App />);
    await drawn(1);
    const viewTab = await waitFor(() => {
      const found = within(stripNamed("Tabs")).getAllByRole("tab")[1];
      if (found === undefined) throw new Error("no view tab yet");
      return found;
    });

    await userEvent.dblClick(viewTab);
    viewTab.focus();
    await userEvent.keyboard("{F2}");

    expect(screen.queryByRole("dialog", { name: /^Rename/ })).not.toBeInTheDocument();
  });
});

describe("Rename chat from the list", () => {
  it("is on the row's menu, and renames the chat on its tab and its head", async () => {
    const asked = core([1], onePane);
    render(<App />);
    await drawn(1);

    fireEvent.contextMenu(theRowOf(1));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Rename chat…" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename chat ide.1" });
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Control>}a{/Control}planner{Enter}");

    await waitFor(() => expect(tabName()).toBe("planner"));
    expect(heads()).toEqual(["planner"]);
    expect(renamed(asked)).toEqual([{ plane: PLANE, session: 1, label: "planner" }]);
  });

  it("opens with F2 on the row, not the palette, and Esc hands the keyboard back to it", async () => {
    core([1], onePane);
    render(<App />);
    await drawn(1);
    theRowOf(1).focus();

    await userEvent.keyboard("{F2}");
    const dialog = await screen.findByRole("dialog", { name: "Rename chat ide.1" });
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
    await waitFor(() => expect(box(dialog)).toHaveFocus());
    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(rowOf(1)).toHaveFocus());
    expect(rows()).toHaveLength(1);
  });
});
