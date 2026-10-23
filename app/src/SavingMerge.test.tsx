// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProjectShownProvider } from "./projectShown";
import { RequestMerge, type RequestOf } from "./SavingMerge";
import { SaveIndicator, SavingView } from "./SavingView";
import {
  FIX_STARTED,
  PLANE_SAVED,
  repoStageText,
  requestIsRed,
  requestWords,
  type FixStarted,
} from "./saving";
import type { Fixing, PlaneSaving, RepoSaving, SaveMerge } from "./bindings";

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
  needs: null,
  number: "#12",
  shown: "red - e2e\u001flint",
};

/** The commit the request is at when the window looks. */
const HEAD = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

/** What a fresh look finds when a third check has failed since the last one. */
const RED_NOW: SaveMerge = {
  ...RED,
  why: "3 checks failed: lint, e2e, build",
  checks: [...RED.checks, { name: "build", url: null }],
  shown: "red - build\u001fe2e\u001flint",
};

function merge(
  state: string,
  why = "its checks are still running",
  needs: string | null = null,
): SaveMerge {
  return { state, why, checks: [], needs, number: "#12", shown: `${state} ${needs ?? "-"} ` };
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

/** A request's Notice, with the project's own request unless `over` says a repo's. */
function request(merge: SaveMerge, over: Partial<RequestOf> = {}) {
  return (
    <RequestMerge
      plane={PLANE}
      workspace={null}
      repo={null}
      name="this project"
      request="pull request"
      pr={PR}
      target="main"
      merge={merge}
      {...over}
    />
  );
}

/** Every command the window sent, and what each is answered with. */
function commandsSent(answers: Record<string, unknown> = {}) {
  const sent: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    sent.push({ cmd, args: args as Record<string, unknown> });
    const answer = answers[cmd];
    if (answer instanceof Error) throw answer.message;
    return answer ?? null;
  });
  return sent;
}

const buttons = () => screen.getAllByRole("button").map((button) => button.textContent);

describe("RequestMerge", () => {
  it("draws a red request as a trouble Notice that names each failed check and links the ones with a page", async () => {
    render(request(RED));

    const line = screen.getByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-red:this project");
    expect(line.textContent).toContain(
      "The pull request #12 of this project is red, so purlis did not merge it: 2 checks failed: lint, e2e. It stays open, and the next save goes onto it.",
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

    // Its two answers, then the request's own page.
    expect(buttons()).toEqual(["Fix in a chat", "Merge as it stands…", "Open the pull request"]);
    await userEvent.click(screen.getByRole("button", { name: "Open the pull request" }));
    expect(opened).toEqual([PR]);
  });

  it("offers each state its own answers and no others", () => {
    const offered = (merge: SaveMerge) => {
      render(request(merge));
      const names = screen.queryAllByRole("button").map((button) => button.textContent);
      cleanup();
      return names;
    };
    // A wait nothing purlis reads will end: the person's merge, and no Fix (nothing failed).
    expect(offered(merge("waiting", "no check has started on it", "person"))).toEqual([
      "Merge as it stands…",
      "Open the pull request",
    ]);
    // A look with no sign-in: the sign-in, and never the merge.
    expect(offered(merge("waiting", "purlis has no sign-in to GitHub", "sign-in"))).toEqual([
      "Sign in…",
      "Open the pull request",
    ]);
    // What the forge will not merge: why and the link, and no button that would try.
    expect(offered(merge("cannot-merge", "#12 conflicts with its base branch"))).toEqual([
      "Open the pull request",
    ]);
    // A wait that ends by itself, a queued request and a merged one are notes with no button.
    for (const state of ["waiting", "queued", "merged"]) expect(offered(merge(state))).toEqual([]);
  });

  it("looks afresh before it asks, and the question shows what the look found, not what was kept", async () => {
    const sent = commandsSent({
      look_at_save_request: { head: HEAD, merge: RED_NOW },
      merge_save_request: "Merged as it stands.",
    });
    const saved = vi.fn();
    window.addEventListener(PLANE_SAVED, saved);
    render(request(RED));

    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    // The press that asks looks, and merges nothing.
    const asking = await screen.findByRole("alertdialog");
    expect(sent).toEqual([
      { cmd: "look_at_save_request", args: { plane: PLANE, workspace: null, repo: null } },
    ]);
    expect(asking.textContent).toContain(
      "Merge the pull request #12 of this project as it stands?",
    );
    expect(asking.textContent).toContain(
      "purlis merges it into main now, without waiting for its checks. 3 checks failed: lint, e2e, build.",
    );
    expect(asking.textContent).toContain("There is no undo.");
    expect(
      within(within(asking).getByRole("list", { name: "Checks that failed" }))
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["lint", "e2e", "build"]);
    // Focus starts on the way back.
    expect(within(asking).getByRole("button", { name: "Cancel" })).toHaveFocus();

    await userEvent.click(within(asking).getByRole("button", { name: "Merge as it stands" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    // The merge carries the commit and the standing the question showed, once.
    expect(sent.filter((one) => one.cmd === "merge_save_request")).toEqual([
      {
        cmd: "merge_save_request",
        args: { plane: PLANE, workspace: null, repo: null, head: HEAD, shown: RED_NOW.shown },
      },
    ]);
    expect(await screen.findByText("Merged as it stands.")).toBeTruthy();
    expect(saved).toHaveBeenCalled();
    window.removeEventListener(PLANE_SAVED, saved);
  });

  it("asks nothing when the fresh look finds nothing left to merge past", async () => {
    // It merged by itself, or closed: no question, and the standing is read again.
    const sent = commandsSent({ look_at_save_request: { head: null, merge: null } });
    const saved = vi.fn();
    window.addEventListener(PLANE_SAVED, saved);
    const { unmount } = render(request(RED));
    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    expect(
      await screen.findByText(
        "The pull request #12 is no longer open for purlis to merge: it merged or was closed.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(sent.map((one) => one.cmd)).toEqual(["look_at_save_request"]);
    expect(saved).toHaveBeenCalled();
    unmount();

    // Its checks are running again: a wait that ends by itself is not asked about.
    commandsSent({
      look_at_save_request: { head: HEAD, merge: merge("waiting", "its checks are still running") },
    });
    render(request(RED));
    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    expect(
      await screen.findByText(
        "The pull request #12 changed since purlis last looked: its checks are still running.",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    window.removeEventListener(PLANE_SAVED, saved);
  });

  it("holds the question back while its project is behind another, and has it there about the same request on return", async () => {
    const sent = commandsSent({
      look_at_save_request: { head: HEAD, merge: RED_NOW },
      merge_save_request: "Merged as it stands.",
    });
    const under = (shown: boolean) => (
      <ProjectShownProvider value={shown}>{request(RED)}</ProjectShownProvider>
    );
    const { rerender } = render(under(true));
    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    expect(await screen.findByRole("alertdialog")).toBeTruthy();

    // Another project comes to the front: nothing of this one is drawn over it.
    rerender(under(false));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(document.querySelector('[data-scope="dialog"][data-part="scrim"]')).toBeNull();
    expect(sent.filter((one) => one.cmd === "merge_save_request")).toEqual([]);

    // Back in front: the same question, about what the look found, and its answer carries
    // the commit and the standing it showed.
    rerender(under(true));
    const asking = await screen.findByRole("alertdialog");
    expect(asking.textContent).toContain(
      "Merge the pull request #12 of this project as it stands?",
    );
    expect(asking.textContent).toContain("3 checks failed: lint, e2e, build.");
    await userEvent.click(within(asking).getByRole("button", { name: "Merge as it stands" }));
    expect(sent.filter((one) => one.cmd === "merge_save_request")).toEqual([
      {
        cmd: "merge_save_request",
        args: { plane: PLANE, workspace: null, repo: null, head: HEAD, shown: RED_NOW.shown },
      },
    ]);
  });

  it("says why it could not look, and asks nothing", async () => {
    const sent = commandsSent({ look_at_save_request: new Error("the forge did not answer") });
    render(request(RED));
    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    expect(
      await screen.findByText(
        "purlis could not look at the pull request #12: the forge did not answer",
      ),
    ).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(sent.map((one) => one.cmd)).toEqual(["look_at_save_request"]);
  });

  it("merges nothing when the question is cancelled", async () => {
    const sent = commandsSent({ look_at_save_request: { head: HEAD, merge: RED } });
    render(request(RED));
    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(sent.map((one) => one.cmd)).toEqual(["look_at_save_request"]);
  });

  it("says a refusal in the core's words beside the link, and has the standing read again", async () => {
    const refusal =
      "The request changed since you looked (1 check failed: lint), so purlis did not merge it. Look at it again";
    const waits = merge("waiting", "purlis could not read its checks: HTTP 502", "person");
    const sent = commandsSent({
      look_at_save_request: { head: HEAD, merge: waits },
      merge_save_request: new Error(refusal),
    });
    const saved = vi.fn();
    window.addEventListener(PLANE_SAVED, saved);
    render(
      request(merge("waiting", "no check has started on it", "person"), {
        workspace: "alpha",
        repo: "widget",
        name: "widget",
        target: null,
      }),
    );

    await userEvent.click(screen.getByRole("button", { name: "Merge as it stands…" }));
    const asking = await screen.findByRole("alertdialog");
    // A wait has no failed check to name; the question says what the fresh look could not
    // read, and a repo whose target purlis cannot tell says so.
    expect(asking.textContent).toContain(
      "purlis merges it into its target branch now, without waiting for its checks. No check has passed on it: purlis could not read its checks: HTTP 502.",
    );
    expect(within(asking).queryByRole("list")).toBeNull();
    await userEvent.click(within(asking).getByRole("button", { name: "Merge as it stands" }));

    expect(sent.at(-1)).toEqual({
      cmd: "merge_save_request",
      args: { plane: PLANE, workspace: "alpha", repo: "widget", head: HEAD, shown: waits.shown },
    });
    const line = await screen.findByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-waits:widget");
    expect(line.textContent).toContain(`purlis did not merge the pull request #12: ${refusal}`);
    expect(screen.getByRole("button", { name: "Open the pull request" })).toBeTruthy();
    expect(saved).toHaveBeenCalled();
    window.removeEventListener(PLANE_SAVED, saved);
  });

  it("says a repo's fix needs a save where the repo is not saved by itself", () => {
    const repos = { workspace: "alpha", repo: "widget", name: "widget" };
    render(request(RED, { ...repos, needsSave: true }));
    expect(screen.getByRole("status").textContent).toContain(
      "It stays open, and the next save of widget goes onto it: a fix made in its clone reaches the pull request when you save the repo.",
    );
    cleanup();
    render(request(RED, repos));
    expect(screen.getByRole("status").textContent).not.toContain("when you save the repo");
  });

  it("starts one Fix chat for a press, and tells the project's window which", async () => {
    const chat: Fixing = {
      session: 7,
      name: "7",
      label: "Fix the save",
      persona: null,
      harness: "claude",
      workspace: null,
      existing: false,
    };
    const sent = commandsSent({ fix_save_request: chat });
    const started: FixStarted[] = [];
    const hear = (event: Event) => started.push((event as CustomEvent<FixStarted>).detail);
    window.addEventListener(FIX_STARTED, hear);
    render(request(RED));

    await userEvent.click(screen.getByRole("button", { name: "Fix in a chat" }));

    expect(sent).toEqual([
      {
        cmd: "fix_save_request",
        args: { plane: PLANE, workspace: null, repo: null, columns: 80, rows: 24 },
      },
    ]);
    await vi.waitFor(() => expect(started).toEqual([{ plane: PLANE, chat }]));
    window.removeEventListener(FIX_STARTED, hear);
  });

  it("says why no Fix chat started, and keeps both answers", async () => {
    commandsSent({ fix_save_request: new Error("This project has no harness profile.") });
    render(request(RED));
    await userEvent.click(screen.getByRole("button", { name: "Fix in a chat" }));
    expect(
      await screen.findByText(
        "purlis could not start a chat to fix it: This project has no harness profile.",
      ),
    ).toBeTruthy();
    expect(buttons()).toEqual(["Fix in a chat", "Merge as it stands…", "Open the pull request"]);
  });

  it("draws a request the forge will not merge as a Notice with why", () => {
    render(
      request(merge("cannot-merge", "!3 conflicts with its target branch"), {
        name: "widget",
        request: "merge request",
        pr: "https://gitlab.com/acme/widget/-/merge_requests/3",
      }),
    );
    const line = screen.getByRole("status");
    expect(line).toHaveAttribute("data-cause", "save-request-cannot-merge:widget");
    expect(line.textContent).toContain(
      "purlis cannot merge the merge request #12 of widget as it stands: !3 conflicts with its target branch. It stays open.",
    );
    expect(screen.getByRole("button", { name: "Open the merge request" })).toBeTruthy();
  });

  it("says a waiting request in a note, which is not a Notice: nothing is wrong", () => {
    const { container } = render(request(merge("waiting")));
    expect(screen.queryByRole("status")).toBeNull();
    const note = container.querySelector('[data-merge="waiting"]');
    expect(note?.textContent).toBe(
      "purlis merges the pull request of this project itself once every check on it has passed: its checks are still running. It looks again by itself.",
    );
  });

  it("says a wait with no bound, and one for want of a sign-in, where the title bar's mark reads them", () => {
    expect(
      requestWords({
        request: "pull request",
        merge: merge("waiting", "no check has started on it", "person"),
      }),
    ).toBe(
      "Pushed — its pull request waits, and nothing says for how long: no check has started on it",
    );
    expect(
      requestWords({ request: "merge request", merge: merge("waiting", "no sign-in", "sign-in") }),
    ).toBe("Pushed — purlis needs a sign-in to look at its merge request");
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
    // With its two answers, here as in the Inbox.
    expect(screen.getByRole("button", { name: "Fix in a chat" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Merge as it stands…" })).toBeTruthy();
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
