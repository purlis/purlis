// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FileScope, FilesFound, FolderEntry, Panels, PlaneId } from "./bindings";
import { forgetProjectThemes } from "./projectTheme";
import type { WorkspaceState } from "./workspaceState";

/**
 * **Explorer opens straight to the files** (#3343, the sidebar-views prototype's Q2 A, "Repos as
 * folders"): no *Files* fold, one repo's tree open at once with its branch beside the title,
 * several repos a closed row each, and the filter held in the view's top, finding by name in the
 * selected repo or all of them, with *Search contents* opening Search on the same scope.
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
    repos: repos.map((name) => ({ name, branch: name === "svc" ? "main" : "dev" })),
    cache_refused: null,
  } as unknown as WorkspaceState["repos"],
  pieces: Object.fromEntries(repos.map((repo) => [repo, []])),
  piecesRefused: {},
  reading: false,
});

const file = (name: string): FolderEntry => ({ name, kind: "file", ignored: false, refused: null });

/** The core: each repo's own folder holds `README.md`; `svc` changed two files; `find_files`
 *  answers one hit per branch the scope names. */
function core() {
  const finds: FileScope[] = [];
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd === "branch_tree") return { entries: [file("README.md")], more: 0 };
      if (cmd === "branch_status")
        return a.repo === "svc"
          ? {
              refused: [],
              changes: [
                { path: "a.rs", mark: "changed", from: null, uncommitted: true },
                { path: "b.rs", mark: "changed", from: null, uncommitted: true },
              ],
              folders: [{ folder: "", mark: "changed", count: 2 }],
              more: 0,
              base: "main",
            }
          : { refused: [], changes: [], folders: [], more: 0, base: "main" };
      if (cmd === "find_files") {
        const scope = a.scope as FileScope;
        finds.push(scope);
        const repos = scope.kind === "branch" ? [scope.repo] : ["svc", "web"];
        const found: FilesFound = {
          files: repos.map((repo) => ({
            plane: PLANE,
            workspace: "alpha",
            repo,
            piece: null,
            path: `src/${repo}-main.rs`,
            matched: [4],
          })),
          branches: repos.length,
          refused: [],
          partial: [],
        };
        return found;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return finds;
}

function draw(repos: string[], onSearchContents = vi.fn(), onOpenFile = vi.fn()) {
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
        onSearchContents={onSearchContents}
      />
    </ChatsHere.Provider>,
  );
  return { onSearchContents, onOpenFile };
}

const tree = () => screen.getByRole("treegrid", { name: "Files of alpha" });

describe("Explorer, repos as folders (#3343)", () => {
  it("has no Files fold: the view's title is the workspace and the tree is the view", () => {
    core();
    draw(["svc"]);

    expect(screen.queryByRole("button", { name: /^Files/ })).toBeNull();
    expect(screen.getByRole("heading", { name: "alpha" })).toBeInTheDocument();
    expect(screen.getByTestId("explorer").querySelector("[data-frame]")).not.toBeNull();
  });

  it("opens one repo's tree at once, the repo and its branch beside the title", async () => {
    core();
    draw(["svc"]);

    expect(await within(tree()).findByRole("row", { name: /^README\.md/ })).toBeInTheDocument();
    const head = screen.getByRole("heading", { name: "alpha" }).parentElement as HTMLElement;
    expect(head).toHaveTextContent("svc · main");
    // One repo: no scope to switch, and the note says the filter covers all of it.
    expect(screen.queryByRole("radiogroup", { name: "Filter the files of" })).toBeNull();
    expect(screen.getByText(/One repo, so the filter covers all of it/)).toBeInTheDocument();
  });

  it("draws several repos as closed rows with their changes and branch, opening in place", async () => {
    core();
    draw(["svc", "web"]);

    const svc = within(tree()).getByRole("row", { name: /^svc/ });
    const web = within(tree()).getByRole("row", { name: /^web/ });
    expect(svc).toHaveAttribute("aria-expanded", "false");
    expect(web).toHaveAttribute("aria-expanded", "false");
    expect(svc).toHaveTextContent("main");
    expect(web).toHaveTextContent("dev");
    await waitFor(() => expect(within(svc).getByLabelText("2 changes")).toBeInTheDocument());
    expect(within(tree()).queryByRole("row", { name: /^README/ })).toBeNull();

    await userEvent.click(web);

    expect(await within(tree()).findByRole("row", { name: /^README\.md/ })).toBeInTheDocument();
    expect(within(tree()).getByRole("row", { name: /^web/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  it("finds by name in the selected repo, or in all of them, and opens a file found", async () => {
    const finds = core();
    const { onOpenFile } = draw(["svc", "web"]);
    // The repo pressed last is the selected one.
    await userEvent.click(within(tree()).getByRole("row", { name: /^web/ }));
    expect(screen.getByRole("radio", { name: "Selected: web" })).toBeChecked();
    // Drawn as the scope's, never as where the next chat starts.
    const web = within(tree()).getByRole("row", { name: /^web/ });
    expect(web).toHaveAttribute("data-in-scope");
    expect(web).not.toHaveAttribute("aria-current");

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "main");

    const found = await screen.findByRole("grid", { name: "Files found" });
    expect(finds.at(-1)).toMatchObject({ kind: "branch", repo: "web", piece: null });
    expect(within(found).getAllByRole("row", { name: /main\.rs/ })).toHaveLength(1);

    await userEvent.click(screen.getByRole("radio", { name: "All repos" }));

    await waitFor(() =>
      expect(
        within(screen.getByRole("grid", { name: "Files found" })).getAllByRole("row", {
          name: /main\.rs/,
        }),
      ).toHaveLength(2),
    );
    expect(finds.at(-1)).toMatchObject({ kind: "workspace", workspace: "alpha" });
    expect(screen.getByRole("searchbox", { name: "Filter files" })).toHaveAttribute(
      "placeholder",
      "Filter all repos",
    );

    await userEvent.click(screen.getByRole("row", { name: /^svc-main\.rs/ }));
    expect(onOpenFile).toHaveBeenCalledWith(
      { workspace: "alpha", repo: "svc", piece: null },
      "src/svc-main.rs",
    );
  });

  it("opens Search on the filter's scope: Branch for the selected repo, Workspace for all", async () => {
    core();
    const { onSearchContents } = draw(["svc", "web"]);

    await userEvent.click(screen.getByRole("button", { name: /Search contents in Branch/ }));
    expect(onSearchContents).toHaveBeenLastCalledWith(
      "branch",
      { workspace: "alpha", repo: "svc", piece: null },
      "",
    );

    await userEvent.click(screen.getByRole("radio", { name: "All repos" }));
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "lib");
    await userEvent.click(screen.getByRole("button", { name: /Search contents in Workspace/ }));
    expect(onSearchContents).toHaveBeenLastCalledWith(
      "workspace",
      { workspace: "alpha", repo: "svc", piece: null },
      "lib",
    );
  });
});
