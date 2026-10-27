// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer, type ExplorerPart, type Spot } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FolderEntry, Panels as PanelsModel, Piece, PlaneId } from "./bindings";
import { declarations } from "./guards.testkit";
import { forgetProjectThemes } from "./projectTheme";
import { forgetLastRead } from "./editor/lastRead";
import { drawnWith } from "./cascade.testkit";
import { drawnAs, inherited, wears } from "./textRoles.testkit";
import type { WorkspaceState } from "./workspaceState";

/**
 * **Every word of the Explorer and of the Repos view wears a text role** (#1920, the second
 * surface to move; `docs/design-system.md` "Text roles"). The Explorer set one size on its root
 * (12px) and ranked its words by opacity and by three greys; each is now one of the roles, and
 * this file holds each to its role's declarations, resolved by the window's own cascade.
 *
 * **A row's name takes the body from the Explorer's root, not from a class of its own**: a
 * changed file's name is tinted with its change's colour and a refused file's is quieter, and
 * a role outranks a view's rule for the word that wears it.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetProjectThemes();
  forgetLastRead();
});

const PLANE = "/plane" as unknown as PlaneId;

const PANELS: PanelsModel = {
  workspace: "alpha",
  repos: ["svc", "web"],
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
  wired: false,
  stale: false,
  said: "",
};

const STATE: WorkspaceState = {
  panels: PANELS,
  repos: { workspace: "alpha", repos: [], cache_refused: null },
  pieces: { svc: [ONE], web: [] },
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

function core(folders: Record<string, FolderEntry[]>) {
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, string | null>;
      if (cmd === "branch_tree")
        return { entries: folders[`${a.piece ?? ""}:${a.folder}`] ?? [], more: 0 };
      if (cmd === "files_watch") return null;
      if (cmd === "project_icons_drawn") return null;
      if (cmd === "extension_icon_themes") return [];
      throw new Error(`unexpected ${cmd}`);
    },
    { shouldMockEvents: true },
  );
}

const PICKED: Spot = { repo: "svc", piece: "one", path: ONE.path };

function draw(on: { workspace?: string; part?: ExplorerPart; spot?: Spot } = {}) {
  return render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        plane={PLANE}
        workspace={"workspace" in on ? on.workspace : "alpha"}
        state={STATE}
        chats={[]}
        part={on.part}
        spot={on.spot}
        onPick={() => {}}
        offers={new Map()}
        onPress={() => {}}
        onReadAgain={() => {}}
        onOpenFile={() => {}}
      />
    </ChatsHere.Provider>,
  ).container;
}

describe("the Explorer's words, each on its text role", () => {
  it("titles the files with the view's title, and what it is of beside it as data (#3343)", async () => {
    core({ "one:": [entry("src", { kind: "folder" }), entry("README.md")] });
    const drawn = draw({ spot: PICKED });
    await screen.findByRole("row", { name: /README\.md/ });

    // *Files* is no fold: its view's one title, the body's strong words (#3340), the workspace.
    const title = drawn.querySelector('[data-section="files"] [data-part="title"]');
    wears(title, "body", "the files' title", true);
    expect(title).toHaveTextContent("alpha");
    const beside = drawn.querySelector('[data-section="files"] .head-beside');
    wears(beside, "meta", "what the files are of");
  });

  it("draws a file's and a folder's name at the body's size, in a tree row's tier", async () => {
    core({ "one:": [entry("src", { kind: "folder" }), entry("README.md")] });
    const drawn = draw({ spot: PICKED });
    const file = await screen.findByRole("row", { name: /README\.md/ });
    const folder = await screen.findByRole("row", { name: /^src/ });

    expect(drawn.querySelector(".explorer")).toHaveClass("ui-text-body");
    // The words are the row's: the body's size and weight from the Explorer's root, in the
    // secondary tier at rest and the primary once chosen, as every row of a tree (#2911, R19).
    for (const row of [file, folder]) {
      expect(row.querySelector(".spot-name")).not.toBeNull();
      expect(drawnWith(row, "color")).toBe("var(--text-secondary)");
      expect(drawnWith(row, "font-size")).toBeUndefined();
    }
  });

  it("says an empty folder as a line of the tree in the quiet tier, not a dimmed body", async () => {
    core({ "one:": [] });
    const drawn = draw({ spot: PICKED });
    const none = (await within(drawn).findByText("Nothing here")).closest<HTMLElement>(
      '[data-part="root"]',
    );
    // Informational words, so the quiet tier (#2911): a line of the tree family.
    expect(none).toHaveAttribute("data-line");
    expect(drawnWith(none as HTMLElement, "color")).toBe("var(--text-quiet)");
    expect(drawnWith(none as HTMLElement, "opacity")).toBeUndefined();
  });

  it("draws the Repos view's rows on the roles: names, where, counts and marks", () => {
    core({});
    const drawn = draw({ part: "repos" });
    const tree = within(drawn).getByRole("treegrid", { name: "Repos and branches" });

    wears(drawn.querySelector(".explorer-section-head"), "label", "the Repos title");
    const root = tree.querySelector(".spot-root");
    // The workspace's row is the current one here: its name at the body's size, in the primary
    // tier a chosen row's words take, at no weight of its own (#2911).
    drawnAs(root?.querySelector(".spot-name"), "body", "the workspace's name");
    wears(root?.querySelector(".spot-what"), "secondary", "the workspace itself");

    const clone = within(drawn).getByTestId("clone-svc");
    wears(clone.querySelector(".repo"), "body", "a repo's name", true);
    // A count is the count family's pill (#2912), its number at the strong weight (T5).
    expect(clone.querySelector(".piece-count")).toHaveAttribute("data-scope", "count");
    // A branch's row: its name at the body's size in a tree row's tier at rest (#2911, R19).
    const branch = tree.querySelector<HTMLElement>('[data-clone="svc"].spot');
    if (branch === null) throw new Error("no branch row drawn under svc");
    expect(branch.querySelector(".spot-name")).not.toBeNull();
    expect(drawnWith(branch, "color")).toBe("var(--text-secondary)");
    wears(branch.querySelector(".spot-in"), "secondary", "the repo a branch is in");
    wears(branch.querySelector(".worktree"), "meta", "a branch's marks");

    // What a clone holds when it holds nothing: a line of the tree, in the quiet tier.
    const none = within(drawn)
      .getByText("No branches cut here")
      .closest<HTMLElement>("[data-line]");
    expect(none && drawnWith(none, "color")).toBe("var(--text-quiet)");
  });

  it("says there is nothing to list as a secondary sentence, at its full strength", () => {
    core({});
    const drawn = draw({ workspace: undefined, part: "repos" });
    const empty = drawn.querySelector(".empty");
    wears(empty, "secondary", "the empty sentence");
    // The role's colour is what makes it quiet: `.empty`'s own dimming is not drawn over it.
    expect(drawnWith(empty as Element, "opacity")).toBe("1");
    expect(inherited(empty as Element, "opacity")).toBe("1");
  });

  it("writes no size, weight or colour of words of its own, beside the tints a state gives", () => {
    const typed = declarations(readFileSync(join(process.cwd(), "src", "App.css"), "utf8"))
      .filter(({ context }) => /^\.explorer\b/.test(context[context.length - 1] ?? ""))
      .filter(({ property: name }) => ["font-size", "font-weight", "line-height"].includes(name))
      .map(({ context, property: name, value }) => `${context.join(" ")} | ${name}: ${value}`);
    expect(typed).toEqual([
      // No weight for the current spot: a state never changes a row's size (#2800, #2911).
      // A change's letter or count: a meta's size at the strong weight, in its change's colour.
      ".explorer .change-mark | font-size: var(--font-size-xs)",
      ".explorer .change-mark | font-weight: var(--font-weight-strong)",
    ]);
  });
});
