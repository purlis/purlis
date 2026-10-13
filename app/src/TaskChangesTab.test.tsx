// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { TaskChangesTab } from "./TaskChangesTab";
import type { TaskChanges } from "./bindings";

/**
 * **A task that changed nothing says what would be listed** (DS-3 #626, FR-19 #614): the
 * tab's empty state is an `EmptyState`, in the tab's own words for a task on its own branch
 * and for one in a shared folder. The tab against the whole window is
 * `TaskChanges.window.test.tsx`'s.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function changes(over: Partial<TaskChanges> = {}): TaskChanges {
  return {
    id: "01K6TASK",
    task: "fix the queue",
    running: false,
    own: null,
    places: [],
    elsewhere: [],
    more: false,
    said: null,
    unknown: null,
    ...over,
  };
}

function open(read: TaskChanges) {
  mockIPC((cmd) => (cmd === "task_changes" ? read : null));
  render(<TaskChangesTab plane={PLANE} id={read.id} changed={0} onOpenView={vi.fn()} />);
  return screen.findByTestId("task-changes-empty");
}

describe("a task that changed nothing", () => {
  it("says, on its own branch, that its changes would be listed here", async () => {
    const empty = await open(
      changes({
        own: { repo: "api", branch: "fix-the-queue", standing: "kept", acts: false, left: null },
      }),
    );

    expect(within(empty).getByText("No changed file to show")).toBeInTheDocument();
    expect(empty).toHaveTextContent(/A file the task changes on fix-the-queue is listed here/);
  });

  it("says, in a shared folder, which of its files would be listed here", async () => {
    const empty = await open(changes());

    expect(
      within(empty).getByText("No file its edit tools wrote is uncommitted here"),
    ).toBeInTheDocument();
    expect(empty).toHaveTextContent(/listed here while it is uncommitted/);
  });

  it("draws no empty state where purlis cannot say what changed", async () => {
    mockIPC((cmd) =>
      cmd === "task_changes" ? changes({ unknown: "Its folder was discarded." }) : null,
    );
    render(<TaskChangesTab plane={PLANE} id="01K6TASK" changed={0} onOpenView={vi.fn()} />);

    expect(await screen.findByTestId("task-changes-unknown")).toBeInTheDocument();
    expect(screen.queryByTestId("task-changes-empty")).toBeNull();
  });
});

describe("a path the engine named and does not read (#704)", () => {
  const WHY =
    "Named and not read: a folder on its path is called .git, the name git keeps for its own folder.";

  function withRefused(): TaskChanges {
    return changes({
      own: { repo: "api", branch: "fix-the-queue", standing: "kept", acts: false, left: null },
      places: [
        {
          workspace: "alpha",
          repo: "api",
          piece: "fix-the-queue",
          base: "main",
          files: [],
          refused: [{ path: "vendor/.GIT/hooks/x", mark: "added", why: WHY, uncommitted: true }],
          more: 0,
          unread: null,
        },
      ],
    });
  }

  it("draws its path, its mark and why, and offers nothing to open, stage or compare", async () => {
    const onOpenView = vi.fn();
    mockIPC((cmd) => (cmd === "task_changes" ? withRefused() : null));
    render(<TaskChangesTab plane={PLANE} id="01K6TASK" changed={0} onOpenView={onOpenView} />);

    const row = await screen.findByTestId("task-changes-refused-row");
    expect(row).toHaveTextContent("vendor/.GIT/hooks/x");
    expect(row).toHaveTextContent("added");
    expect(row).toHaveTextContent("not committed");
    expect(row).toHaveTextContent(WHY);
    expect(within(row).queryByRole("button")).toBeNull();
    expect(within(row).queryByRole("link")).toBeNull();
    expect(row.querySelector("[tabindex]")).toBeNull();
    expect(onOpenView).not.toHaveBeenCalled();
  });

  it("is a change, so the tab does not say nothing changed", async () => {
    mockIPC((cmd) => (cmd === "task_changes" ? withRefused() : null));
    render(<TaskChangesTab plane={PLANE} id="01K6TASK" changed={0} onOpenView={vi.fn()} />);

    await screen.findByTestId("task-changes-refused-row");
    expect(screen.queryByTestId("task-changes-empty")).toBeNull();
  });
});
