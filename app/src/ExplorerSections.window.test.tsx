// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { goToWorkspace, showTheExplorer, workspaceInFront } from "./test-strips";
import App from "./App";
import { forgetThisLaunch } from "./regions";
import { GLOBAL } from "./windowprefs";
import { tipOf } from "./ui/tooltip/tooltip.testkit";
import { selectNamed } from "./ui/select/select.testkit";

/**
 * **Explorer in sections** (#1677, spec #1671 B-12): the focused workspace's repos and branches,
 * the Repos view's; and *Files*, of where the next chat starts, the Explorer view itself, which
 * since #3343 is no fold: the view's title is the workspace and the tree is the view. A clone's
 * fold is kept in the layout file for the next launch.
 *
 * **It lists no workspaces** (qw358): the strip is where one is selected, and Files is of the
 * one selected there.
 *
 * **On the plane root's tab, Files is the whole project's** (issue 1916): read through the
 * window's own `plane_tree` and `plane_file`, never a branch's commands, with a workspace's
 * repo as one row that selects that workspace's tab.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const at = (workspace: string) => `${PLANE}/workspaces/${workspace}`;

type Asked = { cmd: string; args: Record<string, unknown> };

/** Two workspaces: alpha clones `svc` with one branch, beta clones `web` with none. */
function core() {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return [];
    // Both pinned, so the strip draws a tab for each: it is where a workspace is selected.
    if (cmd === "plane_pins") return { project: false, workspaces: ["alpha", "beta"], missing: [] };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
        workspaces: ["alpha", "beta"].map((name) => ({
          name,
          path: at(name),
          vision: "",
          todos: [],
          chats: [],
        })),
      };
    if (cmd === "workspace_panels") {
      const repo = a.workspace === "beta" ? "web" : "svc";
      return {
        workspace: a.workspace,
        repos: [repo],
        paths: { [repo]: `${at(String(a.workspace))}/${repo}` },
        absent: [],
        refused: [],
        todos: [],
        todos_refused: null,
        personas: ["steward"],
        persona: "steward",
      };
    }
    if (cmd === "workspace_repos")
      return { workspace: a.workspace, repos: [], cache_refused: null };
    if (cmd === "worktree_list")
      return a.workspace === "beta" || a.repo === "web"
        ? []
        : [
            {
              piece: "one",
              path: `${at("alpha")}/.worktrees/svc/one`,
              branch: "one",
              wired: true,
              stale: false,
            },
          ];
    if (cmd === "branch_tree")
      return a.folder === ""
        ? {
            entries:
              a.piece === "one"
                ? [{ name: "lib.rs", kind: "file", ignored: false, refused: null }]
                : [
                    { name: "README.md", kind: "file", ignored: false, refused: null },
                    // Each workspace's own, so two workspaces' trees are told apart.
                    {
                      name: `${String(a.workspace)}.md`,
                      kind: "file",
                      ignored: false,
                      refused: null,
                    },
                  ],
            more: 0,
          }
        : { entries: [], more: 0 };
    if (cmd === "branch_status") return { changes: [], folders: [], more: 0, base: "main" };
    // The plane root's files (issue 1916), as the core answers them: nothing purlis keeps to
    // itself is among them, and a workspace's repo names its workspace.
    if (cmd === "plane_tree") {
      const entry = (name: string, kind: string, more: Record<string, unknown> = {}) => ({
        name,
        kind,
        ignored: false,
        refused: null,
        workspace: null,
        ...more,
      });
      if (a.folder === "")
        return {
          entries: [
            entry("docs", "folder"),
            entry("workspaces", "folder"),
            entry("target", "folder", { ignored: true }),
            entry("README.md", "file"),
          ],
          more: 0,
        };
      if (a.folder === "workspaces") return { entries: [entry("beta", "folder")], more: 0 };
      if (a.folder === "workspaces/beta")
        return {
          entries: [
            entry("web", "folder", {
              refused: "a repo of workspace 'beta', whose files are on that workspace's tab",
              workspace: "beta",
            }),
            entry("workspace.md", "file"),
          ],
          more: 0,
        };
      return { entries: [], more: 0 };
    }
    if (cmd === "plane_file") return { kind: "text", text: "read me\n" };
    if (cmd === "reopened_focus") return null;
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
  Reflect.deleteProperty(globalThis, GLOBAL);
});

const explorer = () => within(screen.getByRole("navigation", { name: "Explorer" }));
/** Goes to a workspace from the title bar's ⌄ (#2916), by its name. */
const goTo = (name: string) => goToWorkspace(name);
/** The Repos view, in the left half of the Panel (#1737): Explorer's *Repos and branches*. */
const reposView = () => within(screen.getByRole("navigation", { name: "Repos" }));

describe("Explorer in sections (#1677)", () => {
  it("draws the files alone, no fold, titled for the strip's workspace, and the repos in the Repos view (#1737, #3343)", async () => {
    core();
    render(<App />);
    await showTheExplorer();
    const tree = await explorer().findByRole("treegrid", { name: "Files of alpha" });

    // No heading folds: the view's one title is its region's, the workspace (#3330).
    expect(
      explorer()
        .queryAllByRole("button", { expanded: true })
        .filter((one) => one.closest("h2") !== null),
    ).toEqual([]);
    expect(screen.getByRole("heading", { name: "alpha" })).toBeVisible();
    // Its one repo is its tree, open at once.
    expect(await within(tree).findByRole("row", { name: /^README\.md/ })).toBeVisible();
    expect(reposView().getByRole("treegrid", { name: "Repos and branches" })).toBeVisible();
  });

  it("has no Workspaces section: the title bar is the one place a workspace is selected (qw358)", async () => {
    core();
    render(<App />);
    await showTheExplorer();
    await explorer().findByRole("treegrid", { name: "Files of alpha" });

    expect(explorer().queryByRole("tree", { name: "Workspaces of this project" })).toBeNull();
    expect(explorer().queryByRole("button", { name: "Workspaces" })).toBeNull();
    expect(explorer().queryByRole("row", { name: /^beta/ })).toBeNull();
    // What its rows carried is the title bar's (#2916): which workspace is in front, and the ⌄
    // to go to another.
    expect(workspaceInFront()).toBe("alpha");
  });

  it("follows the title bar: Files is of the workspace in front", async () => {
    const asked = core();
    render(<App />);
    await showTheExplorer();
    const alphas = await explorer().findByRole("treegrid", { name: "Files of alpha" });
    expect(await within(alphas).findByRole("row", { name: /^README\.md/ })).toBeVisible();

    expect(await within(alphas).findByRole("row", { name: /^alpha\.md/ })).toBeVisible();

    await goTo("beta");

    const betas = await explorer().findByRole("treegrid", { name: "Files of beta" });
    expect(await within(betas).findByRole("row", { name: /^beta\.md/ })).toBeVisible();
    // Beta's own rows, and none of alpha's.
    expect(within(betas).queryByRole("row", { name: /^alpha\.md/ })).toBeNull();
    expect(screen.getByRole("heading", { name: "beta" })).toBeVisible();
    expect(explorer().queryByRole("treegrid", { name: "Files of alpha" })).toBeNull();
    await reposView().findByTestId("clone-web");
    // The extensions heard it, as ever from the strip (charter-app#343).
    expect(
      asked.some((one) => one.cmd === "workspace_focused" && one.args.workspace === "beta"),
    ).toBe(true);
  });

  it("lists the whole project's files in Files when the strip is on the plane root (#1916)", async () => {
    const asked = core();
    render(<App />);
    await showTheExplorer();
    await explorer().findByRole("treegrid", { name: "Files of alpha" });
    const before = asked.length;

    await goTo("Plane root");

    const tree = within(await explorer().findByRole("treegrid", { name: "Files of Plane root" }));
    // The project's own top folder, folders first; what git ignores is hidden until asked for.
    await tree.findByRole("row", { name: "README.md" });
    expect(tree.getAllByRole("row").map((row) => row.textContent?.trim())).toEqual([
      "docs",
      "workspaces",
      "README.md",
    ]);
    expect(explorer().queryByText(/is not a workspace/)).toBeNull();
    // Read through the window's own command, and through no branch's: no workspace or repo
    // is named for the plane root, and nothing of it is compared.
    const since = asked.slice(before);
    expect(since.some((one) => one.cmd === "plane_tree" && one.args.folder === "")).toBe(true);
    expect(
      since.filter(
        (one) =>
          ["branch_tree", "branch_status", "piece_file"].includes(one.cmd) &&
          one.args.workspace === "",
      ),
    ).toEqual([]);

    await userEvent.click(explorer().getByRole("button", { name: "Show ignored files" }));
    await tree.findByRole("row", { name: "target" });

    // And back on a workspace, its files again.
    await goTo("alpha");
    await explorer().findByRole("treegrid", { name: "Files of alpha" });
  });

  it("opens a file of the plane root through the window's own read (#1916)", async () => {
    const asked = core();
    render(<App />);
    await showTheExplorer();
    await goTo("Plane root");
    const tree = within(await explorer().findByRole("treegrid", { name: "Files of Plane root" }));

    await userEvent.click(await tree.findByRole("row", { name: "README.md" }));

    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "plane_file" && one.args.path === "README.md")).toBe(
        true,
      ),
    );
    expect(asked.some((one) => one.cmd === "piece_file")).toBe(false);
    // Its tab is named for the plane root, and offers nothing that names a branch.
    const tab = await screen.findByRole("region", { name: "README.md · Plane root" });
    expect(within(tab).queryByRole("button", { name: "Copy path" })).toBeNull();
    expect(within(tab).queryByRole("button", { name: /your editor/i })).toBeNull();
    // A row of the plane root is read, never dragged onto a chat.
    expect(tree.getByRole("row", { name: "README.md" })).not.toHaveAttribute("draggable");
  });

  it("draws a workspace's repo as one row that brings that workspace forward (#1916)", async () => {
    core();
    render(<App />);
    await showTheExplorer();
    await goTo("Plane root");
    const tree = within(await explorer().findByRole("treegrid", { name: "Files of Plane root" }));
    await userEvent.click(await tree.findByRole("row", { name: "workspaces" }));
    await userEvent.click(await tree.findByRole("row", { name: "beta" }));

    const repo = await tree.findByRole("row", { name: /^web/ });
    // One row, never a folder that opens here: it says whose it is.
    expect(repo).not.toHaveAttribute("aria-expanded");
    expect(repo).not.toHaveAttribute("aria-disabled");
    expect(repo).toHaveTextContent("workspace beta");
    expect(tipOf(repo)).toEqual("A repo of workspace beta: its files are on that workspace's tab");

    await userEvent.click(repo);

    await waitFor(() => expect(workspaceInFront()).toBe("beta"));
    await explorer().findByRole("treegrid", { name: "Files of beta" });
  });

  it("draws the picked branch's files in Files, and the one repo's own folder before one is picked", async () => {
    core();
    render(<App />);
    await showTheExplorer();
    await reposView().findByTestId("clone-svc");

    const before = explorer().getByRole("treegrid", { name: "Files of alpha" });
    expect(await within(before).findByRole("row", { name: /^README\.md/ })).toBeVisible();

    // Picked in the Repos view, which is what Files lists (P-10).
    await userEvent.click(await reposView().findByRole("row", { name: /^one/ }));

    const after = await explorer().findByRole("treegrid", { name: "Files of one in svc" });
    expect(await within(after).findByRole("row", { name: /^lib\.rs/ })).toBeVisible();
    // A branch is one row of Repos and branches: its files are drawn once, in Files.
    expect(
      within(reposView().getByRole("treegrid", { name: "Repos and branches" })).queryByRole("row", {
        name: /^lib\.rs/,
      }),
    ).not.toBeInTheDocument();
  });

  it("draws the files open though a file from before #3343 says Files was folded", async () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: { version: 2, regions: [], explorer: { closed: ["files"] } },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
    core();
    render(<App />);
    await showTheExplorer();
    await reposView().findByTestId("clone-svc");

    expect(explorer().queryByRole("button", { name: /^Files/ })).toBeNull();
    expect(explorer().getByTestId("files")).toBeVisible();
  });

  it("launches whole on a file from before qw358 that folded Workspaces, and says nothing of it", async () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: {
          version: 2,
          regions: [],
          explorer: { closed: ["workspaces"] },
          projects: { [PLANE]: { explorer: { closed: ["workspaces"] } } },
        },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
    core();
    render(<App />);
    await showTheExplorer();

    const tree = await explorer().findByRole("treegrid", { name: "Files of alpha" });
    expect(tree).toBeVisible();
    expect(explorer().queryByRole("button", { name: "Workspaces" })).toBeNull();
    // Not a mistake in the file: nothing about Explorer's sections is said.
    expect(screen.queryByText(/which Explorer has no section called/)).toBeNull();
  });
  it("draws the Repos view whole, though a file from before #1737 folded Repos and branches", async () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: { version: 2, regions: [], explorer: { closed: ["repos"] } },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
    core();
    render(<App />);
    await screen.findByTestId("clone-svc");

    const tree = reposView().getByRole("treegrid", { name: "Repos and branches" });
    expect(tree).toBeVisible();
    // A view of its own has no fold, and its rows are the keyboard's: one of them is the stop.
    expect(reposView().queryByRole("button", { expanded: false })).toBeNull();
    expect(tree.querySelector('[role="row"][tabindex="0"]')).not.toBeNull();
  });

  it("gives the keyboard to the files' first row when its key shows Explorer", async () => {
    core();
    render(<App />);
    await screen.findByRole("tab", { name: "Explorer" });
    await explorer_ready();

    // jsdom is not a Mac, so the key is Ctrl's (`sideKeys.ts`).
    await userEvent.keyboard("{Control>}{Shift>}E{/Shift}{/Control}");

    await waitFor(() =>
      expect(document.activeElement).toBe(
        within(explorer().getByRole("treegrid", { name: "Files of alpha" })).getByRole("row", {
          name: /^README\.md/,
        }),
      ),
    );
  });
});

describe("Search contents from Explorer's filter (#3343)", () => {
  it("opens Search on the filter's scope with its words by ⌘⇧F in the view, as its link does", async () => {
    core();
    render(<App />);
    await showTheExplorer();
    const tree = await explorer().findByRole("treegrid", { name: "Files of alpha" });
    await within(tree).findByRole("row", { name: /^README\.md/ });
    const box = explorer().getByRole("searchbox", { name: "Filter files" });
    // Given the keyboard rather than clicked: jsdom lays nothing out, so every box is at 0,0 and a
    // click there is heard by the resize handles too, which take the keyboard.
    box.focus();
    await userEvent.keyboard("read");

    // jsdom is not a Mac, so the key is Ctrl's (`searchKey.ts`).
    await userEvent.keyboard("{Control>}{Shift>}F{/Shift}{/Control}");

    const search = await screen.findByRole("tabpanel", { name: "Search" });
    await waitFor(() =>
      expect(within(search).getByRole("searchbox", { name: "Search the files" })).toHaveValue(
        "read",
      ),
    );
    // The one repo's own folder: Search's Branch, as the link under the filter says.
    expect(selectNamed("Where to search", search)).toHaveTextContent("Branch svc");
  });
});

describe("Explorer kept per project across a relaunch (B-11, #1686)", () => {
  const repos = () => within(reposView().getByRole("treegrid", { name: "Repos and branches" }));

  it("writes a clone folded under the project", async () => {
    const asked = core();
    render(<App />);
    await showTheExplorer();
    await explorer_ready();

    await userEvent.click(repos().getByRole("row", { name: /^svc/ }));

    await waitFor(() => expect(explorerOf(asked)).toMatchObject({ folded: ["alpha/svc"] }));
  });

  it("launches with the project's own folds, whatever the machine's", async () => {
    (globalThis as Record<string, unknown>)[GLOBAL] = {
      layout: {
        path: "layout.json",
        found: true,
        document: {
          version: 2,
          regions: [],
          explorer: { closed: ["repos"] },
          projects: { [PLANE]: { explorer: { closed: ["files"], folded: ["alpha/svc"] } } },
        },
        trouble: null,
      },
      theme: { path: "theme.json", found: false, document: null, trouble: null },
    };
    const asked = core();
    render(<App />);
    await showTheExplorer();
    await explorer_ready();

    expect(repos().getByRole("row", { name: /^svc/ })).toHaveAttribute("aria-expanded", "false");
    // *Files* is no fold since #3343, whatever the file says: its tree is drawn.
    expect(explorer().getByTestId("files")).toBeVisible();
    // Showing Explorer writes the arrangement; the project's folds go with it as they were.
    await waitFor(() => expect(written(asked)).toBeDefined());
    expect(explorerOf(asked)).toEqual({ closed: ["files"], folded: ["alpha/svc"] });
  });
});

/** What the last layout written keeps for the project's Explorer. */
const explorerOf = (asked: Asked[]) =>
  (written(asked)?.projects as Record<string, { explorer?: unknown }> | undefined)?.[PLANE]
    ?.explorer;

/** The explorer once the project is read, whether or not it is the view open. */
async function explorer_ready() {
  await screen.findByTestId("clone-svc");
}

/** The last layout the window wrote, as a document. */
function written(asked: Asked[]): Record<string, unknown> | undefined {
  const last = asked.filter((one) => one.cmd === "write_layout").at(-1);
  return last === undefined ? undefined : JSON.parse(String(last.args.text));
}
