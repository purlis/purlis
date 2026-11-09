// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ArtifactOpened, ArtifactQuery } from "./bindings";
import { ArtifactTab } from "./ArtifactTab";

const TODOS: ArtifactQuery = {
  id: "purlis.todo_list@1",
  scope: "workspace",
  widens: ["done: the done todos, not only the open ones"],
  warning: null,
};

/** The core, answering the artifact's commands, with the names of those it was sent. */
function core(opened: Partial<ArtifactOpened>) {
  const sent: string[] = [];
  mockIPC((cmd) => {
    if (!cmd.startsWith("artifact_")) return null;
    sent.push(cmd);
    if (cmd === "artifact_about")
      return { name: "Backlog state", pinned_by: "Ada", missing: null, too_large: null };
    if (cmd === "artifact_open")
      return {
        label: "artifact-7",
        standing: "approved",
        changed_by: null,
        generated: true,
        queries: [],
        plane_granted: false,
        ...opened,
      };
    return null;
  });
  return sent;
}

function tab() {
  render(
    <ArtifactTab
      plane="p"
      workspace="alpha"
      folder="backlog-state"
      title="Backlog state"
      waits={false}
      onAsk={() => {}}
    />,
  );
}

describe("what an artifact's page reads of purlis, in its bar", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
  });

  it("lists the queries its page declared and the parameters that widen what it reads", async () => {
    core({
      standing: "new",
      queries: [TODOS, { ...TODOS, id: "purlis.change_status@1", widens: [] }],
    });
    tab();
    await waitFor(() =>
      expect(screen.getByTestId("artifact-queries")).toHaveTextContent(
        "Reads purlis.todo_list@1 (done: the done todos, not only the open ones), purlis.change_status@1",
      ),
    );
    expect(screen.queryByTestId("artifact-plane")).toBeNull();
  });

  it("says nothing of queries when its page declared none", async () => {
    core({});
    tab();
    await waitFor(() => expect(screen.getByTestId("artifact-standing")).toBeInTheDocument());
    expect(screen.queryByTestId("artifact-queries")).toBeNull();
  });

  it("warns of a deprecated query the page still calls", async () => {
    const warning = "Calls purlis.old@1, which purlis.todo_list replaces";
    core({ queries: [{ ...TODOS, warning }] });
    tab();
    await waitFor(() =>
      expect(screen.getByTestId("artifact-query-warning")).toHaveTextContent(warning),
    );
  });

  it("asks for plane-wide reads apart from the script, and only once the script is approved", async () => {
    const plane = { ...TODOS, id: "purlis.everything@1", scope: "plane", widens: [] };
    core({ standing: "new", queries: [plane] });
    tab();
    await waitFor(() =>
      expect(screen.getByTestId("artifact-plane")).toHaveTextContent(
        "Reads only this workspace until you allow project-wide reads for purlis.everything@1",
      ),
    );
    expect(screen.queryByRole("button", { name: "Allow project-wide reads" })).toBeNull();
    cleanup();

    const sent = core({ standing: "approved", queries: [plane] });
    tab();
    const allow = await screen.findByRole("button", { name: "Allow project-wide reads" });
    await userEvent.click(allow);
    await vi.waitFor(() => expect(sent).toContain("artifact_grant_plane"));
    expect(sent).not.toContain("artifact_approve");
  });

  it("says plane-wide reads are allowed once granted, with nothing to press", async () => {
    const plane = { ...TODOS, id: "purlis.everything@1", scope: "plane", widens: [] };
    core({ standing: "approved", plane_granted: true, queries: [plane] });
    tab();
    await waitFor(() =>
      expect(screen.getByTestId("artifact-plane")).toHaveTextContent(
        "Project-wide reads allowed on this machine",
      ),
    );
    expect(screen.queryByRole("button", { name: "Allow project-wide reads" })).toBeNull();
  });
});
