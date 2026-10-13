// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { RequestMerge } from "./SavingMerge";
import { SaveIndicator, SavingView } from "./SavingView";
import { repoStageText, requestIsRed, requestWords } from "./saving";
import type { PlaneSaving, RepoSaving, SaveMerge } from "./bindings";

const opened: string[] = [];
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (url: string) => {
    opened.push(url);
    return Promise.resolve();
  },
}));

afterEach(() => {
  cleanup();
  clearMocks();
  opened.length = 0;
});

const PLANE = "/home/dev/plane";
const PR = "https://github.com/acme/plane/pull/12";

function standing(over: Partial<PlaneSaving> = {}): PlaneSaving {
  return {
    stage: "pr-open",
    changed: [],
    ahead: 1,
    pr: PR,
    request: "pull request",
    blocked: null,
    branch: "main",
    pushes: true,
    behind: 0,
    pushFailed: null,
    live: [],
    conflicts: [],
    notice: null,
    mode: "pr-merge",
    modeFrom: "charter.toml",
    journal: [],
    ...over,
  };
}

function repo(over: Partial<RepoSaving> = {}): RepoSaving {
  return {
    name: "widget",
    mode: "pr-merge",
    modeFrom: "charter.toml",
    autosave: false,
    stage: "pr-open",
    branch: "feature/x",
    changed: 0,
    ahead: 0,
    pr: "https://github.com/acme/widget/pull/3",
    request: "pull request",
    blocked: null,
    pushes: true,
    target: "main",
    ownBranch: false,
    ...over,
  };
}

const RED: SaveMerge = {
  state: "red",
  why: "2 checks failed: lint, e2e",
  checks: [
    { name: "lint", url: "https://github.com/acme/plane/actions/runs/9/job/5" },
    { name: "e2e", url: null },
  ],
};

function merge(state: string, why = "its checks are still running"): SaveMerge {
  return { state, why, checks: [] };
}

describe("what a pushed save waits on (ADR 0051, amended 2026-10-13)", () => {
  it("keeps the words it had for a request purlis does not merge", () => {
    expect(requestWords({ request: "pull request" })).toBe("Pushed — waiting on its pull request");
    expect(requestWords({ request: "merge request", merge: null })).toBe(
      "Pushed — waiting on its merge request",
    );
  });

  it("says red with what failed, and each other state in its own words", () => {
    expect(requestWords({ request: "pull request", merge: RED })).toBe(
      "Pushed — its pull request is red: 2 checks failed: lint, e2e",
    );
    expect(requestWords({ request: "pull request", merge: merge("waiting") })).toBe(
      "Pushed — its pull request waits for its checks",
    );
    expect(requestWords({ request: "merge request", merge: merge("queued") })).toBe(
      "Pushed — its merge request is in the merge queue",
    );
    expect(requestWords({ request: "pull request", merge: merge("cannot-merge") })).toBe(
      "Pushed — its pull request cannot merge",
    );
    expect(requestWords({ request: "pull request", merge: merge("merged") })).toBe(
      "Merged — moving onto what merged",
    );
    // A state a newer core names reads as waiting, never as nothing.
    expect(requestWords({ request: "pull request", merge: merge("brand-new") })).toBe(
      "Pushed — waiting on its pull request",
    );
  });

  it("calls only a failed check red", () => {
    expect(requestIsRed({ request: "pull request", merge: RED })).toBe(true);
    for (const state of ["waiting", "queued", "cannot-merge", "merged"])
      expect(requestIsRed({ request: "pull request", merge: merge(state) })).toBe(false);
    expect(requestIsRed({ request: "pull request" })).toBe(false);
  });

  it("says a repo's red request on its row", () => {
    expect(repoStageText(repo({ merge: RED }))).toBe(
      "Pushed — its pull request is red: 2 checks failed: lint, e2e",
    );
    expect(repoStageText(repo())).toBe("Pushed — waiting on its pull request");
  });
});

describe("RequestMerge", () => {
  it("draws a red request as a trouble Notice that names each failed check and links the ones with a page", async () => {
    render(<RequestMerge name="this project" request="pull request" pr={PR} merge={RED} />);

    const line = screen.getByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-red:this project");
    expect(line.textContent).toContain(
      "The pull request of this project is red, so purlis did not merge it: 2 checks failed: lint, e2e. It stays open, and the next save goes onto it.",
    );
    const failed = screen.getByRole("list", { name: "Checks that failed" });
    expect(
      within(failed)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["lint", "e2e"]);
    expect(within(failed).getByRole("link", { name: "lint" }).getAttribute("href")).toBe(
      "https://github.com/acme/plane/actions/runs/9/job/5",
    );
    // A check with no page is its name alone.
    expect(within(failed).queryByRole("link", { name: "e2e" })).toBeNull();

    // Its way out here is the request's own page.
    await userEvent.click(screen.getByRole("button", { name: "Open the pull request" }));
    expect(opened).toEqual([PR]);
  });

  it("draws a request the forge will not merge as a Notice with why", () => {
    render(
      <RequestMerge
        name="widget"
        request="merge request"
        pr="https://gitlab.com/acme/widget/-/merge_requests/3"
        merge={merge("cannot-merge", "!3 conflicts with its target branch")}
      />,
    );
    const line = screen.getByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-cannot-merge:widget");
    expect(line.textContent).toContain(
      "purlis cannot merge the merge request of widget as it stands: !3 conflicts with its target branch. It stays open.",
    );
    expect(screen.getByRole("button", { name: "Open the merge request" })).toBeTruthy();
  });

  it("says a waiting request in a note, which is not a Notice: nothing is wrong", () => {
    const { container } = render(
      <RequestMerge name="this project" request="pull request" pr={PR} merge={merge("waiting")} />,
    );
    expect(screen.queryByRole("status")).toBeNull();
    const note = container.querySelector('[data-merge="waiting"]');
    expect(note?.textContent).toBe(
      "purlis merges the pull request of this project itself once every check on it has passed: its checks are still running. It looks again by itself.",
    );
  });
});

describe("a red request in the window", () => {
  it("turns the title bar's save mark to the alert, in words that say red", () => {
    const { container } = render(
      <SaveIndicator
        saving={standing({ merge: RED })}
        busy={false}
        onOpen={() => undefined}
        onSave={() => undefined}
      />,
    );
    const bar = container.querySelector(".save-indicator");
    expect(bar).toHaveAttribute("data-stage", "pr-open");
    expect(bar).toHaveAttribute("data-checks", "failed");
    expect(
      screen.getByRole("button", {
        name: "Saving: Pushed — its pull request is red: 2 checks failed: lint, e2e",
      }),
    ).toBeTruthy();
  });

  it("leaves the mark as it was for a request that only waits", () => {
    const { container } = render(
      <SaveIndicator
        saving={standing({ merge: merge("waiting") })}
        busy={false}
        onOpen={() => undefined}
        onSave={() => undefined}
      />,
    );
    expect(container.querySelector(".save-indicator")).not.toHaveAttribute("data-checks");
  });

  it("says a repo's red request in the title bar when the project itself is saved", () => {
    const { container } = render(
      <SaveIndicator
        saving={standing({ stage: "saved", pr: null })}
        repos={[repo({ merge: RED }), repo({ name: "gadget" })]}
        busy={false}
        onOpen={() => undefined}
        onSave={() => undefined}
      />,
    );
    expect(container.querySelector(".save-indicator")).toHaveAttribute("data-checks", "failed");
    expect(
      screen.getByRole("button", { name: "Saving: 1 repo's pull request is red" }),
    ).toBeTruthy();
  });

  it("draws the project's red request in the Saving view, under its link", async () => {
    mockIPC((cmd) => (cmd === "plane_saving" ? standing({ merge: RED }) : null));
    render(<SavingView plane={PLANE} />);

    expect(
      await screen.findByText("Pushed — its pull request is red: 2 checks failed: lint, e2e"),
    ).toBeTruthy();
    const line = await screen.findByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-red:this project");
    expect(screen.getByRole("list", { name: "Checks that failed" })).toBeTruthy();
    // Red is not a block: no ways-out group, and Save is not offered for nothing to save.
    expect(screen.queryByRole("group", { name: "Ways out" })).toBeNull();
  });

  it("says in the mode line that purlis merges once every check has passed", async () => {
    mockIPC((cmd) => (cmd === "plane_saving" ? standing({ branch: "release" }) : null));
    render(<SavingView plane={PLANE} />);
    expect(
      await screen.findByText(
        "Mode: pr-merge (charter.toml) — a save pushes to this machine's save branch and keeps a pull request open into release, which purlis merges once every check on it has passed",
      ),
    ).toBeTruthy();
  });
});
