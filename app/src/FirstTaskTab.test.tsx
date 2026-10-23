// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import type { FirstTaskRun } from "./bindings";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { userEvent } from "@testing-library/user-event";
import { FirstTaskTab, type FirstTaskDoes } from "./FirstTaskTab";
import { SETTINGS_ACTION, type SettingsActionAsk } from "./settings/links";
import { hoverTipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * The First task tab on its own (FR-28), for what it reads from the machine and the disk: which
 * harness's program is found (#1698), and which of its chats already started, after a relaunch
 * (#945, in #1091). The tab through the first run is `FirstTask.test.tsx`.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/.local/share/purlis/local-project";
const CLONE = `${PLANE}/workspaces/widget/widget`;

const profile = (name: string, kind: string, isDefault = false) => ({
  name,
  kind,
  shown: name,
  source: "built-in",
  is_default: isDefault,
  approval: null,
  ready_to_type: true,
  harness: null,
  sandbox: null,
});

const START_OPTIONS = {
  profiles: [profile("claude", "claude", true), profile("codex", "codex"), profile("pi", "pi")],
  refused: [],
  personas: ["steward"],
  persona: "steward",
  persona_profiles: {},
  ignore_fix: null,
  declares_none: true,
};

const harness = (name: string, title: string, installed: boolean) => ({
  name,
  title,
  installed,
  signed_in: installed,
  installer: `install ${name}`,
  installer_page: `https://example.invalid/${name}`,
});

/** The machine as the harness setup tab reads it: claude and pi found, codex not. */
const NO_CODEX = {
  harnesses: [
    harness("claude", "Claude Code", true),
    harness("codex", "Codex", false),
    harness("pi", "Pi", true),
  ],
  local_models: [],
};

/** A run the core read back from the clone (`first_task_started`). */
const onDisk = (run: number, diff: string | null = `git add -N -A && git diff c${run} --`) => ({
  run,
  branch: `first-task-${run}`,
  folder: `${PLANE}/workspaces/widget/.worktrees/widget/first-task-${run}`,
  workspace: "widget",
  diff,
  no_diff: diff === null ? "purlis has no record of which branch it was cut from." : null,
});

function machine({
  found = NO_CODEX as unknown,
  started = [] as unknown,
  options = START_OPTIONS as unknown,
}: { found?: unknown; started?: unknown; options?: unknown } = {}) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "start_options") return options;
    if (cmd === "harness_setup_found") return found;
    if (cmd === "first_task_started") return started;
    return null;
  });
  return { calls: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

/** What the plane does for the tab, with no run started this launch. */
const DOES: FirstTaskDoes = {
  start: () => Promise.resolve("not in this test"),
  showDiff: () => {},
  runs: {},
};

const runSection = (name: string) => screen.getByRole("region", { name });

describe("the First task tab, on this machine (#1698)", () => {
  it("never suggests for the second chat a harness whose program is not installed", async () => {
    machine();
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run2 = await screen.findByRole("radiogroup", { name: "Second chat" });
    await waitFor(() => expect(within(run2).getByRole("radio", { name: "pi" })).toBeChecked());
    expect(within(run2).getByRole("radio", { name: "codex" })).not.toBeChecked();
  });

  it("says on its row that its program is not installed, and cannot be started", async () => {
    machine();
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run1 = await screen.findByRole("radiogroup", { name: "First chat" });
    const codex = within(run1).getByRole("radio", { name: "codex" });
    await waitFor(() => expect(codex).toBeDisabled());
    expect(within(run1).getByText("Codex is not installed on this machine.")).toBeInTheDocument();
    expect(await hoverTipOf(codex)).toEqual("Codex is not installed on this machine.");
  });

  it("changes nothing when every program is installed", async () => {
    machine({
      found: {
        harnesses: [
          harness("claude", "Claude Code", true),
          harness("codex", "Codex", true),
          harness("pi", "Pi", true),
        ],
        local_models: [],
      },
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run2 = await screen.findByRole("radiogroup", { name: "Second chat" });
    await waitFor(() => expect(within(run2).getByRole("radio", { name: "codex" })).toBeChecked());
    expect(within(run2).getByRole("radio", { name: "codex" })).toBeEnabled();
    expect(screen.queryByText(/is not installed/)).not.toBeInTheDocument();
  });

  it("offers every profile as before when the machine could not be read", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_options") return START_OPTIONS;
      if (cmd === "harness_setup_found") throw "purlis could not look at this machine";
      if (cmd === "first_task_started") return [];
      return null;
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run2 = await screen.findByRole("radiogroup", { name: "Second chat" });
    await waitFor(() => expect(within(run2).getByRole("radio", { name: "codex" })).toBeChecked());
    expect(within(run2).getByRole("radio", { name: "codex" })).toBeEnabled();
  });

  it("says the machine could not be looked at, rather than saying nothing (#1719)", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_options") return START_OPTIONS;
      if (cmd === "harness_setup_found") throw "the probe was refused";
      if (cmd === "first_task_started") return [];
      return null;
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    expect(
      await screen.findByText(
        "purlis could not look at which harnesses this machine has, so every profile is offered: the probe was refused",
      ),
    ).toBeInTheDocument();
  });

  it("says the clone's branches could not be listed, rather than saying nothing (#1719)", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_options") return START_OPTIONS;
      if (cmd === "harness_setup_found") return NO_CODEX;
      if (cmd === "first_task_started") throw "git is not on PATH";
      return null;
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    expect(
      await screen.findByText(
        "purlis could not list the branches of this repo, so a chat started before this launch may show as not started: git is not on PATH",
      ),
    ).toBeInTheDocument();
  });

  it("starts no built-in profile before the machine has been looked at", async () => {
    let answer: (found: unknown) => void = () => {};
    const look = new Promise((resolve) => (answer = resolve));
    mockIPC((cmd) => {
      if (cmd === "start_options") return START_OPTIONS;
      if (cmd === "harness_setup_found") return look;
      if (cmd === "first_task_started") return [];
      return null;
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run1 = await screen.findByRole("radiogroup", { name: "First chat" });
    // A quick press while the look is out could start codex, whose program is missing.
    await waitFor(() => expect(within(run1).getByRole("radio", { name: "claude" })).toBeChecked());
    const start = within(runSection("First chat")).getByRole("button", {
      name: "Start the first chat",
    });
    expect(start).toBeDisabled();

    answer(NO_CODEX);
    await waitFor(() => expect(start).toBeEnabled());
  });

  it("judges only a built-in profile: one the project defines names its own command", async () => {
    mockIPC((cmd) => {
      if (cmd === "start_options")
        return {
          ...START_OPTIONS,
          profiles: [
            profile("claude", "claude", true),
            { ...profile("codex-mine", "codex"), source: "purlis.local.toml" },
          ],
        };
      if (cmd === "harness_setup_found") return NO_CODEX;
      if (cmd === "first_task_started") return [];
      return null;
    });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const run1 = await screen.findByRole("radiogroup", { name: "First chat" });
    await waitFor(() => expect(within(run1).getByRole("radio", { name: "claude" })).toBeChecked());
    expect(within(run1).getByRole("radio", { name: "codex-mine" })).toBeEnabled();
    expect(screen.queryByText(/is not installed/)).not.toBeInTheDocument();
  });
});

describe("the First task tab after a relaunch (#945)", () => {
  it("shows a chat whose branch is in the clone as started, and offers no second start", async () => {
    const { calls } = machine({ started: [onDisk(1)] });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const first = runSection("First chat");
    expect(
      await within(first).findByText(/Started on the branch first-task-1/),
    ).toBeInTheDocument();
    expect(
      within(first).queryByRole("button", { name: "Start the first chat" }),
    ).not.toBeInTheDocument();
    // The second never started: its Start is offered.
    expect(
      within(runSection("Second chat")).getByRole("button", { name: "Start the second chat" }),
    ).toBeInTheDocument();
    expect(calls("first_task_started")[0].args).toEqual({
      plane: PLANE,
      workspace: "widget",
      repo: "widget",
    });
  });

  it("shows the diff of a chat started before this launch, as the core says it", async () => {
    machine({ started: [onDisk(1)] });
    const shown: unknown[] = [];
    render(
      <FirstTaskTab
        plane={PLANE}
        clone={CLONE}
        does={{ ...DOES, showDiff: (run) => shown.push(run) }}
      />,
    );

    const first = runSection("First chat");
    await userEvent.click(await within(first).findByRole("button", { name: "Show its diff" }));
    expect(shown).toEqual([
      {
        folder: `${PLANE}/workspaces/widget/.worktrees/widget/first-task-1`,
        workspace: "widget",
        diff: "git add -N -A && git diff c1 --",
      },
    ]);
  });

  it("says why a chat started before this launch has no diff", async () => {
    machine({ started: [onDisk(2, null)] });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    const second = runSection("Second chat");
    expect(
      await within(second).findByText(/purlis has no record of which branch it was cut from/),
    ).toBeInTheDocument();
    expect(within(second).queryByRole("button", { name: "Show its diff" })).not.toBeInTheDocument();
  });

  it("offers Start when the core read back no run", async () => {
    machine({ started: [] });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

    await screen.findByRole("radiogroup", { name: "First chat" });
    await waitFor(() =>
      expect(
        within(runSection("First chat")).getByRole("button", { name: "Start the first chat" }),
      ).toBeEnabled(),
    );
    expect(screen.queryByText(/Started on the branch/)).not.toBeInTheDocument();
  });

  it("keeps the run this launch started, with its diff, over what the disk says", async () => {
    machine({ started: [onDisk(1)] });
    const run = {
      session: 11,
      name: "11",
      label: "first task 1",
      persona: "steward",
      harness: "claude",
      workspace: "widget",
      branch: "first-task-1",
      folder: `${PLANE}/workspaces/widget/.worktrees/widget/first-task-1`,
      diff: "git add -N -A && git diff 1a2b3c4d --",
    };
    render(
      <FirstTaskTab
        plane={PLANE}
        clone={CLONE}
        does={{ ...DOES, runs: { [CLONE]: { 1: run } } }}
      />,
    );

    const first = runSection("First chat");
    expect(within(first).getByText(/Started on the branch first-task-1/)).toBeInTheDocument();
    expect(within(first).getByRole("button", { name: "Show its diff" })).toBeInTheDocument();
  });
});

/** opencode's card, as the core reads it: it cannot be typed into. */
const OPENCODE_LINE =
  "opencode cannot have a prompt typed in for you, because purlis cannot tell when it has finished starting.";
const OPENCODE_CARD = {
  name: "opencode",
  title: "opencode",
  label: "What opencode can do here",
  lines: [OPENCODE_LINE],
  cannot_type: `${OPENCODE_LINE} See What opencode can do here.`,
};

describe("the First task tab's off rows and the harness card (#1134)", () => {
  it("ends an off row's reason with the card's label as a link that opens the card's tab", async () => {
    machine({
      found: { harnesses: [], local_models: [] },
      options: {
        ...START_OPTIONS,
        profiles: [
          profile("claude", "claude", true),
          { ...profile("oc", "opencode"), ready_to_type: false, harness: OPENCODE_CARD },
        ],
      },
    });
    const asked: SettingsActionAsk[] = [];
    const hear = (event: Event) => asked.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, hear);
    try {
      render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);

      const run1 = await screen.findByRole("region", { name: "First chat" });
      const links = await within(run1).findAllByRole("button", {
        name: "What opencode can do here",
      });
      expect(links).toHaveLength(1);
      // The line is still said, before the link.
      expect(links[0].parentElement?.textContent).toBe(OPENCODE_CARD.cannot_type);
      await userEvent.click(links[0]);

      expect(asked).toEqual([{ plane: PLANE, action: "harness.card:opencode" }]);
    } finally {
      window.removeEventListener(SETTINGS_ACTION, hear);
    }
  });
});

describe("the First task tab's one-time note on a harness that lacks something (#1134, W10)", () => {
  const LINE = "Codex does not tell purlis when it is waiting for you.";
  const CODEX = {
    ...profile("codex", "codex", true),
    harness: {
      name: "codex",
      title: "Codex",
      label: "What Codex can do here",
      lines: [LINE],
      cannot_type: null,
    },
  };
  const NOTE = `You have not started a chat on Codex on this machine before. ${LINE}`;
  const ALL_FOUND = { harnesses: [harness("codex", "Codex", true)], local_models: [] };

  afterEach(() => localStorage.clear());

  it("says the card's line in the chat's section until a chat has started on it", async () => {
    machine({ found: ALL_FOUND, options: { ...START_OPTIONS, profiles: [CODEX] } });
    const started: FirstTaskDoes = {
      ...DOES,
      start: () =>
        Promise.resolve({
          run: 1,
          branch: "first-task-1",
          folder: "/f",
          workspace: "widget",
          diff: "git diff",
          harness: "codex",
        } as unknown as FirstTaskRun),
    };
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={started} />);

    const run1 = await screen.findByRole("region", { name: "First chat" });
    expect(await within(run1).findByRole("note")).toHaveTextContent(NOTE);
    const start = within(run1).getByRole("button", { name: "Start the first chat" });
    await waitFor(() => expect(start).toBeEnabled());
    await userEvent.click(start);
    cleanup();

    machine({ found: ALL_FOUND, options: { ...START_OPTIONS, profiles: [CODEX] } });
    render(<FirstTaskTab plane={PLANE} clone={CLONE} does={DOES} />);
    const again = await screen.findByRole("region", { name: "Second chat" });
    await within(again).findByRole("radio", { name: "codex" });
    expect(screen.queryByRole("note")).toBeNull();
  });
});
