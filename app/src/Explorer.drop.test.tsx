// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { Explorer } from "./Explorer";
import { perform, type Doing } from "./actions";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FolderEntry, Panels, PlaneId, WorkspaceListing } from "./bindings";
import { forgetProjectThemes } from "./projectTheme";
import { rowDrop } from "./finderDrop";
import { useWorkspaceWrites } from "./WorkspaceWrites";
import type { WorkspaceState } from "./workspaceState";

/**
 * **Files dropped from the file manager onto the workspace's own files are imported** (#5378):
 * a drop on a folder of the person's own imports into it, on no row into the workspace's top,
 * and a drop on what purlis keeps, a repo or `repos/` is refused with why. The row under the
 * drag is drawn as its target. The system's drag events are mocked, and so is the core: what
 * it copies, and refuses, is `the_persons_workspace_files_are_written_and_nothing_else.rs`'s.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
  vi.restoreAllMocks();
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

type Call = { cmd: string; args: Record<string, unknown> };

/** The workspace's folder: `memory/` and `workspace.md` purlis keeps, the person's `notes/`
 *  holding `plan.md`, and repo `api` in `repos/`. `refuse` makes the import refuse. */
function core(refuse?: string) {
  const calls: Call[] = [];
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd.startsWith("workspace_files_")) {
        calls.push({ cmd, args: a });
        if (refuse !== undefined) throw refuse;
        return [`${String(a.folder)}/a.txt`];
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
  mockWindows("main");
  return calls;
}

/** Explorer, its drops and rows' writes carried out as the window carries them out. */
function Window({ hears = true }: { hears?: boolean }) {
  const writes = useWorkspaceWrites(PLANE);
  const doing = { askWorkspaceWrite: writes.ask } as unknown as Doing;
  return (
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
        onPress={(offer) => void perform(offer, doing)}
        onReadAgain={() => {}}
        onOpenFile={() => {}}
        onDropFiles={hears ? writes.drop : undefined}
      />
      {writes.dialog}
    </ChatsHere.Provider>
  );
}

const tree = () => screen.getByRole("treegrid", { name: "Files of alpha" });
const row = (name: RegExp) => within(tree()).findByRole("row", { name });

/** What the page draws at a point: `under`, wherever the drag is. */
function pointAt(under: () => Element) {
  return vi
    .spyOn(document, "elementFromPoint")
    .mockImplementation(() => under() as HTMLElement as never);
}

const POINT = { x: 40, y: 80 };

/** The system says a drag of `paths` has entered, is `over`, `dropped`, or has left. */
async function drag(type: "enter" | "over" | "drop" | "leave", paths = ["/Users/me/a.txt"]) {
  const event = `tauri://drag-${type}`;
  const payload =
    type === "leave" ? null : { paths: type === "over" ? undefined : paths, position: POINT };
  await act(async () => {
    await emit(event, payload);
  });
}

const dropTargets = () => document.querySelectorAll("[data-drop-target]");

describe("Explorer, files dropped from the file manager (#5378)", () => {
  // jsdom draws nothing, so it has no `elementFromPoint` of its own to spy on.
  document.elementFromPoint ??= () => null;

  it("imports a drop on a folder of the person's own into that folder", async () => {
    const calls = core();
    render(<Window />);
    const notes = await row(/^notes/);
    pointAt(() => notes);

    await drag("drop");

    await waitFor(() =>
      expect(calls).toEqual([
        {
          cmd: "workspace_files_import_dropped",
          args: { plane: PLANE, workspace: "alpha", folder: "notes" },
        },
      ]),
    );
  });

  it("imports a drop on no row into the workspace's top", async () => {
    const calls = core();
    render(<Window />);
    await row(/^notes/);
    pointAt(tree);

    await drag("drop");

    await waitFor(() =>
      expect(calls.map((one) => one.args)).toEqual([
        { plane: PLANE, workspace: "alpha", folder: "" },
      ]),
    );
  });

  it("imports a drop on a file of the person's own into the folder that holds it", async () => {
    const calls = core();
    render(<Window />);
    await userEvent.click(await row(/^notes/));
    const plan = await row(/^plan\.md/);
    pointAt(() => plan);

    await drag("drop");

    await waitFor(() => expect(calls.map((one) => one.args.folder)).toEqual(["notes"]));
  });

  // wry tells a drag's point in the view's own points on macOS (`draggingLocation`) and in
  // GTK's logical pixels on Linux, which are the page's; on Windows in the client area's
  // physical pixels (`ScreenToClient`), which the page's scale divides.
  it.each([
    ["MacIntel", { x: 40, y: 80 }],
    ["Linux x86_64", { x: 40, y: 80 }],
    ["Win32", { x: 20, y: 40 }],
  ])("reads the system's point on %s as the page's at %o", async (platform, page) => {
    core();
    render(<Window />);
    await row(/^notes/);
    vi.spyOn(navigator, "platform", "get").mockReturnValue(platform);
    vi.spyOn(window, "devicePixelRatio", "get").mockReturnValue(2);
    const at = pointAt(tree);

    await drag("enter");

    expect(at).toHaveBeenCalledWith(page.x, page.y);
  });

  it("hears nothing of a drag that carries no files, as a drag inside the window does", async () => {
    const calls = core();
    render(<Window />);
    const memory = await row(/^memory/);
    const notes = await row(/^notes/);
    let under: Element = notes;
    pointAt(() => under);

    await drag("enter", []);
    await drag("over");
    expect(dropTargets()).toHaveLength(0);

    under = memory;
    await drag("drop", []);

    expect(dropTargets()).toHaveLength(0);
    expect(calls).toEqual([]);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it.each([
    [/^memory/, "purlis keeps memory itself, and imports nothing into it."],
    [/^workspace\.md/, "purlis keeps workspace.md itself, and imports nothing into it."],
    [/^api/, "purlis imports nothing into the repo api."],
    [/^README\.md/, "purlis imports nothing into the repo api."],
    [/^repos/, "purlis imports nothing into repos/, which holds the workspace's repos."],
  ])("refuses a drop on %s and says why, importing nothing", async (name, why) => {
    const calls = core();
    render(<Window />);
    const target = await row(name);
    pointAt(() => target);

    await drag("drop");

    const said = await screen.findByRole("alertdialog", { name: "The drop did not finish" });
    expect(within(said).getByRole("alert")).toHaveTextContent(why);
    expect(calls).toEqual([]);
  });

  it("says the core's refusal of a drop as Import files… says it", async () => {
    const refused = "purlis copied in 'notes/a.txt', and then stopped: 'b' is not a file.";
    core(refused);
    render(<Window />);
    const notes = await row(/^notes/);
    pointAt(() => notes);

    await drag("drop");

    const said = await screen.findByRole("alertdialog", { name: "The drop did not finish" });
    expect(within(said).getByRole("alert")).toHaveTextContent(refused);
  });

  it("draws the folder under the drag as its target, and nothing once it leaves", async () => {
    core();
    render(<Window />);
    const notes = await row(/^notes/);
    let under: Element = notes;
    pointAt(() => under);

    await drag("enter");
    expect(notes).toHaveAttribute("data-drop-target");

    under = tree();
    await drag("over");
    expect([...dropTargets()]).toEqual([tree()]);

    await drag("leave");
    expect(dropTargets()).toHaveLength(0);
  });

  it("draws a file's folder as the target, a refused row as none, and none after the drop", async () => {
    core();
    render(<Window />);
    const notes = await row(/^notes/);
    await userEvent.click(notes);
    const plan = await row(/^plan\.md/);
    const memory = await row(/^memory/);
    let under: Element = plan;
    pointAt(() => under);

    await drag("enter");
    expect([...dropTargets()]).toEqual([notes]);

    under = memory;
    await drag("over");
    expect(dropTargets()).toHaveLength(0);

    under = notes;
    await drag("over");
    await drag("drop");
    expect(dropTargets()).toHaveLength(0);
  });

  it("leaves a drop outside the workspace's files alone", async () => {
    const calls = core();
    render(<Window />);
    await row(/^notes/);
    pointAt(() => document.body);

    await drag("enter");
    await drag("over");
    await drag("drop");

    expect(dropTargets()).toHaveLength(0);
    expect(calls).toEqual([]);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("hears no drop where the window takes none", async () => {
    const calls = core();
    render(<Window hears={false} />);
    const notes = await row(/^notes/);
    pointAt(() => notes);

    await drag("enter");
    await drag("over");
    await drag("drop");

    expect(dropTargets()).toHaveLength(0);
    expect(calls).toEqual([]);
  });
});

describe("rowDrop", () => {
  const ws = { workspace: "alpha", repo: "", piece: null };

  it("refuses a branch's folder", () => {
    expect(
      rowDrop({ workspace: "alpha", repo: "api", piece: "fix" }, "src", {
        folder: true,
        name: "src",
      }),
    ).toEqual({
      refused:
        "purlis imports nothing into a branch's folder, which its chats work in. Drop the files on a folder of your own.",
    });
  });

  it("takes no drop at the plane root", () => {
    expect(
      rowDrop({ workspace: "", repo: "", piece: null }, "notes", { folder: true, name: "notes" }),
    ).toBeUndefined();
  });

  it("lands a drop on a file at the top in the top", () => {
    expect(rowDrop(ws, "todo.md", { folder: false, name: "todo.md" })).toEqual({ into: "" });
  });
});
