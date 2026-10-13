// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { WorkspaceRemovalNotice, holdsSaid } from "./WorkspaceRemovalNotice";
import { NoticeOf } from "./Notice";
import { grants } from "./noticeSettle";
import {
  askRemoveWorkspace,
  tellWorkspaceRemoved,
  useRemoveWorkspaceAsked,
  useWorkspaceRemovedOnAsk,
} from "./removeWorkspaceAsk";
import type { Shown } from "./bindings";

/**
 * A workspace a chat asked the person to remove is a question on that chat's tab (qw356):
 * which workspace, what removing it deletes, and Remove and Keep. Only a press sends anything;
 * Remove is the window's own remove, never forced, and a refusal is said in the core's words
 * with a way to the window's own dialog.
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
    { id: "remove", label: "Remove", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: [`Safe remove · ${workspace}`],
  answer: { via: "workspace-removal", workspace, memories, todos },
});

/** A core that records what the window sends, and refuses a removal with `refuse`. */
function core(refuse?: { said: string; at_risk: string[] }) {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "remove_asked_workspace") {
      if (refuse !== undefined) throw refuse;
      return ["Removed boot-migration-safety."];
    }
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
        "memory, its todos and its clones. It holds 12 memories and 1 open todo. Nothing is " +
        "removed unless you press Remove, and this chat is told what you decided on its next turn.",
    );
    // The name is the project's own listing's, drawn as data.
    expect(notice.querySelector("code")?.textContent).toBe("boot-migration-safety");
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual(["Remove", "Keep"]);
    expect(sent()).toEqual([]);
  });

  it("removes by the window's own remove on Remove, for the chat and workspace shown", async () => {
    const sent = core();
    const onWay = vi.fn();
    const ask = removal("boot-migration-safety");
    render(<WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[ask]} onWay={onWay} />);
    await userEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() =>
      expect(sent()).toEqual([
        {
          cmd: "remove_asked_workspace",
          args: { plane: PLANE, session: SESSION, workspace: "boot-migration-safety" },
        },
      ]),
    );
    expect(onWay).toHaveBeenCalledWith("Remove", ask);
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

  it("says the core's refusal whole, and leads to the window's own Delete workspace", async () => {
    const said = "alpha holds work nothing else has: svc has 2 commits not pushed.";
    const sent = core({ said, at_risk: ["svc"] });
    const opened: string[] = [];
    renderHook(() => useRemoveWorkspaceAsked(PLANE, (workspace) => opened.push(workspace)));
    render(<WorkspaceRemovalNotice plane={PLANE} session={SESSION} asks={[removal("alpha")]} />);
    expect(screen.queryByRole("button", { name: "Open Delete workspace…" })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Remove" }));

    const notice = screen.getByRole("status", { name: "Remove workspace" });
    await waitFor(() => expect(notice.textContent).toContain(said));
    // Never forced from here: the one command sent carries no force.
    expect(sent()).toEqual([
      {
        cmd: "remove_asked_workspace",
        args: { plane: PLANE, session: SESSION, workspace: "alpha" },
      },
    ]);
    // The question still stands, with the way to where a refusal is read and decided.
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open Delete workspace…" }));
    expect(opened).toEqual(["alpha"]);
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
    // Remove deletes: a Notice that moved under the pointer just now takes no press.
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

  it("tells its own project's window of a removal, with the core's words", () => {
    const told: [string, readonly string[]][] = [];
    const heard = renderHook(() =>
      useWorkspaceRemovedOnAsk(PLANE, (workspace, words) => told.push([workspace, words])),
    );
    tellWorkspaceRemoved("/home/dev/other", "alpha", ["Removed alpha."]);
    tellWorkspaceRemoved(PLANE, "beta", ["Removed beta."]);
    expect(told).toEqual([["beta", ["Removed beta."]]]);
    // A window that has gone hears nothing more.
    heard.unmount();
    tellWorkspaceRemoved(PLANE, "gamma", []);
    expect(told).toHaveLength(1);
  });
});
