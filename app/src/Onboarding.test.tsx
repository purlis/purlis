// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { GitPresence, OnboardingProgress } from "./bindings";

/**
 * **The guided steps, in the window** (#1817, spec #1810): welcome → sign in (when taken) →
 * create, get or open → the git check (when git is missing) → pick your harness → the project.
 *
 * Against a stood-in core (`mockIPC`): the core's halves — the kept place
 * (`purlis_core::onboarding`), the git check (#1813) and the repo taken in (FR-4) — are tested
 * in Rust. What is pinned here is what the person meets: "Step N of M" at every step, the
 * harness step holding the new project behind it, Skip where it makes sense, the keyboard on
 * each step's heading, and a relaunch at any step picking up there.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

const LOCAL = "/home/dev/.local/share/purlis/local-project";
const REPO = "/home/dev/widget";
const CLONE = `${LOCAL}/workspaces/widget/widget`;
const TEAM = "/home/dev/Projects/team";

const FRESH: OnboardingProgress = {
  step: "welcome",
  door: null,
  signing_in: false,
  git_missing: false,
  made: null,
  finished_once: false,
};

const SIDEBAR = {
  root: LOCAL,
  workspaces: [
    { name: "widget", path: `${LOCAL}/workspaces/widget`, vision: "", todos: [], chats: [] },
  ],
  personas: ["steward"],
  persona: "steward",
  unfiled: [],
};

const HARNESSES = {
  harnesses: [
    {
      name: "claude",
      title: "Claude Code",
      installed: false,
      signed_in: false,
      installer: "curl -fsSL https://claude.ai/install.sh | bash",
      installer_page: "https://docs.claude.com",
    },
  ],
  local_models: [],
};

const FOUND = {
  harnesses: [{ name: "claude", title: "Claude Code", installed: false, signed_in: false }],
  forges: [{ cli: "gh", forge: "github", title: "GitHub", installed: false, signed_in: false }],
  templates: [],
};

/**
 * A machine whose kept place is `kept` and whose git `git()` says, with every call and every
 * place the window kept recorded.
 */
function machine({
  kept = null,
  git = () => ({ kind: "present" }),
  answers = () => undefined,
}: {
  kept?: OnboardingProgress | null;
  git?: () => GitPresence;
  answers?: (cmd: string, args: Record<string, unknown>) => unknown;
} = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: given });
    const answer = answers(cmd, given);
    if (answer !== undefined) return answer;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "onboarding_progress") return kept;
    if (cmd === "keep_onboarding_progress") return null;
    if (cmd === "git_presence") return git();
    if (cmd === "first_run_found") return FOUND;
    if (cmd === "harness_setup_found") return HARNESSES;
    if (cmd === "open_repo")
      return {
        opened: {
          opened: { plane: LOCAL, ask: null },
          workspace: "widget",
          cwd: CLONE,
          harness: null,
          none_installed: true,
          instructions: 0,
        },
        asks_forge: null,
      };
    if (cmd === "open_plane") return { plane: given.path, ask: null };
    if (cmd === "plane_sidebar") return SIDEBAR;
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    return null;
  });
  const calls = (cmd: string) => asked.filter((one) => one.cmd === cmd);
  /** The last place the window kept. */
  const lastKept = () =>
    calls("keep_onboarding_progress").at(-1)?.args.progress as OnboardingProgress | undefined;
  return { calls, lastKept };
}

/** "Step N of M: name", as the steps say where the person is. */
async function stepSays(said: string) {
  const steps = await screen.findByRole("navigation", { name: "Setting up" });
  await waitFor(() => expect(steps).toHaveTextContent(said));
  return steps;
}

async function openRepoByPath(path: string) {
  const person = userEvent.setup();
  await person.type(await screen.findByLabelText("Or type the repo's path"), path);
  await person.click(screen.getByRole("button", { name: "Open repo" }));
  return person;
}

/** Whether the project is drawn: its pinned workspaces are in the title bar (#2916). */
function projectDrawn(): boolean {
  return document.querySelector('[role="toolbar"][aria-label="Pinned workspaces"]') !== null;
}

describe("a fresh machine", () => {
  it("goes through every step to an open project, saying where it is at each", async () => {
    const { calls, lastKept } = machine();
    render(<App />);

    await stepSays("Step 1 of 2: Pick a project");
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await stepSays("Step 1 of 2: Pick a project");
    expect(screen.getByRole("heading", { level: 1, name: "Create a project" })).toHaveFocus();

    await openRepoByPath(REPO);
    const harness = await screen.findByRole("heading", { level: 1, name: "Pick your harness" });
    await stepSays("Step 2 of 2: Pick your harness");
    expect(harness).toHaveFocus();
    // The project is made and held behind the step, not drawn.
    expect(projectDrawn()).toBe(false);
    // The Set up a harness tab's own rows and installer, about the new project.
    expect(screen.getByRole("button", { name: "Install Claude Code" })).toBeInTheDocument();
    await waitFor(() =>
      expect(lastKept()).toMatchObject({
        step: "harness",
        made: { plane: LOCAL, cwd: CLONE, workspace: "widget" },
      }),
    );

    // Install is the tab's own ask of the project, and it ends the steps: the installer runs in
    // a shell tab the person may have to answer.
    await person.click(screen.getByRole("button", { name: "Install Claude Code" }));
    await waitFor(() => expect(projectDrawn()).toBe(true));
    expect(screen.queryByRole("navigation", { name: "Setting up" })).toBeNull();
    await waitFor(() => expect(lastKept()).toEqual({ ...FRESH, finished_once: true }));
    // Nothing secret is ever kept: only steps, doors and paths.
    for (const one of calls("keep_onboarding_progress"))
      expect(Object.keys(one.args.progress as object).sort()).toEqual(
        ["door", "finished_once", "git_missing", "made", "signing_in", "step"].sort(),
      );
  });

  it("opens the project when the harness step is skipped", async () => {
    machine();
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await openRepoByPath(REPO);
    await screen.findByRole("heading", { level: 1, name: "Pick your harness" });

    await person.click(screen.getByRole("button", { name: "Skip for now" }));
    await waitFor(() => expect(projectDrawn()).toBe(true));
  });

  it("counts the sign-in when the person takes it, and lets them skip it", async () => {
    const { lastKept } = machine();
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Sign in to GitHub or GitLab" }));
    await stepSays("Step 1 of 3: Sign in");
    expect(
      screen.getByRole("heading", { level: 1, name: "Sign in to GitHub or GitLab" }),
    ).toHaveFocus();

    await person.click(screen.getByRole("button", { name: "Skip for now" }));
    await stepSays("Step 2 of 3: Pick a project");
    // The doors again, for the project.
    expect(screen.getByRole("button", { name: "Get a project from your forge" })).toBeVisible();
    await waitFor(() =>
      expect(lastKept()).toMatchObject({ step: "project", door: null, signing_in: true }),
    );
  });

  it("adds the sign-in step when Create on GitHub finds no account, then comes back to Create", async () => {
    // #1819: GitHub picked on Create with nobody signed in to it.
    const { lastKept } = machine({
      answers: (cmd) =>
        cmd === "forge_owners" ? { host: null, login: "", owners: [] } : undefined,
    });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await stepSays("Step 1 of 2: Pick a project");
    await person.click(screen.getByRole("radio", { name: "GitHub" }));

    await person.click(await screen.findByRole("button", { name: "Sign in to GitHub" }));
    await screen.findByRole("heading", { level: 1, name: "Sign in to GitHub or GitLab" });
    await stepSays("Step 1 of 3: Sign in");
    await waitFor(() =>
      expect(lastKept()).toMatchObject({ step: "sign-in", door: "create", signing_in: true }),
    );

    // Skip for now (or Continue once signed in) is Create again, with GitHub still picked.
    await person.click(screen.getByRole("button", { name: "Skip for now" }));
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    await stepSays("Step 2 of 3: Pick a project");
    expect(screen.getByRole("radio", { name: "GitHub" })).toBeChecked();
  });

  it("is reachable by the keyboard alone, from the welcome to the project", async () => {
    machine();
    render(<App />);
    const person = userEvent.setup();
    const create = await screen.findByRole("button", { name: "Create a project" });
    create.focus();
    await person.keyboard("{Enter}");
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    await person.type(screen.getByLabelText("Or type the repo's path"), `${REPO}{Enter}`);
    await screen.findByRole("heading", { level: 1, name: "Pick your harness" });
    // Tab from the heading reaches Skip for now.
    for (let presses = 0; presses < 12; presses += 1) {
      if (document.activeElement?.textContent === "Skip for now") break;
      await person.tab();
    }
    expect(document.activeElement).toHaveTextContent("Skip for now");
    await person.keyboard("{Enter}");
    await waitFor(() => expect(projectDrawn()).toBe(true));
  });
});

describe("a machine with no git (#1813's guided screen, mounted)", () => {
  it("shows the git check as a step, then carries on with the act once git is there", async () => {
    let git: GitPresence = { kind: "missing", install: { kind: "developer-tools" } };
    const { calls } = machine({ git: () => git });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await openRepoByPath(REPO);

    await screen.findByRole("heading", { level: 1, name: "Install git" });
    await stepSays("Step 2 of 3: Install git");
    expect(screen.getByRole("heading", { name: "Git is not installed" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install" })).toBeInTheDocument();
    // Nothing that needs git ran.
    expect(calls("open_repo")).toHaveLength(0);

    git = { kind: "present" };
    await person.click(screen.getByRole("button", { name: "Check again" }));
    await screen.findByRole("heading", { level: 1, name: "Pick your harness" });
    expect(calls("open_repo")).toHaveLength(1);
    // The git check happened, so it stays counted.
    await stepSays("Step 3 of 3: Pick your harness");
  });

  it("holds Get's clone behind the git check, and clones in its own page once git is there", async () => {
    let git: GitPresence = { kind: "missing", install: { kind: "developer-tools" } };
    const { calls } = machine({
      git: () => git,
      answers: (cmd) => {
        if (cmd === "project_url_folder")
          return { name: "team", folder: "/home/dev/Projects/team" };
        if (cmd === "get_project") return "/home/dev/Projects/team";
        return undefined;
      },
    });
    render(<App />);
    const person = userEvent.setup();
    await person.click(
      await screen.findByRole("button", { name: "Get a project from your forge" }),
    );
    await person.type(
      await screen.findByLabelText("Project URL"),
      "https://github.com/acme/team.git",
    );
    await person.click(screen.getByRole("button", { name: "Get project" }));

    await screen.findByRole("heading", { level: 1, name: "Install git" });
    expect(calls("get_project")).toHaveLength(0);

    git = { kind: "present" };
    await person.click(screen.getByRole("button", { name: "Check again" }));
    // The page it was asked from is still there, URL and all, and the clone runs from it; the
    // project it got arrives behind the harness step.
    await screen.findByRole("heading", { level: 1, name: "Pick your harness" });
    expect(calls("get_project").map((one) => one.args.url)).toEqual([
      "https://github.com/acme/team.git",
    ]);
    expect(calls("open_plane").map((one) => one.args.path)).toEqual(["/home/dev/Projects/team"]);
  });

  it("keeps the window's lines on the launcher in sight, through the git check (#4601)", async () => {
    machine({ git: () => ({ kind: "missing", install: { kind: "developer-tools" } }) });
    render(<App />);
    const person = userEvent.setup();
    const linesInSight = () => {
      const places = [...document.querySelectorAll(".window-notices-at")];
      expect(places).toHaveLength(1);
      expect(places[0].closest("[hidden]")).toBeNull();
    };
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await openRepoByPath(REPO);

    await screen.findByRole("heading", { level: 1, name: "Install git" });
    await waitFor(linesInSight);

    await person.click(screen.getByRole("button", { name: "Not now" }));
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    await waitFor(linesInSight);
  });

  it("goes back to the project's page on Not now, with nothing run", async () => {
    const { calls } = machine({
      git: () => ({ kind: "missing", install: { kind: "download", page: "https://git-scm.com" } }),
    });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await openRepoByPath(REPO);
    await screen.findByRole("heading", { level: 1, name: "Install git" });

    await person.click(screen.getByRole("button", { name: "Not now" }));
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    expect(calls("open_repo")).toHaveLength(0);
  });
});

describe("a relaunch", () => {
  it("picks up at the door's page it was quit on", async () => {
    machine({ kept: { ...FRESH, step: "project", door: "get" } });
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Get a project from your forge" });
    await stepSays("Step 1 of 2: Pick a project");
  });

  it("picks up at the sign-in it was quit on", async () => {
    machine({ kept: { ...FRESH, step: "sign-in", signing_in: true } });
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Sign in to GitHub or GitLab" });
    await stepSays("Step 1 of 3: Sign in");
  });

  it("picks up at the git check, and looks for git again", async () => {
    machine({
      kept: { ...FRESH, step: "git", door: "create", git_missing: true },
      git: () => ({ kind: "missing", install: { kind: "developer-tools" } }),
    });
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Install git" });
    await screen.findByRole("heading", { name: "Git is not installed" });
    await stepSays("Step 2 of 3: Install git");
  });

  it("goes on from the git check to the project's page when git is there now", async () => {
    machine({ kept: { ...FRESH, step: "git", door: "create", git_missing: true } });
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    await stepSays("Step 1 of 3: Pick a project");
  });

  it("picks up at the harness step, opening its project again and holding it", async () => {
    const { calls } = machine({
      kept: {
        ...FRESH,
        step: "harness",
        door: "open",
        made: { plane: TEAM, cwd: TEAM, workspace: "outside/every/workspace" },
      },
    });
    render(<App />);
    await screen.findByRole("heading", { level: 1, name: "Pick your harness" });
    await waitFor(() => expect(calls("open_plane").map((one) => one.args.path)).toContain(TEAM));
    await screen.findByRole("button", { name: "Install Claude Code" });
    expect(projectDrawn()).toBe(false);
    await stepSays("Step 2 of 2: Pick your harness");
  });

  it("shows no harness step after the first time through while a harness is installed", async () => {
    machine({
      kept: { ...FRESH, finished_once: true },
      answers: (cmd) =>
        cmd === "harness_setup_found"
          ? { ...HARNESSES, harnesses: [{ ...HARNESSES.harnesses[0], installed: true }] }
          : undefined,
    });
    render(<App />);
    // Someone back on a machine that has been through the steps is counted through nothing.
    await screen.findByRole("button", { name: "Create a project" });
    expect(screen.queryByRole("navigation", { name: "Setting up" })).not.toBeInTheDocument();
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await openRepoByPath(REPO);
    await waitFor(() => expect(projectDrawn()).toBe(true));
  });
});

describe("the place it keeps", () => {
  it("is the last place the person was at, however slowly the core keeps an earlier one", async () => {
    // A core that keeps each place as its write finishes, and whose first write is slow: the
    // window's next place must not be put under it.
    let onDisk: OnboardingProgress | null = null;
    let finishFirst = () => {};
    let writes = 0;
    machine({
      answers: (cmd, args) => {
        if (cmd !== "keep_onboarding_progress") return undefined;
        const progress = args.progress as OnboardingProgress;
        writes += 1;
        if (writes > 1) {
          onDisk = progress;
          return null;
        }
        return new Promise((resolve) => {
          finishFirst = () => {
            onDisk = progress;
            resolve(null);
          };
        });
      },
    });
    render(<App />);
    await stepSays("Step 1 of 2: Pick a project");
    await waitFor(() => expect(writes).toBe(1));

    const person = userEvent.setup();
    await person.click(
      await screen.findByRole("button", { name: "Get a project from your forge" }),
    );
    await screen.findByRole("heading", { level: 1, name: "Get a project from your forge" });
    finishFirst();

    await waitFor(() => expect(onDisk).toMatchObject({ step: "project", door: "get" }));
  });
});

describe("the kept place, read before a door can be pressed (#2400)", () => {
  /** A machine whose kept place is `kept`, answered only when `answer` is called, with a project
   *  open at `launch` when one is given. */
  function aSlowRead({ launch, kept }: { launch?: string; kept: OnboardingProgress }) {
    let answer = () => {};
    const core = machine({
      answers: (cmd) => {
        if (cmd === "plane_at_launch" && launch !== undefined)
          return { plane: launch, from: launch, why: null };
        if (cmd === "onboarding_progress")
          return new Promise((resolve) => {
            answer = () => resolve(kept);
          });
        return undefined;
      },
    });
    return { ...core, answer: () => answer() };
  }

  it("keeps the Create page a door opened on Add another project, however late the read answers", async () => {
    // The window holds a project, and this machine finished the steps: the plain welcome. Drawn
    // before the read answered, the guided one was swapped for it under the press, and the page
    // the door opened was gone.
    const read = aSlowRead({ launch: TEAM, kept: { ...FRESH, finished_once: true } });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Open a project…" }));
    // The read is asked and has not answered: no welcome to press yet.
    await waitFor(() => expect(read.calls("onboarding_progress").length).toBeGreaterThan(0));
    expect(screen.queryByRole("list", { name: "Ways to start" })).toBeNull();

    read.answer();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await screen.findByRole("heading", { level: 1, name: "Create a project" });
    expect(screen.getByRole("radiogroup", { name: "Where it lives" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Setting up" })).toBeNull();
  });

  it("draws the welcome of a read that never answers by the deadline, and keeps a press from a late answer", async () => {
    const read = aSlowRead({ launch: TEAM, kept: { ...FRESH, finished_once: true } });
    render(<App />);
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Open a project…" }));
    const asked = Date.now();
    await person.click(
      await screen.findByRole("button", { name: "Create a project" }, { timeout: 2_000 }),
    );
    expect(Date.now() - asked).toBeLessThan(2_000);
    await screen.findByRole("heading", { level: 1, name: "Create a project" });

    // The read answers long after: the place takes it (and keeps it), the page stays.
    read.answer();
    await waitFor(() => expect(read.lastKept()).toMatchObject({ finished_once: true }));
    expect(screen.getByRole("heading", { level: 1, name: "Create a project" })).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: "Where it lives" })).toBeInTheDocument();
  });

  it("guides a window with no project at once, without waiting on the read", async () => {
    const read = aSlowRead({ kept: FRESH });
    render(<App />);
    await stepSays("Step 1 of 2: Pick a project");
    const person = userEvent.setup();
    await person.click(await screen.findByRole("button", { name: "Create a project" }));
    await screen.findByRole("heading", { level: 1, name: "Create a project" });

    read.answer();
    await stepSays("Step 1 of 2: Pick a project");
    expect(screen.getByRole("radiogroup", { name: "Where it lives" })).toBeInTheDocument();
  });
});
