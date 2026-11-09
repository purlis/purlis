// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PlaneId, StartedHere } from "./bindings";
import { askArtifactDiscuss, DiscussButton, useArtifactDiscuss } from "./artifactDiscuss";
import { ArtifactTab } from "./ArtifactTab";

const PLANE = "p1" as PlaneId;

const STARTED: StartedHere = {
  session: 7,
  name: "7",
  label: "Discuss Weekly",
  harness: "claude",
  workspace: "w",
  text: "About the artifact artifacts/weekly/ … @.charter/discuss/weekly.md",
  copied: null,
};

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The core, answering `artifact_discuss` with `answer`, and what it was asked. */
function core(answer: () => unknown) {
  const asked: Record<string, unknown>[] = [];
  mockIPC((cmd, args) => {
    if (cmd !== "artifact_discuss") return null;
    asked.push(args as Record<string, unknown>);
    return answer();
  });
  return asked;
}

function listening() {
  const placed = vi.fn();
  const said = vi.fn();
  const approving = vi.fn();
  renderHook(() => useArtifactDiscuss(PLANE, placed, said, approving));
  return { placed, said, approving };
}

describe("Discuss in an artifact's bar", () => {
  it("is a button that asks its project's window to discuss the artifact", async () => {
    const heard = vi.fn();
    window.addEventListener("purlis-artifact-discuss", heard);
    render(<DiscussButton plane={PLANE} workspace="w" folder="weekly" />);
    await userEvent.click(screen.getByRole("button", { name: "Discuss" }));
    window.removeEventListener("purlis-artifact-discuss", heard);
    expect(heard).toHaveBeenCalledTimes(1);
    expect((heard.mock.calls[0][0] as CustomEvent).detail).toEqual({
      plane: PLANE,
      workspace: "w",
      folder: "weekly",
    });
  });

  it("asks the core for the artifact's chat and places the chat it started", async () => {
    const asked = core(() => STARTED);
    const { placed, said } = listening();
    askArtifactDiscuss({ plane: PLANE, workspace: "w", folder: "weekly" });
    await vi.waitFor(() => expect(placed).toHaveBeenCalledTimes(1));
    expect(asked).toEqual([
      { plane: PLANE, workspace: "w", folder: "weekly", columns: 80, rows: 24 },
    ]);
    expect(placed).toHaveBeenCalledWith({ ...STARTED, persona: null });
    expect(said).not.toHaveBeenCalled();
  });

  it("leaves another project's ask to that project's window", async () => {
    const asked = core(() => STARTED);
    const { placed } = listening();
    askArtifactDiscuss({ plane: "p2" as PlaneId, workspace: "w", folder: "weekly" });
    await new Promise((done) => setTimeout(done, 0));
    expect(asked).toEqual([]);
    expect(placed).not.toHaveBeenCalled();
  });

  it("says the line is on the clipboard where purlis cannot type into the harness", async () => {
    core(() => ({
      ...STARTED,
      copied:
        "purlis cannot type into opencode yet, so the line attaching the page is on the clipboard",
    }));
    const { placed, said } = listening();
    askArtifactDiscuss({ plane: PLANE, workspace: "w", folder: "weekly" });
    await vi.waitFor(() => expect(said).toHaveBeenCalledTimes(1));
    expect(placed).toHaveBeenCalledTimes(1);
    expect(said).toHaveBeenCalledWith({
      from: "artifact.discuss",
      refused: false,
      words:
        "Purlis cannot type into opencode yet, so the line attaching the page is on the clipboard.",
    });
  });

  it("says the core's refusal and opens nothing", async () => {
    core(() => {
      throw { words: "'weekly' is not pinned to this workspace any more", approval: null };
    });
    const { placed, said } = listening();
    askArtifactDiscuss({ plane: PLANE, workspace: "w", folder: "weekly" });
    await waitFor(() => expect(said).toHaveBeenCalledTimes(1));
    expect(placed).not.toHaveBeenCalled();
    expect(said).toHaveBeenCalledWith({
      from: "artifact.discuss",
      refused: true,
      words: "'weekly' is not pinned to this workspace any more",
    });
  });
});

describe("the artifact bar's Discuss", () => {
  /** The core answering what the bar asks before anything runs. */
  function about(missing: string | null) {
    mockIPC((cmd) =>
      cmd === "artifact_about"
        ? { name: "Weekly", pinned_by: "Ada", missing, too_large: null, fresh: null }
        : null,
    );
  }

  it("is offered on an artifact that can be opened, before its page runs", async () => {
    about(null);
    render(
      <ArtifactTab
        plane={PLANE}
        workspace="w"
        folder="weekly"
        title="Weekly"
        waits
        onAsk={() => {}}
      />,
    );
    const bar = await screen.findByTestId("artifact-bar");
    await waitFor(() => expect(bar.textContent).toContain("Ada"));
    expect(screen.getByRole("button", { name: "Discuss" })).toBeTruthy();
  });

  it("is not offered on a missing artifact, which has no page to attach", async () => {
    about("there is no folder 'weekly' in artifacts/");
    render(
      <ArtifactTab
        plane={PLANE}
        workspace="w"
        folder="weekly"
        title="Weekly"
        waits={false}
        onAsk={() => {}}
      />,
    );
    await screen.findByTestId("artifact-missing");
    expect(screen.queryByRole("button", { name: "Discuss" })).toBeNull();
  });
});
