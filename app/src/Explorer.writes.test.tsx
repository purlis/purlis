// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { Explorer } from "./Explorer";
import { perform, type Doing } from "./actions";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type {
  ChatTouching,
  FolderEntry,
  OpenChat,
  Panels,
  PlaneId,
  WorkspaceListing,
} from "./bindings";
import { onTheWire } from "./chatEvents.testkit";
import { theFrameNow } from "./frame.testkit";
import { forgetProjectThemes } from "./projectTheme";
import { useWorkspaceWrites } from "./WorkspaceWrites";
import type { WorkspaceState } from "./workspaceState";

/**
 * **The person's own workspace files are written from Explorer** (#5376, ADR 0081 as amended
 * 2026-11-05): New file…, New folder…, Import files…, Rename… and Delete… on their own rows and
 * in the view's head, never on what purlis keeps or in a repo; each asks first, and a rename or
 * delete of a file a chat is writing says so before anything is done. The core is mocked: what
 * it refuses is `the_persons_workspace_files_are_written_and_nothing_else.rs`'s.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
});

const PLANE = "/plane" as unknown as PlaneId;

const panels: Panels = {
  workspace: "alpha",
  repos: ["api"],
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
    repos: [{ name: "api", branch: "main" }],
    cache_refused: null,
  } as unknown as WorkspaceState["repos"],
  pieces: { api: [] },
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

/** A chat working in the workspace's own folder. */
const WRITING = {
  session: 3,
  name: "steward 3",
  cwd: "/plane/workspaces/alpha",
  harness: "claude-code",
  profile: null,
  persona: null,
} as unknown as OpenChat;

type Call = { cmd: string; args: Record<string, unknown> };

/** The workspace's folder: `memory/` and `workspace.md` purlis keeps, the person's `notes/`
 *  holding `plan.md`, and repo `api` in `repos/`. `refuse` makes the writes refuse. */
function core(refuse?: string) {
  const calls: Call[] = [];
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd.startsWith("workspace_files_")) {
        calls.push({ cmd, args: a });
        if (refuse !== undefined) throw refuse;
        if (cmd === "workspace_files_trash") return null;
        if (cmd === "workspace_files_import") return [];
        return a.folder === "" ? a.name : `${String(a.folder ?? "notes")}/${String(a.name)}`;
      }
      if (cmd === "workspace_tree") {
        const listing: WorkspaceListing =
          a.folder === ""
            ? {
                entries: [
                  entry("memory", { kind: "folder", managed: true }),
                  entry("notes", { kind: "folder" }),
                  entry("workspace.md", { managed: true }),
                ],
                more: 0,
                repos: [{ name: "api", inside_repos: true }],
              }
            : { entries: [entry("plan.md")], more: 0, repos: [] };
        return listing;
      }
      if (cmd === "branch_tree") return { entries: [entry("README.md")], more: 0 };
      if (cmd === "branch_status")
        return { refused: [], changes: [], folders: [], more: 0, base: "main" };
      return null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

/** Explorer, its rows' writes carried out as the window carries them out. */
function Window({ chats }: { chats: OpenChat[] }) {
  const writes = useWorkspaceWrites(PLANE);
  const doing = { askWorkspaceWrite: writes.ask } as unknown as Doing;
  return (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        part="explorer"
        plane={PLANE}
        workspace="alpha"
        state={state}
        chats={chats}
        spot={undefined}
        onPick={() => {}}
        offers={new Map()}
        onPress={(offer) => void perform(offer, doing)}
        onReadAgain={() => {}}
        onOpenFile={() => {}}
      />
      {writes.dialog}
    </ChatsHere.Provider>
  );
}

const draw = (chats: OpenChat[] = []) => render(<Window chats={chats} />);

const tree = () => screen.getByRole("treegrid", { name: "Files of alpha" });
const row = (name: RegExp) => within(tree()).findByRole("row", { name });

/** The rows of the menu opened on the row named `name`, then the menu put away. */
async function menuOf(name: RegExp): Promise<string[]> {
  fireEvent.contextMenu(await row(name));
  const menu = await screen.findByRole("menu");
  const items = within(menu)
    .getAllByRole("menuitem")
    .map((one) => one.textContent ?? "");
  await userEvent.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  return items;
}

/** The row named `item` pressed, in the menu of the row named `name`. */
async function pressIn(name: RegExp, item: string) {
  fireEvent.contextMenu(await row(name));
  const menu = await screen.findByRole("menu");
  await userEvent.click(within(menu).getByRole("menuitem", { name: item }));
}

const WRITES = ["New file…", "New folder…", "Import files…", "Rename…", "Delete…"];
const written = (items: string[]) => items.filter((item) => WRITES.includes(item));

describe("Explorer, writing the person's own workspace files (#5376)", () => {
  it("offers a folder of theirs every write, and a file of theirs Rename… and Delete…", async () => {
    core();
    draw();

    expect(written(await menuOf(/^notes/))).toEqual(WRITES);
    await userEvent.click(await row(/^notes/));
    expect(written(await menuOf(/^plan\.md/))).toEqual(["Rename…", "Delete…"]);
  });

  it("offers no write on what purlis keeps, on a repo's row or in a repo", async () => {
    core();
    draw();

    for (const name of [/^memory/, /^workspace\.md/, /^repos/, /^api/, /^README\.md/]) {
      // `repos/` and the one repo's row start open, `README.md` under them (#3343).
      if (name.source === "^repos") {
        expect(await row(name)).toHaveAttribute("aria-expanded", "true");
        continue;
      }
      expect(written(await menuOf(name))).toEqual([]);
    }
  });

  it("names a new file in a dialog and makes it in the folder it was asked in", async () => {
    const calls = core();
    draw();

    await pressIn(/^notes/, "New file…");
    const dialog = await screen.findByRole("dialog", { name: "New file" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "File name" }), "today.md");
    await userEvent.click(within(dialog).getByRole("button", { name: "Create file" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(calls).toEqual([
      {
        cmd: "workspace_files_new_file",
        args: { plane: PLANE, workspace: "alpha", folder: "notes", name: "today.md" },
      },
    ]);
  });

  it("makes a new folder at the workspace's top from the view's head", async () => {
    const calls = core();
    draw();
    await row(/^notes/);

    await userEvent.click(screen.getByRole("button", { name: "New folder" }));
    const dialog = await screen.findByRole("dialog", { name: "New folder" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Folder name" }), "scripts");
    await userEvent.click(within(dialog).getByRole("button", { name: "Create folder" }));

    await waitFor(() =>
      expect(calls.map((one) => one.args)).toEqual([
        { plane: PLANE, workspace: "alpha", folder: "", name: "scripts" },
      ]),
    );
    expect(calls[0].cmd).toBe("workspace_files_new_folder");
  });

  it("imports into a folder through the system's picker, asking nothing of its own", async () => {
    const calls = core();
    draw();

    await pressIn(/^notes/, "Import files…");

    await waitFor(() =>
      expect(calls).toEqual([
        {
          cmd: "workspace_files_import",
          args: { plane: PLANE, workspace: "alpha", folder: "notes" },
        },
      ]),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("sends a file to the Trash after one confirmation", async () => {
    const calls = core();
    draw();
    await userEvent.click(await row(/^notes/));

    await pressIn(/^plan\.md/, "Delete…");
    const asked = await screen.findByRole("alertdialog", { name: "Delete plan.md?" });
    expect(asked).toHaveTextContent("plan.md goes to the Trash, where you can put it back.");
    expect(calls).toEqual([]);
    await userEvent.click(within(asked).getByRole("button", { name: "Move to Trash" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(calls).toEqual([
      {
        cmd: "workspace_files_trash",
        args: { plane: PLANE, workspace: "alpha", path: "notes/plan.md" },
      },
    ]);
  });

  it("says the core's refusal in the dialog, and keeps it up", async () => {
    const refused =
      "purlis did not delete 'notes/plan.md': the system would not put it in the Trash (no Trash). purlis never deletes a file for good, so it is still there.";
    core(refused);
    draw();
    await userEvent.click(await row(/^notes/));

    await pressIn(/^plan\.md/, "Delete…");
    const asked = await screen.findByRole("alertdialog", { name: "Delete plan.md?" });
    await userEvent.click(within(asked).getByRole("button", { name: "Move to Trash" }));

    expect(await within(asked).findByRole("alert")).toHaveTextContent(refused);
    expect(screen.getByRole("alertdialog", { name: "Delete plan.md?" })).toBeInTheDocument();
  });

  it("asks first before renaming a file a chat is writing, then renames it", async () => {
    const calls = core();
    draw([WRITING]);
    await userEvent.click(await row(/^notes/));
    await row(/^plan\.md/);
    await act(async () => {
      await emit(
        ...onTheWire("chat-touching", {
          plane: PLANE,
          session: 3,
          path: "notes/plan.md",
        } satisfies ChatTouching),
      );
      theFrameNow();
    });

    await pressIn(/^plan\.md/, "Rename…");
    const asked = await screen.findByRole("alertdialog", {
      name: "Rename plan.md while a chat works on it?",
    });
    expect(asked).toHaveTextContent("steward 3 is working here now.");
    expect(calls).toEqual([]);
    await userEvent.click(within(asked).getByRole("button", { name: "Rename anyway…" }));

    const dialog = await screen.findByRole("dialog", { name: "Rename plan.md" });
    const box = within(dialog).getByRole("textbox", { name: "New name" });
    await userEvent.clear(box);
    await userEvent.type(box, "old-plan.md");
    await userEvent.click(within(dialog).getByRole("button", { name: "Rename file" }));

    await waitFor(() =>
      expect(calls).toEqual([
        {
          cmd: "workspace_files_rename",
          args: { plane: PLANE, workspace: "alpha", path: "notes/plan.md", name: "old-plan.md" },
        },
      ]),
    );
  });

  it("renames a file no chat is writing with no question before its name", async () => {
    core();
    draw([WRITING]);
    await userEvent.click(await row(/^notes/));

    await pressIn(/^plan\.md/, "Rename…");

    expect(await screen.findByRole("dialog", { name: "Rename plan.md" })).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("says in a delete's one confirmation that a chat is writing the file", async () => {
    core();
    draw([WRITING]);
    await userEvent.click(await row(/^notes/));
    await row(/^plan\.md/);
    await act(async () => {
      await emit(
        ...onTheWire("chat-touching", {
          plane: PLANE,
          session: 3,
          path: "notes/plan.md",
        } satisfies ChatTouching),
      );
      theFrameNow();
    });

    await pressIn(/^plan\.md/, "Delete…");

    const asked = await screen.findByRole("alertdialog", { name: "Delete plan.md?" });
    expect(asked).toHaveTextContent(
      "steward 3 is working here now, and the chat may write it again.",
    );
  });
});
