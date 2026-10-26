// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer, type Spot } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FolderEntry, Panels as PanelsModel, Piece, PlaneId } from "./bindings";
import { forgetProjectThemes } from "./projectTheme";
import type { WorkspaceState } from "./workspaceState";
import type { Referenced } from "./references";
import type { Offer } from "./actions";

/**
 * **Several files at once** (#1151): file and folder rows picked with Cmd or Ctrl and a click,
 * Shift and a click, or Shift and the arrows, and handed to a chat as one paste. A plain click
 * still opens a file. The row menu's *Add to a chat's context* on a picked row hands every
 * picked row, and on a row outside the picks still hands its one row.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
});

const PLANE = "/plane" as unknown as PlaneId;

const PANELS: PanelsModel = {
  workspace: "alpha",
  repos: ["svc"],
  paths: {},
  absent: [],
  refused: [],
  todos: [],
  todos_refused: null,
  done_todos: [],
  personas: [],
  persona: "steward",
  sessions: [],
  contributed: [],
};

const ONE: Piece = {
  piece: "one",
  path: "/plane/workspaces/alpha/.worktrees/svc/one",
  branch: "one",
  wired: true,
  stale: false,
  said: "",
};

const STATE: WorkspaceState = {
  panels: PANELS,
  repos: { workspace: "alpha", repos: [], cache_refused: null },
  pieces: { svc: [ONE] },
  piecesRefused: {},
  reading: false,
};

const entry = (name: string, on: Partial<FolderEntry> = {}): FolderEntry => ({
  name,
  kind: "file",
  ignored: false,
  refused: null,
  ...on,
});

const FOLDERS: Record<string, FolderEntry[]> = {
  "one:": [
    entry("docs", { kind: "folder" }),
    entry("a.txt"),
    entry("b.txt"),
    entry("c.txt"),
    entry("secret", { refused: "a link out of the branch" }),
  ],
};

const PICKED: Spot = { repo: "svc", piece: "one", path: ONE.path };

async function draw(onPress: (offer: Offer) => void = () => {}) {
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, string | null>;
      if (cmd === "branch_tree")
        return { entries: FOLDERS[`${a.piece ?? ""}:${a.folder}`] ?? [], more: 0 };
      if (cmd === "files_watch") return null;
      if (cmd === "project_icons_drawn") return null;
      if (cmd === "extension_icon_themes") return [];
      throw new Error(`unexpected ${cmd}`);
    },
    { shouldMockEvents: true },
  );
  const onOpenFile = vi.fn();
  const onAddToChat = vi.fn<(refs: Referenced[]) => void>();
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        plane={PLANE}
        workspace="alpha"
        state={STATE}
        chats={[]}
        spot={PICKED}
        onPick={() => {}}
        offers={new Map()}
        onPress={onPress}
        onReadAgain={() => {}}
        onOpenFile={onOpenFile}
        onAddToChat={onAddToChat}
      />
    </ChatsHere.Provider>,
  );
  await within(tree()).findByRole("treeitem", { name: /a\.txt/ });
  return { onOpenFile, onAddToChat };
}

const tree = () => screen.getByRole("tree", { name: /^Files of / });
const item = (name: string) => within(tree()).getByRole("treeitem", { name: new RegExp(name) });
const picked = () =>
  within(tree())
    .getAllByRole("treeitem")
    .filter((one) => one.getAttribute("aria-selected") === "true")
    .map((one) => one.textContent?.trim());
/** What the live count says (#1151): the one status the picks have. */
const said = () =>
  screen
    .getAllByRole("status")
    .find((one) => one.classList.contains("hidden-words"))
    ?.textContent?.trim();
const ref = (path: string, folder = false): Referenced => ({
  plane: PLANE,
  workspace: "alpha",
  repo: "svc",
  piece: "one",
  path,
  folder,
});

describe("picking several rows of a branch's files (#1151)", () => {
  it("picks a row on Cmd or Ctrl and a click, and opens nothing", async () => {
    const { onOpenFile } = await draw();

    fireEvent.click(item("a.txt"), { metaKey: true });
    fireEvent.click(item("docs"), { ctrlKey: true });

    expect(tree()).toHaveAttribute("aria-multiselectable", "true");
    expect(picked()).toEqual(["docs", "a.txt"]);
    expect(onOpenFile).not.toHaveBeenCalled();
    // A second Cmd-click takes a row back out.
    fireEvent.click(item("a.txt"), { metaKey: true });
    expect(picked()).toEqual(["docs"]);
  });

  it("picks every row from the last one clicked on Shift and a click", async () => {
    const { onOpenFile } = await draw();

    fireEvent.click(item("a.txt"));
    expect(onOpenFile).toHaveBeenCalledTimes(1);
    expect(picked()).toEqual([]);
    fireEvent.click(item("c.txt"), { shiftKey: true });

    expect(picked()).toEqual(["a.txt", "b.txt", "c.txt"]);
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });

  it("picks from the keyboard with Shift and the arrows, and Esc lets them go", async () => {
    await draw();
    act(() => item("a.txt").focus());

    await userEvent.keyboard("{Shift>}{ArrowDown}{ArrowDown}{/Shift}");

    expect(picked()).toEqual(["a.txt", "b.txt", "c.txt"]);
    expect(item("c.txt")).toHaveFocus();
    // Read out as it changes, to a screen reader only.
    expect(said()).toBe("3 picked");
    await userEvent.keyboard("{Escape}");
    expect(picked()).toEqual([]);
    expect(said()).toBe("");
  });

  it("never picks a row purlis will not hand over", async () => {
    await draw();

    fireEvent.click(item("c.txt"), { metaKey: true });
    fireEvent.click(item("secret"), { metaKey: true });

    expect(picked()).toEqual(["c.txt"]);
  });

  it("hands every picked row to a chat at once, in the tree's order", async () => {
    const { onAddToChat } = await draw();
    expect(screen.queryByRole("button", { name: /to a chat's context/ })).toBeNull();

    fireEvent.click(item("c.txt"), { metaKey: true });
    fireEvent.click(item("docs"), { metaKey: true });
    expect(
      within(screen.getByRole("group", { name: "Picked rows" })).getByText("2 picked"),
    ).toBeInTheDocument();
    expect(said()).toBe("2 picked");
    await userEvent.click(screen.getByRole("button", { name: "Add 2 picked to a chat's context" }));

    expect(onAddToChat).toHaveBeenCalledWith([ref("docs", true), ref("c.txt")]);
  });

  it("lets every pick go on Clear", async () => {
    await draw();
    fireEvent.click(item("a.txt"), { metaKey: true });
    fireEvent.click(item("b.txt"), { metaKey: true });

    await userEvent.click(screen.getByRole("button", { name: "Clear the picked rows" }));

    expect(picked()).toEqual([]);
  });

  it("hands every picked row from a picked row's menu, in the tree's order (#1151)", async () => {
    const pressed: Offer[] = [];
    await draw((offer) => pressed.push(offer));
    fireEvent.click(item("c.txt"), { metaKey: true });
    fireEvent.click(item("docs"), { metaKey: true });

    fireEvent.contextMenu(item("c.txt"));
    await userEvent.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Add 2 picked to a chat's context",
      }),
    );

    const branch = { workspace: "alpha", repo: "svc", piece: "one" };
    expect(pressed.map((offer) => offer.does)).toEqual([
      {
        verb: "addToChat",
        rows: [
          { at: { ...branch, path: "docs" }, folder: true },
          { at: { ...branch, path: "c.txt" }, folder: false },
        ],
      },
    ]);
  });

  it("hands only its own row from the menu of a row outside the picks", async () => {
    const pressed: Offer[] = [];
    await draw((offer) => pressed.push(offer));
    fireEvent.click(item("b.txt"), { metaKey: true });
    fireEvent.click(item("c.txt"), { metaKey: true });

    fireEvent.contextMenu(item("a.txt"));
    await userEvent.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Add to a chat's context",
      }),
    );

    expect(pressed.map((offer) => offer.does)).toEqual([
      {
        verb: "addToChat",
        rows: [
          { at: { workspace: "alpha", repo: "svc", piece: "one", path: "a.txt" }, folder: false },
        ],
      },
    ]);
  });
});
