// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { Explorer } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type {
  ChatTouching,
  FolderEntry,
  OpenChat,
  Panels as PanelsModel,
  Piece,
  PlaneId,
} from "./bindings";
import { ProjectShownProvider } from "./projectShown";
import { onTheWire } from "./chatEvents.testkit";
import { theFrameNow } from "./frame.testkit";
import { countReactsCommits, counts, renderedBy } from "./reactCommits.testkit";

/**
 * **What a chat touching a file costs the Explorer** (#4730, part of #4724), counted in React's
 * own commits as `projectSwitch.cost.test.tsx` counts a switch.
 *
 * An agent reads and edits files as fast as it can, and each one is a `chat-touching` event.
 * Each used to draw the whole Explorer again, every row of every open branch with it, to move
 * one dot. Now nothing is drawn but the marks the touch changes (on the file, the folders above
 * it and the branch's own head): no row, so not React Aria's tree either, which builds every row
 * again whenever one row is drawn. The row's words for a reader move with the mark, which writes
 * them as the row's description. A project that is not in front draws no mark at all, so it
 * listens for none.
 *
 * **Held by counts, which are the same on every machine** (ADR 0086): which components were
 * drawn, by name, and how many in all.
 */

countReactsCommits();

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/plane" as unknown as PlaneId;

/** How many files the branch's own folder holds, each a row drawn. */
const FILES = 60;

/**
 * The most components one touch may draw: the file's mark and the branch head's, each with its
 * tooltip, 21 in all, however many {@link FILES} rows there are. A touch that drew its row drew
 * React Aria's frame of every row of the tree again, 334 for these rows; the explorer drawn again
 * with its rows was 1,257.
 */
const RENDERED = 25;

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

const ONE: Piece = {
  piece: "one",
  path: "/plane/workspaces/alpha/.worktrees/svc/one",
  branch: "one",
  wired: true,
  stale: false,
  said: "",
};

const WORKING = {
  session: 3,
  name: "fix login",
  cwd: ONE.path,
  harness: "claude-code",
  profile: null,
  persona: null,
} as OpenChat;

const entry = (name: string): FolderEntry => ({
  name,
  kind: "file",
  ignored: false,
  refused: null,
});

function core() {
  mockIPC(
    (cmd, args) => {
      const a = args as Record<string, string | null>;
      if (cmd === "branch_tree") {
        if (a.folder === "")
          return {
            entries: Array.from({ length: FILES }, (_, n) => entry(`file-${n}.ts`)),
            more: 0,
          };
        return { entries: [], more: 0 };
      }
      if (cmd === "branch_status") throw "not read here";
      if (cmd === "branch_ahead_behind") throw "not read here";
      if (cmd === "files_watch" || cmd === "branch_watch") return null;
      if (cmd === "project_icons_drawn") return null;
      if (cmd === "extension_icon_themes") return [];
      throw new Error(`unexpected ${cmd}`);
    },
    { shouldMockEvents: true },
  );
}

function draw(shown = true) {
  return render(drawn(shown));
}

function drawn(shown: boolean) {
  return (
    <ProjectShownProvider value={shown}>
      <ChatsHere.Provider value={fixedChats(nothingKnown)}>
        <Explorer
          plane={PLANE}
          workspace="alpha"
          state={{
            panels: PANELS,
            repos: { workspace: "alpha", repos: [], cache_refused: null },
            pieces: { svc: [ONE] },
            piecesRefused: {},
            reading: false,
          }}
          chats={[WORKING]}
          spot={{ repo: "svc", piece: "one", path: ONE.path }}
          onPick={() => {}}
          offers={new Map()}
          onPress={() => {}}
          onReadAgain={() => {}}
          onOpenFile={() => {}}
        />
      </ChatsHere.Provider>
    </ProjectShownProvider>
  );
}

/** Everything the explorer started has settled: its folder read, its listeners in place. */
async function settled() {
  await act(async () => {
    await new Promise((done) => setTimeout(done, 50));
  });
}

/** A touch as the core sends it, and the frame its marks are told in (`heldStore.ts`). */
const touch = (path: string) =>
  act(async () => {
    await emit(
      ...onTheWire("chat-touching", { plane: PLANE, session: 3, path } satisfies ChatTouching),
    );
    theFrameNow();
  });

const reset = () => {
  Object.assign(counts, { rendered: 0, mounted: 0, unmounted: 0, commits: 0 });
  renderedBy.clear();
};

/** What the row `id` says after its name, to a reader. */
const describedOn = (id: string) =>
  document.querySelector<HTMLElement>(`[data-row="${id}"]`)?.getAttribute("aria-description") ??
  undefined;

const markOn = (id: string) =>
  document
    .querySelector<HTMLElement>(`[data-row="${id}"]`)
    ?.querySelector(".touch-mark")
    ?.getAttribute("aria-label") ?? undefined;

describe("what a chat touching a file costs the explorer", () => {
  it("draws only the rows whose mark the touch changes, not the tree", async () => {
    core();
    draw();
    await screen.findByRole("row", { name: /^file-59\.ts/ });
    await settled();

    reset();
    await touch("file-7.ts");

    await waitFor(() =>
      expect(markOn("file:svc/one:file-7.ts")).toBe("fix login is working here now"),
    );
    expect(markOn("file:svc/one:file-8.ts")).toBeUndefined();
    expect(describedOn("file:svc/one:file-7.ts")).toBe("fix login is working here now");
    expect(describedOn("file:svc/one:file-8.ts")).toBeUndefined();
    // Neither the explorer nor any row is drawn again: only the marks. `NoDrag` is drawn once in
    // each row React Aria draws.
    expect(renderedBy.get("Explorer") ?? 0).toBe(0);
    expect(renderedBy.get("FileRow") ?? 0).toBe(0);
    expect(renderedBy.get("TreeRow") ?? 0).toBe(0);
    expect(renderedBy.get("NoDrag") ?? 0).toBe(0);
    expect(counts.rendered).toBeLessThanOrEqual(RENDERED);

    // The same chat touching the same file again changes no mark, so draws nothing.
    reset();
    await touch("file-7.ts");
    expect(counts.rendered).toBe(0);
  });

  it("does no work for a touch while its project is not in front", async () => {
    core();
    const { rerender } = draw(false);
    await settled();

    reset();
    await touch("file-7.ts");
    await settled();
    expect(counts.commits).toBe(0);

    // Back in front, it marks what its chats touch from then on.
    rerender(drawn(true));
    await screen.findByRole("row", { name: /^file-59\.ts/ });
    await settled();
    expect(markOn("file:svc/one:file-7.ts")).toBeUndefined();
    await touch("file-7.ts");
    await waitFor(() =>
      expect(markOn("file:svc/one:file-7.ts")).toBe("fix login is working here now"),
    );
  });
});
