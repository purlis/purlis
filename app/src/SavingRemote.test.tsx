// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SavingView } from "./SavingView";
import type { PlaneSaving, RemoteQuestion, RepoSaving } from "./bindings";

/**
 * **The Saving view's question about a clone's remote** (#1848): a Notice naming the remote,
 * whose press records it and saves, for the project and for each repo.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function standing(over: Partial<PlaneSaving> = {}): PlaneSaving {
  return {
    stage: "committed",
    changed: [],
    ahead: 1,
    pr: null,
    request: "pull request",
    blocked: null,
    branch: "main",
    pushes: true,
    behind: 0,
    pushFailed: null,
    live: [],
    conflicts: [],
    notice: null,
    mode: "push",
    modeFrom: "charter.toml",
    journal: [],
    ...over,
  };
}

function repo(over: Partial<RepoSaving> = {}): RepoSaving {
  return {
    name: "widget",
    mode: "push",
    modeFrom: "default",
    autosave: true,
    stage: "committed",
    branch: "main",
    changed: 0,
    ahead: 1,
    pr: null,
    request: "pull request",
    blocked: null,
    pushes: true,
    target: null,
    ownBranch: false,
    ...over,
  };
}

const NOT_SAVED: RemoteQuestion = { kind: "not-saved", now: "github.com/acme/plane" };

type Asked = { cmd: string; args: Record<string, unknown> };

function core(plane: PlaneSaving, repos: RepoSaving[] = []): Asked[] {
  const asked: Asked[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "plane_saving") return plane;
    if (cmd === "workspace_saving") return repos;
    if (cmd === "confirm_remote") return ["✓ Pushed"];
    return null;
  });
  return asked;
}

describe("the question about a clone's remote", () => {
  it("says the project waits for one save from the window, naming its remote", async () => {
    const asked = core(standing({ remote: NOT_SAVED }));
    render(<SavingView plane={PLANE} />);

    const notice = await screen.findByText(/has not saved this project from the window yet/);
    expect(notice.textContent).toContain("github.com/acme/plane");
    await userEvent.click(screen.getByRole("button", { name: "Save once from here" }));

    await waitFor(() =>
      expect(asked.find((call) => call.cmd === "confirm_remote")?.args).toMatchObject({
        plane: PLANE,
        workspace: null,
        repo: null,
      }),
    );
    expect(await screen.findByText("✓ Pushed")).toBeTruthy();
  });

  it("asks about a repo by its name, and confirms that repo's remote", async () => {
    const asked = core(standing(), [
      repo({ name: "api" }),
      repo({ name: "widget", remote: { kind: "not-saved", now: "github.com/acme/widget" } }),
    ]);
    render(<SavingView plane={PLANE} workspace="alpha" />);

    await screen.findByText(/has not saved widget from the window yet/);
    expect(screen.queryByText(/has not saved api/)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Save once from here" }));

    await waitFor(() =>
      expect(asked.find((call) => call.cmd === "confirm_remote")?.args).toMatchObject({
        plane: PLANE,
        workspace: "alpha",
        repo: "widget",
      }),
    );
  });

  it("asks nothing where there is no question", async () => {
    core(standing());
    render(<SavingView plane={PLANE} />);

    await screen.findByText(/Mode: push/);
    expect(screen.queryByRole("button", { name: "Save once from here" })).toBeNull();
  });
});
