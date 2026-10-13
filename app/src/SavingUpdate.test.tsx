// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SaveIndicator, SavingView } from "./SavingView";
import { askUpdate, takeUpdateAsk } from "./saving";
import type { PlaneSaving, ProjectUpdate } from "./bindings";

/**
 * **Update from remote, in the window** (qw348): where the Saving view and the title bar say
 * how many commits came in, a button brings them in. A project with unsaved work is saved
 * first or left alone, never moved, and an update that left the project as it was is a Notice
 * with the reason and a way out.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  takeUpdateAsk(PLANE);
});

const PLANE = "/home/dev/plane";

function standing(over: Partial<PlaneSaving> = {}): PlaneSaving {
  return {
    stage: "saved",
    changed: [],
    ahead: 0,
    pr: null,
    request: "pull request",
    blocked: null,
    branch: "main",
    pushes: true,
    behind: 259,
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

function ended(over: Partial<ProjectUpdate> = {}): ProjectUpdate {
  return {
    outcome: "updated",
    reason: null,
    said: "Updated: brought in 259 incoming commits.",
    files: [],
    whole: true,
    lines: ["Fetching `main` from the remote …"],
    ...over,
  };
}

type Asked = { cmd: string; args: Record<string, unknown> };

function core(plane: PlaneSaving, updates: ProjectUpdate[], save: unknown = ["✓ Saved"]): Asked[] {
  const asked: Asked[] = [];
  const left = [...updates];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: args as Record<string, unknown> });
    if (cmd === "plane_saving") return plane;
    if (cmd === "workspace_saving") return [];
    if (cmd === "update_project") return left.shift() ?? ended();
    if (cmd === "save_plane") {
      if (typeof save === "string") throw save;
      return save;
    }
    return null;
  });
  return asked;
}

const did = (asked: Asked[]) =>
  asked.map((call) => call.cmd).filter((cmd) => cmd === "save_plane" || cmd === "update_project");

describe("Update from remote in the Saving view", () => {
  it("is offered where commits came in, and says what it brought in", async () => {
    const asked = core(standing(), [ended()]);
    render(<SavingView plane={PLANE} />);

    await userEvent.click(await screen.findByRole("button", { name: "Update" }));

    expect(await screen.findByText(/Updated: brought in 259 incoming commits\./)).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Fetching `main`");
    expect(asked.find((call) => call.cmd === "update_project")?.args).toMatchObject({
      plane: PLANE,
    });
  });

  it("is not offered where nothing came in", async () => {
    core(standing({ behind: 0 }), []);
    render(<SavingView plane={PLANE} />);

    await screen.findByText("Saved");
    expect(screen.queryByRole("button", { name: "Update" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Save, then update" })).toBeNull();
  });

  it("saves first where the project has unsaved work, and only then updates", async () => {
    const asked = core(standing({ stage: "changed", changed: ["a.md"] }), [ended()]);
    render(<SavingView plane={PLANE} />);

    expect(await screen.findByRole("button", { name: "Save, then update" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Update" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Save, then update" }));

    await waitFor(() => expect(did(asked)).toEqual(["save_plane", "update_project"]));
  });

  it("does not update where the save was refused, and says the refusal", async () => {
    const asked = core(
      standing({ stage: "changed", changed: ["a.md"] }),
      [ended()],
      "a secret-shaped value in a staged file",
    );
    render(<SavingView plane={PLANE} />);

    await userEvent.click(await screen.findByRole("button", { name: "Save, then update" }));

    expect((await screen.findByRole("alert")).textContent).toContain("secret-shaped");
    expect(did(asked)).toEqual(["save_plane"]);
  });

  it("says a conflict as a Notice naming the files, with a terminal as the way out", async () => {
    core(standing({ stage: "committed", ahead: 1 }), [
      ended({
        outcome: "left",
        reason: "conflict",
        said: "Nothing was updated: this project's own commits and the incoming ones change the same lines in `a.md`. The project is exactly as it was.",
        files: ["a.md"],
      }),
    ]);
    const way = vi.fn();
    window.addEventListener("charter-saving-way-out", way);
    render(<SavingView plane={PLANE} />);

    await userEvent.click(await screen.findByRole("button", { name: "Update" }));

    const notice = await screen.findByText(/exactly as it was/);
    expect(notice.closest("[data-cause]")?.getAttribute("data-cause")).toBe(
      "saving-update-left:conflict",
    );
    expect(screen.getByRole("list", { name: "The files it is about" }).textContent).toBe("a.md");
    await userEvent.click(screen.getByRole("button", { name: "Open terminal here" }));
    window.removeEventListener("charter-saving-way-out", way);
    expect(way).toHaveBeenCalledTimes(1);
  });

  it("offers Update again where nothing in the project is wrong", async () => {
    const asked = core(standing(), [
      ended({
        outcome: "left",
        reason: "not-fetched",
        said: "Nothing was updated: the remote could not be fetched: offline.",
      }),
      ended(),
    ]);
    render(<SavingView plane={PLANE} />);

    await userEvent.click(await screen.findByRole("button", { name: "Update" }));
    await userEvent.click(await screen.findByRole("button", { name: "Update again" }));

    expect(await screen.findByText(/Updated: brought in/)).toBeTruthy();
    expect(screen.queryByText(/could not be fetched/)).toBeNull();
    expect(did(asked)).toEqual(["update_project", "update_project"]);
  });

  it("says save first as a Notice where the core found unsaved work the view had not", async () => {
    const asked = core(standing(), [
      ended({
        outcome: "left",
        reason: "unsaved",
        said: "Save first, then update: 1 changed file here is not saved, and an update never moves unsaved work.",
      }),
      ended(),
    ]);
    render(<SavingView plane={PLANE} />);

    await userEvent.click(await screen.findByRole("button", { name: "Update" }));
    expect(await screen.findByText(/Save first, then update/)).toBeTruthy();
    // The stage was "saved" when the view last read it, so its save has nothing to take: the
    // Notice says the sentence and is dismissed, and the next read offers Save, then update.
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(screen.queryByText(/Save first, then update/)).toBeNull();
    expect(did(asked)).toEqual(["update_project"]);
  });

  it("runs an update the title bar asked for, once", async () => {
    const asked = core(standing(), [ended()]);
    askUpdate(PLANE);
    render(<SavingView plane={PLANE} />);

    expect(await screen.findByText(/Updated: brought in/)).toBeTruthy();
    expect(did(asked)).toEqual(["update_project"]);
    expect(takeUpdateAsk(PLANE)).toBe(false);
  });
});

describe("the title bar's incoming count", () => {
  it("is the Update button where the bar can update, named in full", async () => {
    const onUpdate = vi.fn();
    render(
      <SaveIndicator
        saving={standing()}
        busy={false}
        onOpen={() => undefined}
        onSave={() => undefined}
        onUpdate={onUpdate}
      />,
    );

    const update = screen.getByRole("button", {
      name: "Update: bring in 259 incoming commits from the remote",
    });
    expect(update.textContent).toBe("↓259");
    await userEvent.click(update);
    expect(onUpdate).toHaveBeenCalledTimes(1);
  });

  it("stays part of the stage where the bar cannot update", () => {
    render(
      <SaveIndicator
        saving={standing({ behind: 1 })}
        busy={false}
        onOpen={() => undefined}
        onSave={() => undefined}
      />,
    );

    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.getByRole("button").textContent).toContain("↓1");
  });
});
