// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { ViewPane } from "./Views";
import { memoryView, type MemoryRef } from "./memories";
import type { PlaneAnswer, PlaneChange } from "./bindings";

/**
 * **A view tab reads again for what it shows** (#1160 L3). Every view tab followed every write
 * to any store the views read, so a todo closed in a terminal asked each open memory tab for
 * its memory again. A memory's tab follows its own store now, from the changes `plane-changed`
 * already carries; a view that cannot say what it shows follows them all, as before.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const NOTE: MemoryRef = { scope: { kind: "workspace", name: "alpha" }, slug: "deploy-key" };

function core(): { asked: string[] } {
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "memory_read")
        return {
          scope: NOTE.scope,
          slug: NOTE.slug,
          title: "The deploy key rotates",
          stamp: "2026-10-10 16:50",
          place: "alpha",
          body: "Every month.",
          path: "workspaces/alpha/memory/deploy-key.md",
          text: "# The deploy key rotates\n\nEvery month.\n",
        };
      return null;
    },
    { shouldMockEvents: true },
  );
  return { asked };
}

/** What the core says when a file of `kind` is written in workspace `workspace`. */
const written = (kind: PlaneChange["kind"], workspace: string, answers: PlaneAnswer[]) =>
  act(() =>
    emit("plane-changed", {
      plane: PLANE,
      changes: [{ kind, workspace, persona: null, path: `workspaces/${workspace}/${kind}/x.md` }],
      answers,
    }),
  );

const settle = () => act(() => new Promise((done) => setTimeout(done, 30)));

describe("a memory's tab, told the plane changed", () => {
  it("reads its memory again for a write to its store, and not for a todo or another store", async () => {
    const { asked } = core();
    render(
      <ViewPane
        plane={PLANE}
        view={memoryView(NOTE)}
        title="The deploy key rotates"
        waits={false}
        offered={[]}
        onOpenView={() => {}}
        onAsk={() => {}}
        onVaultChanged={() => {}}
      />,
    );
    await screen.findByText("Every month.");
    await settle();
    const reads = () => asked.filter((one) => one === "memory_read").length;
    const first = reads();

    // A todo closed in alpha, and a memory saved in beta: what `planechange.rs` says each is.
    await written("todos", "alpha", [
      { answer: "sidebar" },
      { answer: "panels", workspace: "alpha" },
      { answer: "git" },
      { answer: "views" },
    ]);
    await written("memory", "beta", [{ answer: "panels", workspace: "beta" }, { answer: "views" }]);
    await settle();
    expect(reads()).toBe(first);

    await written("memory", "alpha", [
      { answer: "panels", workspace: "alpha" },
      { answer: "views" },
    ]);
    await waitFor(() => expect(reads()).toBe(first + 1));
  });
});
