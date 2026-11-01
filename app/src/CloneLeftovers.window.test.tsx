// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { findOnCard } from "./actionResult.testkit";
import { cloneRepos, forgetRepoClones } from "./repoClones";
import { forgetThisLaunch } from "./regions";
import {
  chipName,
  showTheChanges,
  showTheExplorer,
  workspaceChips,
  workspaceInFront,
} from "./test-strips";

/**
 * **What a clone leaves on the window once it has ended**: the row in the Repos view and the
 * notices about it, against the whole app and a fake core of several workspaces, because each
 * claim is about what the person is left looking at after the clone is over.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";

type Held = { land: () => void };

/**
 * The core: each workspace in `named` names those repos, none of them cloned, and alpha and beta
 * are on the title bar. A clone that works moves the repo among the clones, which is what the next
 * `workspace_panels` reads. `fails` answers that repo's next N clones with the core's sentence, and
 * a repo in `holds` waits for the test to land it or for Cancel.
 */
function core(
  over: {
    named?: Record<string, string[]>;
    fails?: Record<string, { said: string; times: number }>;
    holds?: string[];
  } = {},
) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  const planes = new Map<string, { cloned: string[]; missing: string[] }>();
  const of = (workspace: unknown) => {
    const name = String(workspace);
    const held = planes.get(name) ?? { cloned: [], missing: [...(over.named?.[name] ?? [])] };
    planes.set(name, held);
    return held;
  };
  const fails = { ...over.fails };
  const held = new Map<string, Held & { stop: () => void }>();
  const made: string[] = [];
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: a });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [];
      if (cmd === "plane_pins")
        return { project: false, workspaces: ["alpha", "beta"], missing: [] };
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: ["steward"],
          persona: "steward",
          unfiled: [],
          workspaces: [...new Set(["alpha", "beta", ...made])].map((name) => ({
            name,
            path: `${PLANE}/workspaces/${name}`,
            vision: "",
            todos: [],
            chats: [],
          })),
        };
      if (cmd === "workspace_panels") {
        const { cloned, missing } = of(a.workspace);
        return {
          workspace: a.workspace,
          repos: [...cloned],
          paths: Object.fromEntries(
            cloned.map((repo) => [repo, `${PLANE}/workspaces/${String(a.workspace)}/${repo}`]),
          ),
          absent: [...missing],
          refused: [],
          todos: [],
          todos_refused: null,
          personas: ["steward"],
          persona: "steward",
          sessions: [],
          contributed: [],
        };
      }
      if (cmd === "workspace_repos")
        return { workspace: a.workspace, repos: [], cache_refused: null };
      if (cmd === "worktree_list") return [];
      if (cmd === "take_repos") return null;
      if (cmd === "reachable_repos")
        return {
          repos: [
            { name: "api", path: "acme/api", description: "" },
            { name: "web", path: "acme/web", description: "" },
          ],
          trouble: [],
        };
      if (cmd === "workspace_create") {
        made.push(String(a.name));
        return [`✓ Workspace '${String(a.name)}' ready (LOCAL) → workspaces/${String(a.name)}/`];
      }
      if (cmd === "cancel_clone_repo") {
        held.get(String(a.repo))?.stop();
        return null;
      }
      if (cmd === "clone_repo") {
        const repo = String(a.repo);
        if (over.holds?.includes(repo)) {
          const stopped = await new Promise<boolean>((end) => {
            held.set(repo, { land: () => end(false), stop: () => end(true) });
          });
          if (stopped) return null;
        }
        const failing = fails[repo];
        if (failing && failing.times > 0) {
          failing.times -= 1;
          throw failing.said;
        }
        const { cloned, missing } = of(a.workspace);
        if (missing.includes(repo)) missing.splice(missing.indexOf(repo), 1);
        if (!cloned.includes(repo)) cloned.push(repo);
        return [`✓ ${repo} → workspaces/${String(a.workspace)}/${repo}`];
      }
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    /** The repo's folder and its line in the manifest went: it is neither cloned nor named. */
    dropped: async (workspace: string, repo: string) => {
      const { cloned } = of(workspace);
      cloned.splice(cloned.indexOf(repo), 1);
      await act(async () => {
        await emit("plane-changed", { plane: PLANE });
      });
    },
    clones: () => asked.filter((one) => one.cmd === "clone_repo").map((one) => one.args.repo),
    held: (repo: string) => held.get(repo),
  };
}

beforeEach(() => {
  globalThis.localStorage.clear();
  forgetThisLaunch();
});
afterEach(() => {
  cleanup();
  clearMocks();
  forgetRepoClones();
});

const chipOf = (workspace: string) => {
  const chip = workspaceChips().find((one) => chipName(one) === workspace);
  if (chip === undefined) throw new Error(`no ${workspace} in the title bar`);
  return chip;
};

/** Puts `workspace` in front by its chip, and shows its Repos and branches. */
const focus = async (workspace: string) => {
  await waitFor(() => chipOf(workspace));
  await userEvent.click(chipOf(workspace));
  await waitFor(() => expect(workspaceInFront()).toBe(workspace));
  return showTheExplorer();
};

describe("a repo that landed while its workspace was not focused", () => {
  it("leaves no row when it is dropped before the workspace is focused", async () => {
    const { dropped } = core({ named: { alpha: ["svc"] } });
    render(<App />);
    await focus("alpha");

    await act(async () => {
      await cloneRepos(PLANE, "beta", ["newcomer"]);
    });
    await dropped("beta", "newcomer");
    await focus("beta");

    expect(await screen.findAllByText("No repos in this workspace")).not.toEqual([]);
    expect(screen.queryByTestId("absent-newcomer")).toBeNull();
    expect(screen.queryByText("Cloned newcomer.")).toBeNull();
  });

  it("is among the clones when the workspace is focused, as ever", async () => {
    core({ named: { alpha: ["svc"] } });
    render(<App />);
    await focus("alpha");

    await act(async () => {
      await cloneRepos(PLANE, "beta", ["newcomer"]);
    });
    await focus("beta");

    expect(await screen.findByTestId("clone-newcomer")).toBeInTheDocument();
    expect(screen.queryByTestId("absent-newcomer")).toBeNull();
  });
});

/** Makes the workspace gamma in its dialog, with `repos` picked. */
async function makeGamma(repos: string[]) {
  await waitFor(() => chipOf("alpha"));
  fireEvent.contextMenu(chipOf("alpha"));
  await userEvent.click(await screen.findByRole("menuitem", { name: "New workspace…" }));
  const dialog = await screen.findByRole("dialog");
  await userEvent.type(within(dialog).getByLabelText("Name"), "gamma");
  for (const repo of repos)
    await userEvent.click(await within(dialog).findByRole("checkbox", { name: repo }));
  await userEvent.click(within(dialog).getByRole("button", { name: "Create workspace" }));
  await waitFor(() => expect(workspaceInFront()).toBe("gamma"));
  await showTheExplorer();
}

const NO_ACCESS = "api: clone failed — no access.";

describe("a new workspace's repo whose clone failed", () => {
  it("keeps a row saying why, with a Retry that clones it", async () => {
    const { clones } = core({ fails: { api: { said: NO_ACCESS, times: 1 } } });
    render(<App />);

    await makeGamma(["api"]);

    const row = await screen.findByTestId("absent-api");
    expect(await within(row).findByRole("alert")).toHaveTextContent(NO_ACCESS);
    expect(screen.queryByText("No repos in this workspace")).toBeNull();
    await userEvent.click(await within(row).findByRole("button", { name: "Retry api" }));

    expect(await screen.findByTestId("clone-api")).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByTestId("absent-api")).toBeNull());
    expect(clones()).toEqual(["api", "api"]);
  });

  it("says so on its row in the Changes view too", async () => {
    core({ fails: { api: { said: NO_ACCESS, times: 1 } } });
    render(<App />);

    await makeGamma(["api"]);

    const bar = await showTheChanges();
    expect(await within(bar).findByTestId("repo-api")).toHaveTextContent(NO_ACCESS);
  });

  it("is offered no Remove, since the workspace does not name it, and Dismiss clears the row", async () => {
    core({ fails: { api: { said: NO_ACCESS, times: 1 } } });
    render(<App />);

    await makeGamma(["api"]);

    const row = await screen.findByTestId("absent-api");
    expect(within(row).queryByRole("button", { name: /^Remove/ })).toBeNull();
    await userEvent.click(await within(row).findByRole("button", { name: "Dismiss api" }));

    await waitFor(() => expect(screen.queryByTestId("absent-api")).toBeNull());
    expect(await screen.findAllByText("No repos in this workspace")).not.toEqual([]);
  });

  it("offers Clone and Dismiss on its row's menu", async () => {
    core({ fails: { api: { said: NO_ACCESS, times: 1 } } });
    render(<App />);

    await makeGamma(["api"]);

    fireEvent.contextMenu(within(await screen.findByTestId("absent-api")).getByText("api"));
    const menu = await screen.findByRole("menu");
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.getAttribute("aria-label")),
    ).toEqual(["Clone", "Dismiss"]);
  });

  it("does not come back in a workspace made again under the same name", async () => {
    core({ fails: { api: { said: NO_ACCESS, times: 1 } } });
    render(<App />);
    await makeGamma(["api"]);
    await screen.findByTestId("absent-api");

    await focus("alpha");
    await makeGamma([]);

    await waitFor(() => expect(screen.queryByTestId("absent-api")).toBeNull());
  });
});

describe("a new workspace's repo the person stopped", () => {
  it("leaves no row, and the chip keeps its Retry", async () => {
    const { clones } = core({ holds: ["api"] });
    render(<App />);

    await makeGamma(["api"]);
    await waitFor(() => expect(clones()).toEqual(["api"]));
    const row = await screen.findByTestId("absent-api");
    await userEvent.click(await within(row).findByRole("button", { name: "Cancel cloning api" }));

    await waitFor(() => expect(screen.queryByTestId("absent-api")).toBeNull());
    expect(await findOnCard("gamma", "Stopped cloning api")).toHaveTextContent("Retry");
  });
});

describe("a workspace that names repos and has none cloned", () => {
  it("does not say it has no repos above the ones not cloned here", async () => {
    core({ named: { alpha: ["charter"] } });
    render(<App />);

    const explorer = await focus("alpha");

    expect(await within(explorer).findByTestId("absent-charter")).toBeInTheDocument();
    expect(within(explorer).queryByText("No repos in this workspace")).toBeNull();
  });
});
