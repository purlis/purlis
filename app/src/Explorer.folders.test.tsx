// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { Explorer, type SearchKeyHere } from "./Explorer";
import { cascadeOf } from "./cascade.testkit";
import { inherited } from "./textRoles.testkit";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type {
  BranchChanged,
  FileScope,
  FilesFound,
  FolderEntry,
  Panels,
  PlaneId,
  RepoChanged,
} from "./bindings";
import { forgetProjectThemes } from "./projectTheme";
import type { WorkspaceState } from "./workspaceState";
import { findGrid } from "./ui/table/table.testkit";

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

const stateOf = (repos: string[], workspace = "alpha"): WorkspaceState => ({
  panels: { ...panels(repos), workspace },
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

/** Each command the core was asked, in order, with what names its repos. */
let asked: string[] = [];

/** The core: each repo's own folder holds `README.md`; `svc` changed two files; `find_files`
 *  answers one hit per branch the scope names. */
function core(refused: string[] = [], unreadable?: string) {
  const finds: FileScope[] = [];
  asked = [];
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, unknown>;
      if (cmd === "branch_status") asked.push(`branch_status:${String(a.repo)}`);
      if (cmd === "repos_changed") {
        const repos = a.repos as string[];
        asked.push(`repos_changed:${repos.join(",")}`);
        return repos.map((repo): RepoChanged => ({
          repo,
          changes: repo === "svc" ? { folder: "", mark: "changed", count: 2 } : null,
          trouble: null,
        }));
      }
      if (cmd === "branch_tree" && a.folder === unreadable) throw "permission denied";
      if (cmd === "branch_tree")
        return {
          entries:
            a.folder === "src"
              ? [file("lib.rs")]
              : [
                  { ...file("src"), kind: "folder" },
                  file("README.md"),
                  file(`${String(a.workspace)}.md`),
                ],
          more: 0,
        };
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
          refused,
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

function draw(
  repos: string[],
  onSearchContents = vi.fn(),
  onOpenFile = vi.fn(),
  onSearchKey?: (here: SearchKeyHere | undefined) => void,
) {
  const drawn = (workspace: string, them: string[]) => (
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        part="explorer"
        plane={PLANE}
        workspace={workspace}
        state={stateOf(them, workspace)}
        chats={[]}
        spot={undefined}
        onPick={() => {}}
        offers={new Map()}
        onPress={() => {}}
        onReadAgain={() => {}}
        onOpenFile={onOpenFile}
        onSearchContents={onSearchContents}
        onSearchKey={onSearchKey}
      />
    </ChatsHere.Provider>
  );
  const { rerender } = render(drawn("alpha", repos));
  return {
    onSearchContents,
    onOpenFile,
    /** The strip moved to another workspace. */
    goTo: (workspace: string, them: string[]) => rerender(drawn(workspace, them)),
  };
}

const panelCss = cascadeOf("ui/panel/panel.css");

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
    // One repo: no scope to switch, and the note says the filter covers it and the workspace's
    // own files (#5375).
    expect(screen.queryByRole("radiogroup", { name: "Filter the files of" })).toBeNull();
    expect(
      screen.getByText(/One repo, so the filter covers it and the workspace's files/),
    ).toBeInTheDocument();
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

  it("counts every closed row in one ask, reading no repo's status, and asks again when one moves (#3489)", async () => {
    // A wide workspace: twenty repos, each a closed row with its count. Their counts are one ask
    // of the core, which keeps each repo's answer until it moves, never a status read per repo.
    const repos = ["svc", ...Array.from({ length: 19 }, (_, at) => `repo${at}`)];
    core();
    draw(repos);

    const svc = within(tree()).getByRole("row", { name: /^svc/ });
    await waitFor(() => expect(within(svc).getByLabelText("2 changes")).toBeInTheDocument());
    expect(asked.filter((one) => one.startsWith("branch_status"))).toEqual([]);
    expect(asked.filter((one) => one.startsWith("repos_changed"))).toEqual([
      `repos_changed:${repos.join(",")}`,
    ]);

    // Something moved in one closed repo: the counts are asked again, in one ask.
    await act(() =>
      emit("branch-changed", {
        branches: [{ plane: PLANE, workspace: "alpha", repo: "repo3", piece: null }],
      } satisfies BranchChanged),
    );
    await waitFor(() =>
      expect(asked.filter((one) => one.startsWith("repos_changed"))).toHaveLength(2),
    );
    expect(asked.filter((one) => one.startsWith("branch_status"))).toEqual([]);
  });

  it("reads a repo's status once it is opened, and only that repo's (#3489)", async () => {
    core();
    draw(["svc", "web"]);
    await waitFor(() => expect(asked.some((one) => one.startsWith("repos_changed"))).toBe(true));

    await userEvent.click(within(tree()).getByRole("row", { name: /^web/ }));

    await waitFor(() => expect(asked).toContain("branch_status:web"));
    expect(asked.filter((one) => one.startsWith("branch_status"))).toEqual(["branch_status:web"]);
  });

  it("finds by name among the workspace's files, or in all the repos, and opens a file found", async () => {
    const finds = core();
    const { onOpenFile } = draw(["svc", "web"]);
    // A repo's row pressed opens it in place and leaves *Selected* the workspace's own files
    // (ADR 0038, amended 2026-11-05).
    await userEvent.click(within(tree()).getByRole("row", { name: /^web/ }));
    expect(screen.getByRole("radio", { name: "Selected: alpha" })).toBeChecked();

    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "main");

    const found = await findGrid(screen, "Files found");
    expect(finds.at(-1)).toMatchObject({ kind: "branch", repo: "", piece: null });
    // One place's files are no group: no name over them, as in the prototype.
    expect(found.querySelector('[data-part="header"]')).toBeNull();
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
    expect(
      [
        ...screen
          .getByRole("grid", { name: "Files found" })
          .querySelectorAll('[data-part="header"]'),
      ].map((one) => one.textContent),
    ).toEqual(["svc1", "web1"]);
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

  it("says a branch it could not look in, in the danger colour (#3917)", async () => {
    const why = "web in workspace 'alpha' has no branch folder called 'gone'";
    core([why]);
    draw(["svc", "web"]);
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "main");
    const said = await screen.findByText(why);
    expect(said).toHaveAttribute("role", "alert");
    // No role of its own: a role's colour is in a later layer than App.css's and would draw it
    // secondary. It takes the view's size, and the words that say something is wrong are
    // `danger.text` (`docs/ui-primitives.md`), held on the trouble band's `danger.surface`.
    expect(inherited(said, "color")).toBe("var(--danger-text)");
  });

  it("says a folder it could not read in the danger colour, its mark in the failed one (#3938)", async () => {
    core([], "src");
    draw(["svc"]);
    await userEvent.click(await within(tree()).findByRole("row", { name: /^src/ }));
    const said = await within(tree()).findByText(/permission denied/);
    // Words that say something is wrong are `danger.text`; `state.failed` is the mark's.
    expect(inherited(said, "color")).toBe("var(--danger-text)");
    const mark = said.parentElement?.querySelector(".node-icon");
    expect(mark && inherited(mark, "color")).toBe("var(--state-failed)");
  });

  it("opens Search on the filter's scope: none for the workspace's files, Workspace for all", async () => {
    core();
    const { onSearchContents } = draw(["svc", "web"]);
    // *Selected* is the workspace's own files (#5375), which content search does not read, and
    // opening a repo's row leaves it so (ADR 0038, amended 2026-11-05).
    expect(screen.queryByRole("button", { name: /Search contents in Branch/ })).toBeNull();
    await userEvent.click(within(tree()).getByRole("row", { name: /^svc/ }));
    expect(screen.queryByRole("button", { name: /Search contents in Branch/ })).toBeNull();

    await userEvent.click(screen.getByRole("radio", { name: "All repos" }));
    await userEvent.type(screen.getByRole("searchbox", { name: "Filter files" }), "lib");
    await userEvent.click(screen.getByRole("button", { name: /Search contents in Workspace/ }));
    expect(onSearchContents).toHaveBeenLastCalledWith(
      "workspace",
      { workspace: "alpha", repo: "svc", piece: null },
      "lib",
    );
  });

  it("leaves Selected the workspace's files when a repo is opened by its arrow, as a press does", async () => {
    core();
    draw(["svc", "web"]);
    const web = within(tree()).getByRole("row", { name: /^web/ });
    web.focus();

    await userEvent.keyboard("{ArrowRight}");

    await waitFor(() => expect(web).toHaveAttribute("aria-expanded", "true"));
    expect(screen.getByRole("radio", { name: "Selected: alpha" })).toBeChecked();
  });

  it("keeps a row the keyboard comes to clear of the held top (WCAG 2.4.11)", async () => {
    core();
    draw(["svc"]);
    const row = await within(tree()).findByRole("row", { name: /^README\.md/ });

    expect(row.closest("[data-frame]")).not.toBeNull();
    expect(panelCss(row, "scroll-margin-block-start")).toBe("var(--panel-section-top-size)");
  });

  it("collapses every folder it opened, and keeps the one repo's tree open", async () => {
    core();
    draw(["svc"]);
    await userEvent.click(await within(tree()).findByRole("row", { name: /^src/ }));
    expect(await within(tree()).findByRole("row", { name: /^lib\.rs/ })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Collapse all folders" }));

    await waitFor(() => expect(within(tree()).queryByRole("row", { name: /^lib\.rs/ })).toBeNull());
    expect(within(tree()).getByRole("row", { name: /^src/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(within(tree()).getByRole("row", { name: /^README\.md/ })).toBeInTheDocument();
  });

  it("draws another workspace's one repo as its own rows, though the repo has the same name", async () => {
    core();
    const { goTo } = draw(["svc"]);
    expect(await within(tree()).findByRole("row", { name: /^alpha\.md/ })).toBeInTheDocument();

    // The rows' ids are the workspace's: the tree family never changes an id in place (D-3343-7).
    goTo("beta", ["svc"]);

    const beta = screen.getByRole("treegrid", { name: "Files of beta" });
    expect(await within(beta).findByRole("row", { name: /^beta\.md/ })).toBeInTheDocument();
    expect(within(beta).queryByRole("row", { name: /^alpha\.md/ })).toBeNull();
  });

  it("steps from the filter into the files it found with ↓", async () => {
    core();
    draw(["svc", "web"]);
    const box = screen.getByRole("searchbox", { name: "Filter files" });
    await userEvent.type(box, "main");
    const found = await findGrid(screen, "Files found");
    await within(found).findByRole("row", { name: /main\.rs/ });

    await userEvent.keyboard("{ArrowDown}");

    expect(found).toContainElement(document.activeElement as HTMLElement);
  });

  it("hands ⌘⇧F the filter's scope and words while the keyboard is in its view", async () => {
    core();
    const keys: (SearchKeyHere | undefined)[] = [];
    const { onSearchContents } = draw(["svc", "web"], vi.fn(), vi.fn(), (here) => keys.push(here));
    await userEvent.click(screen.getByRole("radio", { name: "All repos" }));
    const box = screen.getByRole("searchbox", { name: "Filter files" });
    await userEvent.type(box, "lib");

    const here = keys.at(-1);
    expect(here?.inside?.contains(box)).toBe(true);
    here?.run();

    expect(onSearchContents).toHaveBeenLastCalledWith(
      "workspace",
      { workspace: "alpha", repo: "svc", piece: null },
      "lib",
    );
  });
});
