// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SaveConflict } from "./SavingResolve";
import { FIX_STARTED, type FixStarted } from "./saving";
import type { Fixing, PlaneSaving, SaveSettled } from "./bindings";

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

function standing(over: Partial<PlaneSaving> = {}): PlaneSaving {
  return {
    stage: "blocked",
    changed: [],
    ahead: 1,
    pr: null,
    request: "pull request",
    blocked: null,
    branch: "main",
    pushes: true,
    behind: 3,
    pushFailed: null,
    live: [],
    conflicts: [],
    conflictKeys: {},
    notice: null,
    mode: null,
    modeFrom: "default",
    journal: [],
    ...over,
  };
}

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

const settled = (over: Partial<SaveSettled>): SaveSettled => ({
  outcome: "refused",
  said: "",
  files: [],
  lines: [],
  ...over,
});

const STOPPED: Partial<PlaneSaving> = {
  blocked: "a rebase is stopped part-way with conflicts in notes.md",
  conflicts: ["notes.md"],
  stoppedBy: "rebase",
};

describe("SaveConflict", () => {
  it("offers Resolve on a save's replay that conflicted, which the window runs itself", async () => {
    const sent = commandsSent({
      settle_save_conflict: settled({
        outcome: "stopped",
        said: "git stopped on a conflict.",
        files: ["notes.md"],
      }),
    });
    render(
      <SaveConflict
        plane={PLANE}
        saving={standing({ conflicts: ["notes.md"], toResolve: "Press Resolve." })}
      />,
    );

    expect(screen.getByRole("status").textContent).toContain("resolve folder beside the project");
    expect(screen.queryByRole("button", { name: "Resolve in a chat" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Resolve" }));

    expect(sent).toEqual([
      {
        cmd: "settle_save_conflict",
        args: { plane: PLANE, step: "start", files: ["notes.md"], keys: [] },
      },
    ]);
    expect(await screen.findByText("git stopped on a conflict.")).toBeTruthy();
  });

  it("while the window's replay is stopped, offers a chat, Continue, Abort and the terminal", async () => {
    const sent = commandsSent({
      settle_save_conflict: settled({ outcome: "saved", said: "Resolved." }),
    });
    const onSaved = vi.fn();
    render(<SaveConflict plane={PLANE} saving={standing(STOPPED)} onSaved={onSaved} />);

    expect(
      within(screen.getByRole("list", { name: "Where it conflicts" }))
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["notes.md"]);
    for (const name of ["Resolve in a chat", "Continue", "Abort", "Open terminal here"])
      expect(screen.getByRole("button", { name })).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Continue" }));

    expect(sent).toEqual([
      {
        cmd: "settle_save_conflict",
        args: { plane: PLANE, step: "continue", files: ["notes.md"], keys: [] },
      },
    ]);
    expect(await screen.findByText("Resolved.")).toBeTruthy();
    expect(onSaved).toHaveBeenCalledTimes(1);
  });

  it("names the keys both machines changed beside the file, and Continue stages the file alone", async () => {
    // #5824 F6 review: a key in the path made Continue refuse it as no file git stopped in.
    const sent = commandsSent({
      settle_save_conflict: settled({ outcome: "saved", said: "Resolved." }),
    });
    render(
      <SaveConflict
        plane={PLANE}
        saving={standing({
          ...STOPPED,
          stoppedBy: "merge",
          conflicts: ["charter.toml"],
          conflictKeys: { "charter.toml": ["sandbox.mode", "theme.name"] },
        })}
      />,
    );

    expect(
      within(screen.getByRole("list", { name: "Where it conflicts" }))
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["charter.toml: sandbox.mode, theme.name"]);

    await userEvent.click(screen.getByRole("button", { name: "Continue" }));

    expect(sent).toEqual([
      {
        cmd: "settle_save_conflict",
        args: { plane: PLANE, step: "continue", files: ["charter.toml"], keys: [] },
      },
    ]);
  });

  it("aborts on the press, and says what the window refused", async () => {
    const sent = commandsSent({
      settle_save_conflict: settled({
        said: "`notes.md` still has conflict markers (`<<<<<<<`, `>>>>>>>`). Settle them, then Continue.",
      }),
    });
    render(<SaveConflict plane={PLANE} saving={standing(STOPPED)} />);

    await userEvent.click(screen.getByRole("button", { name: "Abort" }));

    expect(sent[0]).toEqual({
      cmd: "settle_save_conflict",
      args: { plane: PLANE, step: "abort", files: ["notes.md"], keys: [] },
    });
    expect(await screen.findByText(/still has conflict markers/)).toBeTruthy();
  });

  it("starts the chat that edits the files, and the window brings its tab forward", async () => {
    const chat: Fixing = {
      session: 9,
      name: "9",
      label: "Resolve the conflict",
      persona: null,
      harness: "claude",
      workspace: null,
      existing: false,
    };
    const sent = commandsSent({ resolve_save_in_a_chat: chat });
    const started: FixStarted[] = [];
    const hear = (event: Event) => started.push((event as CustomEvent<FixStarted>).detail);
    window.addEventListener(FIX_STARTED, hear);
    render(<SaveConflict plane={PLANE} saving={standing(STOPPED)} />);

    await userEvent.click(screen.getByRole("button", { name: "Resolve in a chat" }));

    expect(sent).toEqual([
      { cmd: "resolve_save_in_a_chat", args: { plane: PLANE, columns: 80, rows: 24 } },
    ]);
    await vi.waitFor(() => expect(started).toEqual([{ plane: PLANE, chat }]));
    window.removeEventListener(FIX_STARTED, hear);
  });

  it("offers no chat once nothing is left in conflict, only Continue and Abort", () => {
    commandsSent();
    render(<SaveConflict plane={PLANE} saving={standing({ ...STOPPED, conflicts: [] })} />);

    expect(screen.queryByRole("button", { name: "Resolve in a chat" })).toBeNull();
    expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Abort" })).toBeTruthy();
  });

  it("draws nothing for a block that is not a conflict of a save's replay", () => {
    const { container } = render(
      <SaveConflict plane={PLANE} saving={standing({ blocked: "off its branch" })} />,
    );
    expect(container.textContent).toBe("");
  });

  it("offers Skip when the settling left the commit empty, and sends it", async () => {
    let answers: SaveSettled[] = [
      settled({ outcome: "empty", said: "Skip drops it." }),
      settled({ outcome: "saved", said: "Resolved." }),
    ];
    const sent: { cmd: string; args: Record<string, unknown> }[] = [];
    mockIPC((cmd, args) => {
      sent.push({ cmd, args: args as Record<string, unknown> });
      const [first, ...rest] = answers;
      answers = rest;
      return first;
    });
    render(<SaveConflict plane={PLANE} saving={standing({ ...STOPPED, conflicts: [] })} />);

    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    await userEvent.click(await screen.findByRole("button", { name: "Skip" }));

    expect(sent.map((one) => one.args.step)).toEqual(["continue", "skip"]);
    expect(await screen.findByText("Resolved.")).toBeTruthy();
  });
});

describe("SaveConflict, with a resolve open in its folder (#5824 F7)", () => {
  const FOLDER = "/home/dev/plane/.purlis/resolve";
  const RESOLVING: Partial<PlaneSaving> = {
    blocked: "a resolve is open",
    conflicts: ["notes.md"],
    stoppedBy: "resolve",
    resolving: { folder: FOLDER, keys: [] },
  };

  it("names the folder, offers a chat, Continue and Abort, and never says the project stopped", () => {
    commandsSent();
    render(<SaveConflict plane={PLANE} saving={standing(RESOLVING)} />);
    expect(screen.getByText(new RegExp(`resolve folder ${FOLDER}`))).toBeTruthy();
    expect(screen.queryByText(/is stopped/)).toBeNull();
    for (const name of ["Resolve in a chat", "Continue", "Abort"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
  });

  it("asks for each settings key's value, and Continue sends the choice", async () => {
    const sent = commandsSent({
      settle_save_conflict: settled({ outcome: "refused", said: "not yet" }),
    });
    render(
      <SaveConflict
        plane={PLANE}
        saving={standing({
          ...RESOLVING,
          conflicts: ["charter.toml"],
          conflictKeys: { "charter.toml": ["sandbox.mode"] },
          resolving: {
            folder: FOLDER,
            keys: [
              { file: "charter.toml", key: "sandbox.mode", ours: '"workspace"', theirs: '"off"' },
            ],
          },
        })}
      />,
    );
    const group = screen.getByRole("radiogroup", { name: "sandbox.mode in charter.toml" });
    // No chat for a settings key: only the person chooses it.
    expect(screen.queryByRole("button", { name: "Resolve in a chat" })).toBeNull();
    await userEvent.click(within(group).getByRole("radio", { name: 'The remote\'s: "off"' }));
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(sent).toEqual([
      {
        cmd: "settle_save_conflict",
        args: {
          plane: PLANE,
          step: "continue",
          files: ["charter.toml"],
          keys: [{ file: "charter.toml", key: "sandbox.mode", theirs: true }],
        },
      },
    ]);
  });

  it("offers Skip once Continue finds nothing of this machine's left", async () => {
    commandsSent({
      settle_save_conflict: settled({ outcome: "empty", said: "Settled, nothing is left." }),
    });
    render(<SaveConflict plane={PLANE} saving={standing(RESOLVING)} />);
    await userEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(await screen.findByRole("button", { name: "Skip" })).toBeTruthy();
  });
});
