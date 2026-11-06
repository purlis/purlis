// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer } from "./Explorer";
import { keptChanged } from "./branchFolders";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FolderEntry, KeptMark, Panels, PlaneId, WorkspaceListing } from "./bindings";
import type { Offer } from "./actions";
import { forgetProjectThemes } from "./projectTheme";
import type { WorkspaceState } from "./workspaceState";

/**
 * **In a LIVE workspace each file says where it is kept** (#5377, ADR 0038 as amended
 * 2026-11-05): *This machine only*, or *Saved with the project* when the LIVE allowlist holds
 * it, and an entry at the top kept on this machine offers Save with the project. A LOCAL
 * workspace's files carry no mark and offer no save.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
});

const PLANE = "/plane" as unknown as PlaneId;

const panels: Panels = {
  workspace: "alpha",
  repos: [],
  paths: {},
  absent: [],
  refused: [],
  todos: [],
  done_todos: [],
  todos_refused: null,
  personas: [],
  persona: "steward",
  sessions: [],
  contributed: [],
};

const state: WorkspaceState = {
  panels,
  repos: {
    workspace: "alpha",
    repos: [],
    cache_refused: null,
  } as unknown as WorkspaceState["repos"],
  pieces: {},
  piecesRefused: {},
  reading: false,
};

const entry = (name: string, more: Partial<FolderEntry> = {}): FolderEntry => ({
  name,
  kind: "file",
  ignored: false,
  refused: null,
  ...more,
});

/** The workspace's own folder, LIVE when `live` says, with `notes` saved once `saved.notes`. */
function core(live: boolean) {
  const saved = { notes: false };
  const mark = (shared: boolean): { kept?: KeptMark } =>
    live ? { kept: shared ? "with-the-project" : "this-machine-only" } : {};
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd !== "workspace_tree") return null;
      const listing: WorkspaceListing =
        a.folder === ""
          ? {
              entries: [
                entry("memory", { kind: "folder", managed: true, ...mark(true) }),
                entry("notes", { kind: "folder", ...mark(saved.notes) }),
                entry("run.sh", mark(false)),
              ],
              more: 0,
              repos: [],
            }
          : { entries: [entry("plan.md", mark(saved.notes))], more: 0, repos: [] };
      return listing;
    },
    { shouldMockEvents: true },
  );
  return saved;
}

function draw() {
  const onPress = vi.fn<(offer: Offer) => void>();
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        part="explorer"
        plane={PLANE}
        workspace="alpha"
        state={state}
        chats={[]}
        spot={undefined}
        onPick={() => {}}
        offers={new Map()}
        onPress={onPress}
        onReadAgain={() => {}}
        onOpenFile={() => {}}
      />
    </ChatsHere.Provider>,
  );
  return { onPress };
}

const tree = () => screen.getByRole("treegrid", { name: "Files of alpha" });
const row = (name: RegExp) => within(tree()).findByRole("row", { name });

/** The rows of the menu of the row `name` names, closed again after. */
async function menuOf(name: RegExp): Promise<string[]> {
  fireEvent.contextMenu(await row(name));
  const menu = await screen.findByRole("menu");
  const items = within(menu)
    .getAllByRole("menuitem")
    .map((one) => one.getAttribute("aria-label") ?? one.textContent ?? "");
  await userEvent.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  return items;
}

describe("Explorer, where a LIVE workspace's files are kept (#5377)", () => {
  it("marks each file where it is kept, in words a screen reader hears", async () => {
    core(true);
    draw();

    const notes = await row(/^notes this machine only/);
    expect(within(notes).getByText("This machine only")).toBeInTheDocument();
    const memory = await row(/^memory managed saved with the project/);
    expect(within(memory).getByText("Saved with the project")).toBeInTheDocument();
    expect(await row(/^run\.sh this machine only/)).toBeInTheDocument();
  });

  it("offers Save with the project on the person's entry at the top, and nowhere else", async () => {
    core(true);
    const { onPress } = draw();

    expect((await menuOf(/^notes/)).at(-1)).toBe("Save with the project");
    expect(await menuOf(/^run\.sh/)).toContain("Save with the project");
    expect(await menuOf(/^memory/)).not.toContain("Save with the project");
    await userEvent.click(await row(/^notes/));
    expect(await menuOf(/^plan\.md/)).not.toContain("Save with the project");

    fireEvent.contextMenu(await row(/^run\.sh/));
    await userEvent.click(
      within(await screen.findByRole("menu")).getByRole("menuitem", {
        name: "Save with the project",
      }),
    );
    expect(onPress).toHaveBeenCalledWith(
      expect.objectContaining({
        does: {
          verb: "saveWithTheProject",
          at: { workspace: "alpha", repo: "", piece: null, path: "run.sh" },
        },
      }),
    );
  });

  it("reads the marks again once a save changed where a file is kept", async () => {
    const saved = core(true);
    draw();
    await row(/^notes this machine only/);

    saved.notes = true;
    act(() => keptChanged(PLANE, "alpha"));

    expect(await row(/^notes saved with the project/)).toBeInTheDocument();
  });

  it("marks nothing and offers no save in a workspace that is not LIVE", async () => {
    core(false);
    draw();

    await row(/^notes/);
    expect(within(tree()).queryByText("This machine only")).toBeNull();
    expect(within(tree()).queryByText("Saved with the project")).toBeNull();
    expect(await menuOf(/^notes/)).not.toContain("Save with the project");
    expect(await menuOf(/^run\.sh/)).not.toContain("Save with the project");
  });
});
