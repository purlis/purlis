// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { WorkspaceRemovalNotice, holdsSaid } from "./WorkspaceRemovalNotice";
import { NoticeOf } from "./Notice";
import { grants } from "./noticeSettle";
import { askRemoveWorkspace, useRemoveWorkspaceAsked } from "./removeWorkspaceAsk";
import type { Shown } from "./bindings";

/**
 * A workspace a chat asked the person to remove is a question on that chat's tab (qw356):
 * which workspace, what removing it deletes, and Remove… and Keep. Remove… removes nothing:
 * it opens the window's own Delete workspace, where the person's press decides. Keep puts the
 * question away.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;

/** The ask the registry lists for chat `session` asking to have `workspace` removed. */
const removal = (workspace: string, session = SESSION, memories = 12, todos = 1): Shown => ({
  session,
  ask: `removal:${session}:${workspace}`,
  says: `Removing workspace ${workspace} deletes its memory, its todos and its clones`,
  options: [
    { id: "remove", label: "Remove…", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: [`Safe remove · ${workspace}`],
  answer: { via: "workspace-removal", workspace, memories, todos },
});

/** A core that records what the window sends, and refuses Keep with `refuse`. */
function core(refuse?: string) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "keep_asked_workspace" && refuse !== undefined) throw refuse;
    return null;
  });
  /** What was sent that is not a read of the asks. */
  return () => asked.filter((one) => one.cmd !== "asks_waiting");
}

describe("a workspace a chat asked you to remove", () => {
  it("says which workspace, what removing it deletes and holds, and sends nothing unasked", () => {
    const sent = core();
    render(
      <WorkspaceRemovalNotice
        plane={PLANE}
        session={SESSION}
        asks={[removal("boot-migration-safety")]}
      />,
    );
    const notice = screen.getByRole("status", { name: "Remove workspace" });
    expect(notice.textContent).toContain(
      "This chat asks you to remove workspace boot-migration-safety. Removing it deletes its " +
        "memory, its todos and its clones. It holds 12 memories and 1 open todo. Remove… opens " +
        "Delete workspace, where you decide: nothing is removed here, and this chat is told " +
        "what you decided on its next turn.",
    );
    // The name is the project's own listing's, drawn as data.
    expect(notice.querySelector("code")?.textContent).toBe("boot-migration-safety");
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual([
      "Remove…",
      "Keep",
    ]);
    expect(sent()).toEqual([]);
  });

  it("opens the window's own Delete workspace on Remove…, and sends no removal", async () => {
    const sent = core();
    const onWay = vi.fn();
    const opened: string[] = [];
    renderHook(() => useRemoveWorkspaceAsked(PLANE, (workspace) => opened.push(workspace)));
    const ask = removal("boot-migration-safety");
    render(<WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[ask]} onWay={onWay} />);
    await userEvent.click(screen.getByRole("button", { name: "Remove…" }));
    await waitFor(() => expect(opened).toEqual(["boot-migration-safety"]));
    // Nothing is removed by the press, and nothing is on its way to the chat: no command.
    expect(sent()).toEqual([]);
    expect(onWay).not.toHaveBeenCalled();
    // The question still stands while the dialog is read.
    expect(screen.getByRole("button", { name: "Remove…" })).toBeTruthy();
  });

  it("puts the question away on Keep, and removes nothing", async () => {
    const sent = core();
    render(<WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[removal("alpha")]} />);
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    await waitFor(() =>
      expect(sent()).toEqual([
        {
          cmd: "keep_asked_workspace",
          args: { plane: PLANE, session: SESSION, workspace: "alpha" },
        },
      ]),
    );
  });

  it("says why a Keep was not taken, and the question stands", async () => {
    const said = "Nothing is asking to remove alpha any more, so nothing was removed.";
    const sent = core(said);
    const onWay = vi.fn();
    const ask = removal("alpha");
    render(<WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[ask]} onWay={onWay} />);
    await userEvent.click(screen.getByRole("button", { name: "Keep" }));
    const notice = screen.getByRole("status", { name: "Remove workspace" });
    await waitFor(() => expect(notice.textContent).toContain(said));
    expect(sent().map((one) => one.cmd)).toEqual(["keep_asked_workspace"]);
    expect(onWay).toHaveBeenCalledWith("Keep", ask);
  });

  it("draws only this chat's removals, and none of its other asks", () => {
    core();
    const other: Shown = {
      session: SESSION,
      ask: "dispatch:7",
      says: "Wants to hand a task to devops",
      options: [{ id: "keep", label: "Keep blocked", allows: false }],
      source: "dispatch",
      chain: ["steward 3"],
      answer: { via: "dispatch", id: 7, shown: "s0" },
    };
    render(
      <WorkspaceRemovalNotice
        plane={PLANE}
        session={SESSION}
        asks={[removal("alpha", 4), other, removal("beta")]}
      />,
    );
    const notices = screen.getAllByRole("status", { name: "Remove workspace" });
    expect(notices).toHaveLength(1);
    expect(notices[0].querySelector("code")?.textContent).toBe("beta");
  });

  it("says whose question it is where its chat is not the one on screen", () => {
    core();
    render(
      <NoticeOf.Provider value={{ whose: "steward 4", onGo: () => {} }}>
        <WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[removal("alpha")]} />
      </NoticeOf.Provider>,
    );
    const said = screen.getByRole("status", { name: "steward 4: Remove workspace" }).textContent;
    expect(said).toContain("steward 4: It asks you to remove workspace alpha.");
    expect(said).toContain("and it is told what you decided");
    expect(said).not.toContain("his chat");
  });

  it("waits to settle before a press, as a Notice that grants does", () => {
    // Remove… leads to a removal: a Notice that moved under the pointer just now takes no press.
    expect(grants(`workspace-removal:${SESSION}:alpha`)).toBe(true);
  });

  it("counts what it holds in words", () => {
    expect(holdsSaid(0, 0)).toBe("It holds no memories and no open todos.");
    expect(holdsSaid(1, 1)).toBe("It holds 1 memory and 1 open todo.");
    expect(holdsSaid(12, 3)).toBe("It holds 12 memories and 3 open todos.");
  });
});

describe("what a Notice asks of its project's window", () => {
  it("opens Delete workspace in the window of its own project only", () => {
    const opened: string[] = [];
    renderHook(() => useRemoveWorkspaceAsked(PLANE, (workspace) => opened.push(workspace)));
    askRemoveWorkspace("/home/dev/other", "alpha");
    askRemoveWorkspace(PLANE, "beta");
    expect(opened).toEqual(["beta"]);
  });
});
