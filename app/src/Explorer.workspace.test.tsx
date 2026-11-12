// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type {
  FileScope,
  FilesFound,
  FolderEntry,
  Panels,
  PlaneId,
  WorkspaceListing,
} from "./bindings";
import { forgetProjectThemes } from "./projectTheme";
import type { WorkspaceState } from "./workspaceState";
import { findGrid } from "./ui/table/table.testkit";

/**
 * **With the workspace picked, Explorer shows its folder** (#5375, ADR 0038 as amended
 * 2026-11-05): the workspace's own files as on disk, its repos as rows where they stand — in
 * `repos/`, or at the top for a clone made before structure version 6 — what purlis keeps
 * marked *managed*, and dotfiles only with Show hidden. What purlis keeps is never written; the
 * person's own rows' writes are `Explorer.writes.test.tsx`'s (#5376).
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
});

const PLANE = "/plane" as unknown as PlaneId;

const panels = (repos: string[]): Panels => ({
  workspace: "alpha",
  repos,
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
});

const stateOf = (repos: string[]): WorkspaceState => ({
  panels: panels(repos),
  repos: {
    workspace: "alpha",
    repos: repos.map((name) => ({ name, branch: "main" })),
    cache_refused: null,
  } as unknown as WorkspaceState["repos"],
  pieces: Object.fromEntries(repos.map((repo) => [repo, []])),
  piecesRefused: {},
  reading: false,
});

const entry = (name: string, more: Partial<FolderEntry> = {}): FolderEntry => ({
  name,
  kind: "file",
  ignored: false,
  refused: null,
  ...more,
});

/** The workspace's own folder: what purlis keeps, a dotfile and the person's notes. `api` is in
 *  `repos/`, `svc` an old clone at the top. */
function core(inRepos: string[] = ["api"], atTop: string[] = ["svc"]) {
  const finds: FileScope[] = [];
  const asked: string[] = [];
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      asked.push(cmd);
      if (cmd === "workspace_tree") {
        const listing: WorkspaceListing =
          a.folder === ""
            ? {
                entries: [
                  entry(".claude", { kind: "folder", managed: true, hidden: true }),
                  entry("memory", { kind: "folder", managed: true }),
                  entry("notes", { kind: "folder" }),
                  entry(".env", { hidden: true }),
                  entry("workspace.md", { managed: true }),
                ],
                more: 0,
                repos: [
                  ...inRepos.map((name) => ({ name, inside_repos: true })),
                  ...atTop.map((name) => ({ name, inside_repos: false })),
                ],
              }
            : { entries: [entry("plan.md")], more: 0, repos: [] };
        return listing;
      }
      if (cmd === "branch_tree") return { entries: [entry("README.md")], more: 0 };
      if (cmd === "branch_status")
        return { refused: [], changes: [], folders: [], more: 0, base: "main" };
      if (cmd === "find_files") {
        const scope = a.scope as FileScope;
        finds.push(scope);
        const found: FilesFound = {
          files: [
            {
              plane: PLANE,
              workspace: "alpha",
              repo: "",
              piece: null,
              path: "notes/plan.md",
              matched: [6],
            },
            ...(scope.kind === "workspace-folder"
              ? [
                  {
                    plane: PLANE,
                    workspace: "alpha",
                    repo: "api",
                    piece: null,
                    path: "docs/plan.md",
                    matched: [5],
                  },
                ]
              : []),
          ],
          branches: 1,
          refused: [],
          partial: [],
        };
        return found;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return { finds, asked };
}

function draw(repos: string[], onOpenFile = vi.fn()) {
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        part="explorer"
        plane={PLANE}
        workspace="alpha"
        state={stateOf(repos)}
        chats={[]}
        spot={undefined}
        onPick={() => {}}
        offers={new Map()}
        onPress={() => {}}
        onReadAgain={() => {}}
        onOpenFile={onOpenFile}
      />
    </ChatsHere.Provider>,
  );
  return { onOpenFile };
}

const tree = () => screen.getByRole("treegrid", { name: "Files of alpha" });
const row = (name: RegExp) => within(tree()).findByRole("row", { name });

describe("Explorer, the workspace's own folder (#5375)", () => {
  it("draws the workspace's files and its repos, a repo in repos/ under that folder", async () => {
    core();
    draw(["api", "svc"]);

    expect(await row(/^notes/)).toBeInTheDocument();
    expect(await row(/^workspace\.md/)).toBeInTheDocument();
    // `svc` is an old clone at the top: a row of its own there, closed, of several repos.
    const svc = await row(/^svc/);
    expect(svc).toHaveAttribute("aria-level", "1");
    expect(svc).toHaveAttribute("aria-expanded", "false");
    // `api` is in `repos/`: no row at the top until `repos/` is opened.
    const repos = await row(/^repos/);
    expect(repos).toHaveAttribute("aria-expanded", "false");
    expect(within(tree()).queryByRole("row", { name: /^api/ })).toBeNull();

    await userEvent.click(repos);

    const api = await row(/^api/);
    expect(api).toHaveAttribute("aria-level", "2");
    expect(api).toHaveAttribute("aria-expanded", "false");
  });

  it("opens the one repo's row, and repos/ above it, so its files show with no click (#3343)", async () => {
    core(["api"], []);
    draw(["api"]);

    expect(await row(/^README\.md/)).toBeInTheDocument();
    expect(await row(/^repos/)).toHaveAttribute("aria-expanded", "true");
    expect(await row(/^api/)).toHaveAttribute("aria-expanded", "true");
    // And the workspace's own files beside it.
    expect(await row(/^notes/)).toBeInTheDocument();
  });

  it("marks what purlis keeps managed, in words a screen reader hears", async () => {
    core();
    draw(["api", "svc"]);

    const memory = await row(/^memory managed/);
    expect(memory).toHaveAttribute("data-managed");
    expect(within(memory).getByText("managed")).toBeInTheDocument();
    expect(await row(/^workspace\.md managed/)).toHaveAttribute("data-managed");
    expect(await row(/^notes/)).not.toHaveAttribute("data-managed");
  });

  it("draws a dotfile only with Show hidden", async () => {
    core();
    draw(["api", "svc"]);
    await row(/^notes/);
    expect(within(tree()).queryByRole("row", { name: /^\.env/ })).toBeNull();
    expect(within(tree()).queryByRole("row", { name: /^\.claude/ })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Show hidden" }));

    expect(await row(/^\.env/)).toBeInTheDocument();
    expect(await row(/^\.claude managed/)).toBeInTheDocument();
  });

  it("opens the person's own file as one of the workspace's files", async () => {
    core();
    const { onOpenFile } = draw(["api", "svc"]);

    await userEvent.click(await row(/^notes/));
    await userEvent.click(await row(/^plan\.md/));

    expect(onOpenFile).toHaveBeenCalledWith(
      { workspace: "alpha", repo: "", piece: null },
      "notes/plan.md",
    );
  });

  it("reads the workspace's folder by its own command, and compares nothing of it", async () => {
    const { asked } = core();
    draw(["api", "svc"]);
    await row(/^notes/);

    expect(asked).toContain("workspace_tree");
    // The repos' rows are counted in one ask, reading no repo's status (#3489); the workspace's
    // own files are no branch.
    await waitFor(() => expect(asked).toContain("repos_changed"));
    expect(asked).not.toContain("branch_status");
    expect(asked).not.toContain("piece_file");
  });

  it("finds among the workspace's files with several repos, and in every repo with All repos", async () => {
    const { finds } = core();
    draw(["api", "svc"]);
    expect(screen.getByRole("radio", { name: "Selected: alpha" })).toBeChecked();

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "plan");

    await findGrid(screen, "Files found");
    expect(finds.at(-1)).toMatchObject({
      kind: "branch",
      workspace: "alpha",
      repo: "",
      piece: null,
    });

    await userEvent.click(screen.getByRole("radio", { name: "All repos" }));
    await waitFor(() => expect(finds.at(-1)).toMatchObject({ kind: "workspace" }));
  });

  it("with one repo, finds among the workspace's files and that repo, with no switch", async () => {
    const { finds } = core(["api"], []);
    draw(["api"]);
    expect(screen.queryByRole("radiogroup", { name: "Filter the files of" })).toBeNull();

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "plan");

    await findGrid(screen, "Files found");
    expect(finds.at(-1)).toEqual({ kind: "workspace-folder", plane: PLANE, workspace: "alpha" });
  });

  it("keeps Selected on the workspace's files when a repo's row is opened (ADR 0038, 2026-11-05)", async () => {
    const { finds } = core();
    draw(["api", "svc"]);
    // `api` is in `repos/`, closed with several repos.
    await userEvent.click(await row(/^repos/));
    await userEvent.click(await row(/^api/));
    await row(/^README\.md/);

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "plan");

    const found = await findGrid(screen, "Files found");
    expect(screen.getByRole("radio", { name: "Selected: alpha" })).toBeChecked();
    expect(finds.at(-1)).toMatchObject({ kind: "branch", repo: "", piece: null });
    expect(await within(found).findByRole("row", { name: /^plan\.md/ })).toHaveTextContent("notes");
  });

  it("with one repo, says which hits are the workspace's and which the repo's", async () => {
    core(["api"], []);
    draw(["api"]);

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "plan");

    const found = await findGrid(screen, "Files found");
    const own = await within(found).findByRole("rowgroup", { name: /^alpha/ });
    const repo = within(found).getByRole("rowgroup", { name: /^api/ });
    expect(within(own).getByRole("row", { name: /^plan\.md/ })).toHaveTextContent("notes");
    expect(within(repo).getByRole("row", { name: /^plan\.md/ })).toHaveTextContent("docs");
  });

  it("offers no write on what purlis keeps, a file or a folder (#5376)", async () => {
    core();
    draw(["api", "svc"]);
    await row(/^notes/);

    const writes = [/new file/i, /new folder/i, /rename/i, /trash/i, /import/i, /delete/i];

    // The person's own rows are written from `Explorer.writes.test.tsx`; these are purlis's.
    for (const name of [/^workspace\.md/, /^memory/]) {
      fireEvent.contextMenu(await row(name));
      const menu = await screen.findByRole("menu");
      const items = within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label") ?? one.textContent ?? "");
      expect(items.length).toBeGreaterThan(0);
      expect(items.filter((item) => writes.some((write) => write.test(item)))).toEqual([]);
      await userEvent.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    }
  });
});
