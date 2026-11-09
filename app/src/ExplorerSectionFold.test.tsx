// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { Panels as PanelsModel, PlaneId } from "./bindings";
import { forgetExplorerSections } from "./explorerSections";
import type { WorkspaceState } from "./workspaceState";

/**
 * **The whole Explorer's Repos and branches folds under its heading** (#1677), and since #2930
 * the fold is the disclosure family's (`ui/disclosure`), not a row of the old `.ui-row` set. What
 * a keyboard and a screen reader meet is held here, so the move changes none of it: a level-2
 * heading holding one button named by the section, saying `aria-expanded` and controlling the
 * body; a press folds and opens it, and the keyboard stays on the button; a folded body is
 * hidden and keeps its tree mounted, so its folds and its stop come back with it.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetExplorerSections();
});

const PANELS: PanelsModel = {
  workspace: "alpha",
  repos: ["svc"],
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

const STATE: WorkspaceState = {
  panels: PANELS,
  repos: { workspace: "alpha", repos: [], cache_refused: null },
  pieces: { svc: [] },
  piecesRefused: {},
  reading: false,
};

function draw() {
  mockIPC(
    (cmd) => {
      if (cmd === "branch_tree") return { entries: [], more: 0 };
      if (cmd === "files_watch") return null;
      if (cmd === "project_icons_drawn") return null;
      if (cmd === "extension_icon_themes") return [];
      if (cmd === "window_prefs_write" || cmd === "layout_write") return null;
      return null;
    },
    { shouldMockEvents: true },
  );
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <Explorer
        plane={"/plane" as unknown as PlaneId}
        workspace="alpha"
        state={STATE}
        chats={[]}
        spot={undefined}
        onPick={() => {}}
        offers={new Map()}
        onPress={() => {}}
        onReadAgain={() => {}}
        onOpenFile={() => {}}
      />
    </ChatsHere.Provider>,
  );
}

describe("the whole Explorer's Repos and branches fold (#1677, #2930)", () => {
  it("is a level-2 heading holding one button that says it is open and names what it controls", () => {
    draw();
    const heading = screen.getByRole("heading", { level: 2, name: /Repos and branches/ });
    const button = within(heading).getByRole("button", { name: "Repos and branches" });
    expect(button).toHaveAttribute("aria-expanded", "true");
    const body = document.getElementById(button.getAttribute("aria-controls") ?? "");
    expect(body).not.toBeNull();
    expect(body).toContainElement(screen.getByRole("treegrid", { name: "Repos and branches" }));
  });

  it("is the disclosure family's fold, its button the button family's, and no row of the old set", () => {
    draw();
    const button = screen.getByRole("button", { name: "Repos and branches" });
    expect(button).toHaveAttribute("data-scope", "button");
    expect(button.closest('[data-scope="disclosure"][data-part="trigger"]')).toBe(
      screen.getByRole("heading", { level: 2, name: /Repos and branches/ }),
    );
    expect(document.querySelector(".ui-row")).toBeNull();
  });

  it("folds and opens on a press, keeps the keyboard on its button, and keeps the tree mounted", async () => {
    draw();
    const user = userEvent.setup();
    const button = screen.getByRole("button", { name: "Repos and branches" });
    const tree = screen.getByRole("treegrid", { name: "Repos and branches" });

    button.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(button).toHaveAttribute("aria-expanded", "false"));
    expect(button).toHaveFocus();
    // Hidden, never unmounted: the same tree is still in the document.
    expect(tree).toBeInTheDocument();
    expect(tree).not.toBeVisible();

    await user.keyboard(" ");
    await waitFor(() => expect(button).toHaveAttribute("aria-expanded", "true"));
    expect(button).toHaveFocus();
    expect(screen.getByRole("treegrid", { name: "Repos and branches" })).toBe(tree);
    expect(tree).toBeVisible();
  });

  it("is reached by Tab", async () => {
    draw();
    const user = userEvent.setup();
    await user.tab();
    expect(screen.getByRole("button", { name: "Repos and branches" })).toHaveFocus();
  });
});
