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
  it("heads the Files section with the one section label, and what it is of beside it as data", async () => {
    core({ "one:": [entry("src", { kind: "folder" }), entry("README.md")] });
    const drawn = draw({ spot: PICKED });
    await screen.findByRole("treeitem", { name: /README\.md/ });

    const head = drawn.querySelector('[data-section="files"] .explorer-section-head');
    wears(head, "label", "the Files heading");
    // The words are the disclosure's, a row of the set: it wears the label itself, or the
    // row's own colour would draw the title brighter than every other section's.
    const title = head?.querySelector("button");
    wears(title, "label", "the Files heading's button");

    const of = head?.querySelector(".section-of");
    wears(of, "secondary", "what Files is of");
    // Data, never capitals (T4): the label's capitals and spacing stop at it.
    const rule = declarations(readFileSync(join(process.cwd(), "src", "App.css"), "utf8")).filter(
      ({ context }) => context.join(" ").endsWith(".explorer .explorer-section-head .section-of"),
    );
    expect(Object.fromEntries(rule.map((one) => [one.property, one.value]))).toMatchObject({
      "text-transform": "none",
      "letter-spacing": "normal",
    });
  });

  it("draws a file's and a folder's name as a body, from the Explorer's root", async () => {
    core({ "one:": [entry("src", { kind: "folder" }), entry("README.md")] });
    const drawn = draw({ spot: PICKED });
    const file = await screen.findByRole("treeitem", { name: /README\.md/ });
    const folder = await screen.findByRole("treeitem", { name: /^src/ });

    expect(drawn.querySelector(".explorer")).toHaveClass("ui-text-body");
    drawnAs(file.querySelector(".spot-name"), "body", "a file's name");
    drawnAs(folder.querySelector(".spot-name"), "body", "a folder's name");
  });

  it("says an empty folder as a secondary line, not a dimmed body", async () => {
    core({ "one:": [] });
    const drawn = draw({ spot: PICKED });
    const none = await within(drawn).findByText("Nothing here");
    wears(none, "secondary", "Nothing here");
    expect(drawnWith(none, "opacity")).toBeUndefined();
  });

  it("draws the Repos view's rows on the roles: names, where, counts and marks", () => {
    core({});
    const drawn = draw({ part: "repos" });
    const tree = within(drawn).getByRole("tree", { name: "Repos and branches" });

    wears(drawn.querySelector(".explorer-section-head"), "label", "the Repos title");
    const root = tree.querySelector(".spot-root");
    drawnAs(root?.querySelector(".spot-name"), "body", "the workspace's name", true);
    wears(root?.querySelector(".spot-what"), "secondary", "the workspace itself");

    const clone = within(drawn).getByTestId("clone-svc");
    wears(clone.querySelector(".repo"), "body", "a repo's name", true);
    // A count is the count family's pill (#2912), its number at the strong weight (T5).
    expect(clone.querySelector(".piece-count")).toHaveAttribute("data-scope", "count");
    drawnAs(clone.querySelector(".spot .spot-name"), "body", "a branch's name");
    wears(clone.querySelector(".spot-in"), "secondary", "the repo a branch is in");
    wears(clone.querySelector(".worktree"), "meta", "a branch's marks");

    wears(within(drawn).getByText("No branches cut here"), "secondary", "No branches cut here");
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
      // The current spot's own weight, on the row that names where the next chat starts.
      '.explorer .spot[aria-current="true"] | font-weight: var(--font-weight-strong)',
      // A change's letter or count: a meta's size at the strong weight, in its change's colour.
      ".explorer .change-mark | font-size: var(--font-size-xs)",
      ".explorer .change-mark | font-weight: var(--font-weight-strong)",
    ]);
  });
});
