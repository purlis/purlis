// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneId, WorkspaceFile } from "../bindings";
import { PieceFileTab } from "./PieceFiles";
import { workspaceFiles } from "../pieceViews";
import type { Offer } from "../actions";

/**
 * **A workspace's own file in the light editor** (#5375): read through `workspace_file`, never a
 * branch's command; one purlis keeps says so, read-only, with the way to its own view.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/plane" as unknown as PlaneId;
const OWN = workspaceFiles("alpha");

function core(files: Record<string, WorkspaceFile>) {
  const asked: string[] = [];
  mockIPC((cmd, args) => {
    const a = args as Record<string, unknown>;
    asked.push(cmd);
    if (cmd === "workspace_file") {
      const answer = files[String(a.path)];
      if (answer === undefined)
        throw `'${String(a.path)}' is not in the workspace's folder any more`;
      return answer;
    }
    return null;
  });
  return asked;
}

const text = (
  body: string,
  managed = false,
  view: WorkspaceFile["view"] = null,
): WorkspaceFile => ({
  file: { kind: "text", text: body },
  managed,
  view,
});

describe("a workspace's own file (#5375)", () => {
  it("opens the person's file by the workspace's own command, with no managed mark", async () => {
    const asked = core({ "notes/plan.md": text("the plan") });
    render(<PieceFileTab plane={PLANE} cut={OWN} path="notes/plan.md" />);

    expect(await screen.findByText("the plan")).toBeInTheDocument();
    expect(asked).toContain("workspace_file");
    expect(asked).not.toContain("piece_file");
    // A workspace's own files are no branch: nothing of them is compared.
    expect(asked).not.toContain("branch_status");
    expect(screen.queryByText(/Kept by purlis/)).toBeNull();
  });

  it("says a file purlis keeps is managed, and links to its own view", async () => {
    core({ "memory/one.md": text("a memory", true, "memory") });
    const onPress = vi.fn<(offer: Offer) => void>();
    render(<PieceFileTab plane={PLANE} cut={OWN} path="memory/one.md" onPress={onPress} />);

    expect(await screen.findByText(/Kept by purlis, so it opens read-only/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open Memory" }));

    expect(onPress.mock.calls[0][0].does).toEqual({ verb: "showSideView", view: "memory" });
  });

  it("links workspace.md to its vision's own editor", async () => {
    core({ "workspace.md": text("# alpha", true, "vision") });
    const onPress = vi.fn<(offer: Offer) => void>();
    render(<PieceFileTab plane={PLANE} cut={OWN} path="workspace.md" onPress={onPress} />);

    await userEvent.click(await screen.findByRole("button", { name: "Edit the vision" }));

    expect(onPress.mock.calls[0][0].does).toEqual({ verb: "editVision", workspace: "alpha" });
  });

  it("says a managed file with no view of its own is managed, with no link", async () => {
    core({ "workspace.json": text("{}", true, null) });
    render(<PieceFileTab plane={PLANE} cut={OWN} path="workspace.json" onPress={() => {}} />);

    expect(await screen.findByText(/Kept by purlis/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /^(Open (Memory|Todos|Sessions)|Edit the vision)$/ }),
    ).toBeNull();
  });
});
