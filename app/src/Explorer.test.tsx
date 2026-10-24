// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { Explorer, type Spot } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { catalogue, catalogued, type Catalogued, type Offer } from "./actions";
import { noTabs } from "./tabs";
import type { OpenChat, Panels as PanelsModel, Piece } from "./bindings";
import type { WorkspaceState } from "./workspaceState";
import { drawnWith } from "./cascade.testkit";
import { tipOf } from "./ui/tooltip/tooltip.testkit";

afterEach(cleanup);

const ALPHA = "/home/dev/plane/workspaces/alpha";
const CUT = `${ALPHA}/.worktrees/svc`;

const PANELS: PanelsModel = {
  workspace: "alpha",
  repos: ["svc", "tool"],
  paths: {},
  absent: [],
  refused: [],
  todos: [],
  todos_refused: null,
  personas: ["steward"],
  persona: "steward",
  sessions: [],
  contributed: [],
};

function piece(name: string, on: Partial<Piece> = {}): Piece {
  return {
    piece: name,
    path: `${CUT}/${name}`,
    branch: name,
    wired: true,
    stale: false,
    said: "",
    ...on,
  };
}

function chat(session: number, name: string, cwd: string, on: Partial<OpenChat> = {}): OpenChat {
  return {
    session,
    name,
    cwd,
    harness: "claude",
    in_front: false,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
    ...on,
  };
}

function state(on: Partial<WorkspaceState> = {}): WorkspaceState {
  return {
    panels: PANELS,
    repos: { workspace: "alpha", repos: [], cache_refused: null },
    pieces: { svc: [piece("one"), piece("two")], tool: [] },
    piecesRefused: {},
    reading: false,
    ...on,
  };
}

function draw(on: {
  state?: WorkspaceState;
  chats?: OpenChat[];
  spot?: Spot;
  onPick?: (spot: Spot | undefined) => void;
  workspace?: string;
  /** The catalogue a piece row's menu is drawn out of. Empty here for every test but the one
   *  about the menu: `Menued` draws nothing for an item the catalogue has no rows for, so the
   *  tree these tests are about is the tree they were always about. */
  offers?: Catalogued;
  onPress?: (offer: Offer) => void;
  states?: ChatStates;
}) {
  render(
    // What the chats are doing is read by each row off the project's store (SC-3), which is
    // the one `PlaneView` provides; here it is one that holds `states` and never moves.
    <ChatsHere.Provider value={fixedChats(on.states ?? nothingKnown)}>
      <Explorer
        workspace={"workspace" in on ? on.workspace : "alpha"}
        state={on.state ?? state()}
        chats={on.chats ?? []}
        spot={on.spot}
        onPick={on.onPick ?? (() => {})}
        offers={on.offers ?? new Map()}
        onPress={on.onPress ?? (() => {})}
        onReadAgain={() => {}}
      />
    </ChatsHere.Provider>,
  );
}

/** The spot marked as the one the next chat would start in. */
const picked = () =>
  screen
    .getAllByRole("treeitem")
    .filter((one) => one.getAttribute("aria-current") === "true")
    .map((one) => one.textContent);

describe("the explorer", () => {
  it("lists the focused workspace's clones and the worktrees cut off each", () => {
    draw({});

    const svc = screen.getByTestId("clone-svc");
    expect(within(svc).getByRole("treeitem", { name: /one/ })).toBeInTheDocument();
    expect(within(svc).getByRole("treeitem", { name: /two/ })).toBeInTheDocument();
    expect(screen.getByTestId("clone-tool")).toHaveTextContent("No branches cut here");
  });

  it("lists no workspaces: the strip is where one is selected (qw358)", () => {
    // The operator's words: "workspaces selector is very redundant thing, as workspaces we have
    // in top bar separate tabs". ADR 0036 made the strip the axis; #1677 drew its list a second
    // time here, and that section is gone.
    draw({});

    const explorer = screen.getByTestId("explorer");
    expect(screen.queryByRole("tree", { name: "Workspaces of this project" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Workspaces" })).toBeNull();
    expect(explorer.querySelector('[data-section="workspaces"]')).toBeNull();
    expect(explorer.querySelector('[data-row^="workspace:"]')).toBeNull();
    expect(
      [...explorer.querySelectorAll("[data-section]")].map((one) =>
        one.getAttribute("data-section"),
      ),
    ).toEqual(["repos", "files"]);
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  });

  it("is the Files section alone in the window's Explorer view, named for the workspace", () => {
    render(
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <Explorer
          part="explorer"
          workspace="alpha"
          state={state()}
          chats={[]}
          spot={undefined}
          onPick={() => {}}
          offers={new Map()}
          onPress={() => {}}
          onReadAgain={() => {}}
        />
      </ChatsHere.Provider>,
    );

    const explorer = screen.getByTestId("explorer");
    expect(
      [...explorer.querySelectorAll("[data-section]")].map((one) =>
        one.getAttribute("data-section"),
      ),
    ).toEqual(["files"]);
    // The heading keeps naming whose files they are.
    expect(screen.getByRole("button", { name: "Files alpha" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("treegrid", { name: "Files of alpha" })).toBeInTheDocument();
  });

  it("follows the strip: the files are of whichever workspace it selects", () => {
    const of = (workspace: string | undefined, repos: string[]) => (
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <Explorer
          part="explorer"
          workspace={workspace}
          state={state({ panels: { ...PANELS, workspace: workspace ?? "", repos }, pieces: {} })}
          chats={[]}
          spot={undefined}
          onPick={() => {}}
          offers={new Map()}
          onPress={() => {}}
          onReadAgain={() => {}}
        />
      </ChatsHere.Provider>
    );
    const { rerender } = render(of("alpha", ["svc", "tool"]));
    const names = () =>
      within(screen.getByTestId("files"))
        .getAllByRole("row")
        .map((one) => one.textContent?.trim());
    expect(screen.getByRole("treegrid", { name: "Files of alpha" })).toBeInTheDocument();
    expect(names()).toEqual(["svc", "tool"]);

    // The strip moved to beta: the same view, beta's files, and nothing of alpha's.
    rerender(of("beta", ["web"]));

    expect(screen.getByRole("button", { name: "Files beta" })).toBeInTheDocument();
    expect(screen.getByRole("treegrid", { name: "Files of beta" })).toBeInTheDocument();
    expect(screen.queryByRole("treegrid", { name: "Files of alpha" })).toBeNull();
    expect(names()).toEqual(["web"]);
  });

  it("draws no chat, wherever it works: the Chats view lists them (#1673)", () => {
    draw({
      chats: [
        chat(1, "ide.1", `${CUT}/one`),
        chat(2, "root.2", ALPHA),
        chat(3, "tool.3", `${ALPHA}/tool`, { from: { chat: 1, task: true } as OpenChat["from"] }),
      ],
    });

    const explorer = screen.getByRole("navigation", { name: "Explorer" });
    for (const name of ["ide.1", "root.2", "tool.3"])
      expect(within(explorer).queryByText(name, { exact: false })).toBeNull();
    expect(within(explorer).queryByText(/tasks?\b/)).toBeNull();
    // The tree is the place: the workspace, its clones, their branches and files.
    expect(
      within(explorer)
        .getAllByRole("treeitem")
        .map((row) => row.dataset.row?.split(":")[0]),
    ).not.toContain("chat");
  });

  it("starts on the workspace's own directory, so a plain New tab opens where it always did", () => {
    draw({});

    expect(picked()).toEqual([expect.stringContaining("alpha")]);
  });

  it("hands back the path the core spelled when a worktree is picked", async () => {
    // The window never joins a path together. `worktree_list` answers with one, and that is
    // what the next chat is started in.
    const onPick = vi.fn();
    draw({ onPick });

    await userEvent.click(screen.getByRole("treeitem", { name: /^one/ }));

    expect(onPick).toHaveBeenCalledWith({ repo: "svc", piece: "one", path: `${CUT}/one` });
  });

  it("marks the picked worktree, and only it", () => {
    draw({ spot: { repo: "svc", piece: "two", path: `${CUT}/two` } });

    expect(picked()).toEqual([expect.stringContaining("two")]);
  });

  it("goes back to the workspace when its own row is picked", async () => {
    const onPick = vi.fn();
    draw({ spot: { repo: "svc", piece: "two", path: `${CUT}/two` }, onPick });

    await userEvent.click(screen.getByRole("treeitem", { name: /the workspace itself/ }));

    expect(onPick).toHaveBeenCalledWith(undefined);
  });

  it("says a worktree has no purlis layer before a chat is started in it", () => {
    // `unwired` is what the operator has to see BEFORE they click: a chat started in such a
    // tree runs with none of the plane's ask/deny rules and no persona agents.
    draw({ state: state({ pieces: { svc: [piece("one", { wired: false })], tool: [] } }) });

    expect(screen.getByTestId("piece-svc-one")).toHaveTextContent("unwired");
  });

  it("says what each piece declared, or how long it has been silent", () => {
    // charter#368: a finished piece and a quiet one must not look alike on the row.
    draw({
      state: state({
        pieces: {
          svc: [
            piece("one", { said: "done" }),
            piece("two", { said: "abandoned: wrong approach" }),
            piece("three", { said: "silent 3d" }),
            piece("four"),
          ],
          tool: [],
        },
      }),
    });

    expect(screen.getByTestId("piece-svc-one")).toHaveTextContent("done");
    expect(screen.getByTestId("piece-svc-two")).toHaveTextContent("abandoned: wrong approach");
    expect(screen.getByTestId("piece-svc-three")).toHaveTextContent("silent 3d");
    expect(screen.getByTestId("piece-svc-four").querySelector("[data-testid=piece-said]")).toBe(
      null,
    );
  });

  it("says a branch cut for a chat that never started is unclaimed, and for how long", () => {
    // #835: shown, never swept. Removing it stays the row's own action.
    draw({
      state: state({
        pieces: { svc: [piece("one", { unclaimed: "3d" }), piece("two")], tool: [] },
      }),
    });

    const mark = screen.getByTestId("piece-svc-one").querySelector("[data-testid=piece-unclaimed]");
    expect(mark).toHaveTextContent("unclaimed 3d");
    expect(tipOf(mark)).toEqual(expect.stringContaining("never started"));
    expect(screen.getByTestId("piece-svc-two").querySelector("[data-testid=piece-unclaimed]")).toBe(
      null,
    );
  });

  it("says a branch folder purlis could not read is unreadable, and why (#1476)", () => {
    const why = "this folder's git link was changed, so purlis is not reading it";
    draw({
      state: state({
        pieces: { svc: [piece("one", { uncommitted: null, unread: why }), piece("two")], tool: [] },
      }),
    });

    const mark = screen.getByTestId("piece-svc-one").querySelector("[data-testid=piece-unread]");
    expect(mark).toHaveTextContent("unreadable");
    expect(tipOf(mark)).toEqual(why);
    expect(screen.getByTestId("piece-svc-two").querySelector("[data-testid=piece-unread]")).toBe(
      null,
    );
  });

  it("says a registration whose directory is gone is stale", () => {
    draw({ state: state({ pieces: { svc: [piece("one", { stale: true })], tool: [] } }) });

    expect(screen.getByTestId("piece-svc-one")).toHaveTextContent("stale");
  });

  it("names the branch each worktree is on", () => {
    draw({ state: state({ pieces: { svc: [piece("one", { branch: "fix/login" })], tool: [] } }) });

    expect(screen.getByTestId("piece-svc-one")).toHaveTextContent("fix/login");
  });

  it("reads a branch's row as its branch in its repo, with its folder kept in the tooltip (#1102)", () => {
    // ADR 0072 §4: the row is *`fix/login` in svc*. The folder's name is where git keeps the
    // branch, and is shown only where a path is wanted.
    draw({ state: state({ pieces: { svc: [piece("one", { branch: "fix/login" })], tool: [] } }) });

    const row = within(screen.getByTestId("piece-svc-one")).getByRole("treeitem", {
      name: "fix/login in svc",
    });
    expect(tipOf(row)).toEqual(`${CUT}/one`);
    // Said once: the mark beside the row no longer repeats the branch.
    expect(within(screen.getByTestId("piece-svc-one")).getAllByText("fix/login")).toHaveLength(1);
  });

  it("reads a folder with no branch checked out by the folder's name (#1102)", () => {
    draw({ state: state({ pieces: { svc: [piece("one", { branch: null })], tool: [] } }) });

    expect(
      within(screen.getByTestId("piece-svc-one")).getByRole("treeitem", { name: "one" }),
    ).toBeInTheDocument();
  });

  it("says why a clone's worktrees could not be listed rather than showing none", () => {
    draw({
      state: state({
        pieces: { tool: [] },
        piecesRefused: { svc: "purlis will not run git through a symlink" },
      }),
    });

    const svc = screen.getByTestId("clone-svc");
    const refused = within(svc).getByRole("status");
    expect(refused).toHaveTextContent("symlink");
    // Its way out (NO-4): the read asked again.
    expect(within(refused).getByRole("button", { name: "Read again" })).toBeInTheDocument();
    expect(svc).not.toHaveTextContent("No branches cut here");
  });

  it("names why on the clone's heading, where its branches could not be read (#1476)", () => {
    draw({
      state: state({
        pieces: { tool: [] },
        piecesRefused: { svc: "purlis will not run git through a symlink" },
      }),
    });

    const count = within(screen.getByTestId("clone-svc")).getByText("unreadable");
    expect(tipOf(count)).toEqual("purlis will not run git through a symlink");
  });

  it("says the listing is still coming rather than saying there is nothing", () => {
    draw({ state: state({ pieces: {} }) });

    expect(screen.getByTestId("clone-svc")).toHaveTextContent("Asking git…");
  });

  // ---------------------------------------------------------------------------------------
  // What is already running where
  // ---------------------------------------------------------------------------------------

  // ---------------------------------------------------------------------------------------
  // Nothing to explore
  // ---------------------------------------------------------------------------------------

  it("names the plane root in the Files heading when the strip is on it, and reads no folder without a project (#1916)", () => {
    draw({ workspace: undefined });

    const explorer = screen.getByTestId("explorer");
    // Still a files browser with its heading, never a list of workspaces to pick from, and
    // no sentence in place of the tree: with a project, the tree is the whole project's
    // (`ExplorerSections.window.test.tsx`).
    expect(screen.getByRole("button", { name: "Files Plane root" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(explorer).not.toHaveTextContent(/is not a workspace/);
    expect(screen.queryByRole("tree")).toBeNull();
    expect(explorer.querySelector('[data-section="workspaces"]')).toBeNull();
  });

  it("says the Repos view has no repos to list on the plane root", () => {
    render(
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <Explorer
          part="repos"
          workspace={undefined}
          state={state()}
          chats={[]}
          spot={undefined}
          onPick={() => {}}
          offers={new Map()}
          onPress={() => {}}
          onReadAgain={() => {}}
        />
      </ChatsHere.Provider>,
    );

    expect(screen.getByTestId("repos-view")).toHaveTextContent("there are no repos to list");
  });

  it("says a workspace holds no repos rather than drawing an empty region", () => {
    draw({ state: state({ panels: { ...PANELS, repos: [] }, pieces: {} }) });

    expect(screen.getByTestId("explorer")).toHaveTextContent("No repos in this workspace");
  });

  it("names a repo the manifest claims that nobody has cloned", () => {
    draw({ state: state({ panels: { ...PANELS, repos: ["svc"], absent: ["later"] } }) });

    expect(screen.getByTestId("absent")).toHaveTextContent("later");
    // And it is not a heading with worktrees under it: there is nothing to cut one from.
    expect(screen.queryByTestId("clone-later")).not.toBeInTheDocument();
  });

  it("shows what purlis would not read instead of a workspace with fewer repos in it", () => {
    draw({
      state: state({
        panels: {
          ...PANELS,
          repos: ["svc"],
          refused: [["alias", "'alias' is reached through a symlink"]],
        },
      }),
    });

    const refused = screen
      .getByText(/'alias' is reached through a symlink/)
      .closest("[data-cause]") as HTMLElement;
    expect(refused.getAttribute("data-cause")).toBe("repo-refused:alpha/alias");
    expect(within(refused).getByRole("button", { name: "Read again" })).toBeInTheDocument();
  });
});

/**
 * Right-click on a piece row (charter-app#174).
 *
 * **Asked in jsdom, because a WebDriver right-click sends no `contextmenu` event at all** —
 * measured on webkit 605.1.15 and on WebKitGTK, and recorded in `docs/ui-primitives.md`. A
 * scenario spec can prove a menu is on screen; whether a right-click opens one is a question
 * only this environment can answer.
 *
 * What is asserted here is the wiring and nothing else: the row has a menu, and the menu's
 * rows are the catalogue's. What those rows SAY is `actions.test.ts`'s, and saying it again
 * here would be the second answer the whole design exists to prevent.
 */
describe("a piece row's menu", () => {
  const cut = { workspace: "alpha", repo: "svc", piece: "one", branch: "one" };
  const offers = () =>
    catalogued(
      catalogue({
        tabs: noTabs(),
        workspaces: ["alpha"],
        focused: "alpha",
        plane: "/plane",
        pieces: [cut],
        needsYou: [],
        nameOf: String,
      }),
    );

  it("opens on a right-click with the catalogue's own rows for that piece", async () => {
    draw({ offers: offers() });

    const row = within(screen.getByTestId("piece-svc-one")).getByRole("treeitem", {
      name: "one in svc",
    });
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));

    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual([
      "Focus",
      "Browse files",
      // The branch's own folder (#1143).
      "Copy absolute path",
      expect.stringMatching(/^Reveal in /),
      "Open a shell tab here",
      // The repo it merges into is another thing than the branch, so the row names it.
      "Merge into svc",
      "Remove folder",
    ]);
  });

  it("hands the catalogue's offer back when a row is pressed", async () => {
    const pressed: string[] = [];
    draw({ offers: offers(), onPress: (offer) => pressed.push(offer.id) });

    const row = within(screen.getByTestId("piece-svc-one")).getByRole("treeitem", {
      name: "one in svc",
    });
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    await screen.findByRole("menu");
    await userEvent.click(screen.getByRole("menuitem", { name: "Remove folder" }));

    expect(pressed).toEqual(["worktree.remove:svc/one"]);
  });
});

/**
 * Right-click on a clone's heading (charter-app#174, the second half).
 *
 * It had no menu because nothing in the catalogue was about a clone, and nothing could be until
 * the core said where one is. `Panels.paths` says so now, and the catalogue has two rows per
 * clone: a new tab in it, and picking it as where new chats start.
 */
describe("a clone row's menu", () => {
  const SVC = `${ALPHA}/svc`;
  const offers = (startsIn?: string) =>
    catalogued(
      catalogue({
        tabs: noTabs(),
        workspaces: ["alpha"],
        focused: "alpha",
        plane: "/plane",
        clones: [{ repo: "svc", path: SVC }],
        startsIn,
        needsYou: [],
        nameOf: String,
      }),
    );
  const heading = () =>
    within(screen.getByTestId("clone-svc")).getByRole("treeitem", { name: /^svc/ });

  it("opens on a right-click with the catalogue's own rows for that clone", async () => {
    draw({ offers: offers() });

    heading().dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));

    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Focus", "New tab", "New branch…", "Start new chats here"]);
  });

  it("hands the catalogue's offer back when a row is pressed", async () => {
    const pressed: string[] = [];
    draw({ offers: offers(), onPress: (offer) => pressed.push(offer.id) });

    heading().dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    await screen.findByRole("menu");
    await userEvent.click(screen.getByRole("menuitem", { name: "Start new chats here" }));

    expect(pressed).toEqual(["clone.pick:svc"]);
  });

  it("opens from the keyboard on the clone's row, which the arrows reach", async () => {
    const pressed: string[] = [];
    draw({ offers: offers(), onPress: (offer) => pressed.push(offer.id) });
    screen.getByRole("treeitem", { name: /^alpha/ }).focus();
    await userEvent.keyboard("{ArrowDown}");
    expect(heading()).toHaveFocus();

    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await screen.findByRole("menu");
    await userEvent.keyboard("{Enter}");

    // The first row, its own folder's cockpit (#1152).
    expect(pressed).toEqual(["clone.focus:svc"]);
  });

  it("marks a picked clone as where the next chat starts, and only it", () => {
    draw({ offers: offers(SVC), spot: { repo: "svc", path: SVC } });

    expect(picked()).toEqual([expect.stringMatching(/^svc/)]);
  });
});

// ---------------------------------------------------------------------------------------
// Drawn as a tree (M6.6)
// ---------------------------------------------------------------------------------------

describe("the explorer is drawn as a tree", () => {
  it("marks each kind of node with its own icon", () => {
    draw({});

    const svc = screen.getByTestId("clone-svc");
    // Lucide names each `<svg>` after its icon, which is the only handle a test has on which
    // mark was drawn — the mark itself is hidden from the accessibility tree on purpose.
    expect(svc.querySelector("summary .lucide-folder-git-2")).not.toBeNull();
    expect(svc.querySelector("summary .twisty")).not.toBeNull();
    const one = screen.getByTestId("piece-svc-one");
    expect(one.querySelector(".spot .lucide-git-branch")).not.toBeNull();
  });

  it("keeps every row named by its words, with the icons beside them unread", () => {
    draw({ chats: [chat(1, "ide.1", `${CUT}/one`)] });

    // The names the rest of this file and the scenario specs press by. An icon that joined
    // an accessible name would rename every row it was put on.
    expect(screen.getByRole("treeitem", { name: /^one in svc$/ })).toBeInTheDocument();
    for (const svg of screen.getByTestId("explorer").querySelectorAll("svg")) {
      expect(svg.getAttribute("aria-hidden")).toBe("true");
    }
  });

  it("spins only while something is still being read", () => {
    draw({ state: state({ pieces: {} }) });
    expect(screen.getByTestId("clone-svc").querySelector(".pending .spinning")).not.toBeNull();

    cleanup();
    draw({});
    expect(screen.getByTestId("explorer").querySelector(".spinning")).toBeNull();
  });
});

/**
 * **The tree's guides may not paint a surface**, because a region moves (ADR 0038).
 *
 * The usual way to stop a tree's vertical line at its last row is a rectangle in the background
 * colour laid over the line's tail. That works exactly as long as the tree is on that
 * background — and the explorer can be moved to a slot on another surface, where the mask would
 * be a visible block. The shared tree row (`primitives/collections.css`, #1672, DS-12) draws
 * straight guides that need no mask: a background of the guide's colour and transparent, one
 * column per level above the row. This holds the rule a well-meaning simplification would break.
 */
describe("the explorer's tree guides", () => {
  const strip = (file: string) =>
    readFileSync(join(process.cwd(), "src", file), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const shared = strip("primitives/collections.css");
  const guides = [...shared.matchAll(/([^{}]*)\{([^}]*)\}/g)].filter(([, , body]) =>
    body.includes("var(--tree-guide)"),
  );

  it("are drawn by the shared tree row", () => {
    expect(guides.map(([, selector]) => selector.trim())).toEqual([".ui-tree-row"]);
  });

  it("paint the guide's colour and nothing else, so they need to know nothing about what is behind them", () => {
    const [[, , body]] = guides;
    const image = /background-image:([^;]*);/.exec(body)?.[1] ?? "";
    const colours = [...image.matchAll(/var\((--[\w-]+)\)/g)]
      .map((hit) => hit[1])
      .filter((read) => !read.startsWith("--ui-tree"));
    expect(colours).toEqual(["--tree-guide"]);
    expect(image).toContain("transparent");
    expect(body).not.toMatch(/background-color\s*:/);
  });

  it("are no longer drawn as elbows by the rows", () => {
    const css = strip("App.css");
    expect([...css.matchAll(/\.explorer[^{}]*::(?:before|after)[^{}]*\{[^}]*border/g)]).toEqual([]);
  });
});

/**
 * **The explorer's rows are the shared tree row** (DS-12): one height, a step per level, the
 * shared selected look and the shared hover, from the tokens, wherever the row is a button or a
 * clone's `<summary>`; and its toggles and filter are the shared button and field.
 */
describe("the explorer on the shared styles", () => {
  it("draws every row as a shared tree row, a step in per level", () => {
    draw({});
    const tree = screen.getByRole("tree", { name: "Repos and branches" });
    expect(tree).toHaveClass("ui-tree");
    const rows = within(tree).getAllByRole("treeitem");
    expect(rows.length).toBeGreaterThan(3);
    for (const row of rows) {
      expect(row).toHaveClass("ui-row", "ui-tree-row");
      expect(drawnWith(row, "min-block-size")).toBe("var(--ui-row-height)");
      expect(drawnWith(row, "padding-inline-start")).toContain("var(--ui-tree-step)");
      // Under the pointer: the hover, but the selected row stays selected.
      expect(drawnWith(row, "background", { hover: true })).toBe(
        row.getAttribute("aria-current") === "true"
          ? "var(--list-selected)"
          : "var(--surface-hover)",
      );
    }
    // The picked spot is the selected row, its look the shared one.
    const current = rows.find((row) => row.getAttribute("aria-current") === "true");
    expect(current && drawnWith(current, "background")).toBe("var(--list-selected)");
    // The clone's twist hangs in its step, turned by the row's own `aria-expanded`.
    const twist = screen.getByTestId("clone-svc").querySelector("summary > .ui-tree-twist");
    expect(twist?.closest("summary")).toHaveAttribute("aria-expanded", "true");
    // The twist is a box holding the chevron (`docs/ui-primitives.md`), never the icon itself:
    // an icon given the class kept its own height and sat at the row's top (qw358).
    if (twist === null) throw new Error("the clone's row has no twist");
    expect(twist.tagName).toBe("SPAN");
    expect(twist.querySelector(":scope > svg")).not.toBeNull();
    expect(drawnWith(twist, "block-size")).toBe("var(--ui-row-height)");
    expect(drawnWith(twist, "margin-block")).toBe("auto");
  });

  it("stands what a clone holds where its branches' names start", () => {
    draw({ state: state({ pieces: {} }) });
    const pending = screen.getByTestId("clone-svc").querySelector(".pending");
    if (pending === null) throw new Error("the clone says nothing while git is asked");
    // One level under the clone's row: the branches' level, said on what holds them.
    expect(pending.closest("[data-level]")).toHaveAttribute("data-level", "3");
    expect(drawnWith(pending, "padding-inline-start")).toContain("var(--ui-tree-depth)");
  });
});

// ---------------------------------------------------------------------------------------
// A WAI-ARIA tree (charter-app#238)
// ---------------------------------------------------------------------------------------

/**
 * **The explorer is a tree for the keyboard and for a screen reader**, the WAI-ARIA "Tree
 * View" pattern: `role="tree"` and `treeitem`, each row's level and place among its siblings,
 * `aria-expanded` on the rows that fold, and the keys that open, close and climb.
 *
 * The tree drawn here is `alpha` → `svc` (→ `one`, `two`) and `tool`, which has no branches.
 * Their files are the *Files* section's own tree (#1677).
 */
describe("the explorer is a WAI-ARIA tree", () => {
  // A chat working in a branch, which the tree no longer draws (#1673): its shape is the same
  // with or without it.
  const withAChat = () => draw({ chats: [chat(7, "seven", `${CUT}/one`)] });
  const tree = () => screen.getByRole("tree", { name: "Repos and branches" });
  const item = (name: RegExp) => within(tree()).getByRole("treeitem", { name });
  /** A treeitem as its first word, its level and its place among its siblings. */
  const shape = (row: HTMLElement) =>
    [
      row.querySelector(".spot-name, .repo, .session")?.textContent,
      row.getAttribute("aria-level"),
      `${row.getAttribute("aria-posinset")}/${row.getAttribute("aria-setsize")}`,
    ].join(" ");

  it("draws every row as a treeitem with its level and its place among its siblings", () => {
    withAChat();

    expect(within(tree()).getAllByRole("treeitem").map(shape)).toEqual([
      "alpha 1 1/1",
      "svc 2 1/2",
      "one 3 1/2",
      "two 3 2/2",
      "tool 2 2/2",
    ]);
  });

  it("says on every parent whether it is open, and nothing on a leaf", async () => {
    withAChat();

    // A clone folds, even one with no worktrees: what it opens onto is the note saying so.
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "true");
    expect(item(/^tool/)).toHaveAttribute("aria-expanded", "true");
    // Parents that cannot fold are always open, and say so.
    expect(item(/^alpha/)).toHaveAttribute("aria-expanded", "true");
    // A branch is a leaf here: its files are the Files section's (#1677).
    expect(item(/^one/)).not.toHaveAttribute("aria-expanded");
    expect(item(/^two/)).not.toHaveAttribute("aria-expanded");

    await userEvent.click(item(/^svc/));
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "false");
  });

  it("Right opens a closed clone, then moves to its first child, and stops at a leaf", async () => {
    withAChat();
    await userEvent.click(item(/^svc/));
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "false");

    await userEvent.keyboard("{ArrowRight}");
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "true");
    expect(item(/^svc/)).toHaveFocus();

    // Its first child is its first branch, a leaf, where Right stays.
    await userEvent.keyboard("{ArrowRight}");
    expect(item(/^one/)).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    expect(item(/^one/)).toHaveFocus();
  });

  it("Right on the workspace row moves to its first child", async () => {
    withAChat();
    item(/^alpha/).focus();

    await userEvent.keyboard("{ArrowRight}");
    expect(item(/^svc/)).toHaveFocus();
  });

  it("Left moves to the parent, closes an open clone, and stops at the workspace row", async () => {
    withAChat();
    item(/^one/).focus();

    await userEvent.keyboard("{ArrowLeft}");
    expect(item(/^svc/)).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "false");
    expect(item(/^svc/)).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(item(/^alpha/)).toHaveFocus();
    await userEvent.keyboard("{ArrowLeft}");
    expect(item(/^alpha/)).toHaveFocus();
  });

  it("Up and Down skip what a folded clone holds, and Home and End reach the ends", async () => {
    withAChat();
    item(/^svc/).focus();
    await userEvent.keyboard("{ArrowLeft}");

    await userEvent.keyboard("{ArrowDown}");
    expect(item(/^tool/)).toHaveFocus();
    await userEvent.keyboard("{Home}");
    expect(item(/^alpha/)).toHaveFocus();
    await userEvent.keyboard("{End}");
    expect(item(/^tool/)).toHaveFocus();
  });

  it("a typed letter moves to the next row whose name starts with it, and wraps", async () => {
    withAChat();
    item(/^alpha/).focus();

    await userEvent.keyboard("t");
    expect(item(/^two/)).toHaveFocus();
    await userEvent.keyboard("T");
    expect(item(/^tool/)).toHaveFocus();
    await userEvent.keyboard("t");
    expect(item(/^two/)).toHaveFocus();
    // A letter nothing starts with moves nothing.
    await userEvent.keyboard("z");
    expect(item(/^two/)).toHaveFocus();
  });

  it("a typed letter finds a branch's row by the branch it reads, not its folder (#1102)", async () => {
    draw({ state: state({ pieces: { svc: [piece("one", { branch: "work/login" })], tool: [] } }) });
    item(/^alpha/).focus();

    await userEvent.keyboard("w");
    expect(item(/^work\/login in svc/)).toHaveFocus();
    // The folder's name is not what the row reads, so its letter finds nothing.
    await userEvent.keyboard("o");
    expect(item(/^work\/login in svc/)).toHaveFocus();
  });

  it("Enter does what a click does: a worktree is picked", async () => {
    const onPick = vi.fn();
    draw({ chats: [chat(7, "seven", `${CUT}/one`)], onPick });

    item(/^one/).focus();
    await userEvent.keyboard("{Enter}");
    expect(onPick).toHaveBeenCalledWith({ repo: "svc", piece: "one", path: `${CUT}/one` });
  });

  it("leaves a key with a modifier alone", async () => {
    withAChat();
    item(/^svc/).focus();

    await userEvent.keyboard("{Meta>}{ArrowLeft}{/Meta}");
    expect(item(/^svc/)).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{Control>}t{/Control}");
    expect(item(/^svc/)).toHaveFocus();
  });
});
