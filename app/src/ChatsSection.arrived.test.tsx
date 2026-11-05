// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";

/**
 * **A chat another chat started, on its row** (#4759, spec #4750 stories 26 and 28): a handoff,
 * or a task on a harness purlis has no hooks into, starts in no pane and is announced, and its
 * row says "new" until it has been on screen. A task that can't report asks keeps the faint
 * hand on its row, since nothing it asks reaches the ✋.
 */

afterEach(cleanup);

/** The chat that asked, open in a tab. */
const ASKER: ListedChat = {
  session: 1,
  name: "steward 1",
  persona: "steward",
  workspace: "alpha",
  shell: false,
  parent: null,
  mode: null,
  from: null,
  tab: true,
  branch: null,
  report: null,
  outcome: null,
  asking: null,
  harness: "Claude Code",
};

/** Chat `session`, started by the asker as `mode`, in no pane. */
const started = (
  session: number,
  name: string,
  mode: "handoff" | "task",
  more: Partial<ListedChat> = {},
): ListedChat => ({
  ...ASKER,
  session,
  name,
  parent: 1,
  mode,
  from: "steward 1",
  tab: false,
  ...more,
});

function draw(chats: ListedChat[], noted: ReadonlySet<number> = new Set()) {
  render(
    <ChatsHere.Provider value={fixedChats(nothingKnown)}>
      <ChatsSection rows={chatsTree(chats)} front={1} noted={noted} onOpen={() => {}} />
    </ChatsHere.Provider>,
  );
}

/** The row of the chat called `name`. */
const rowOf = (name: string) =>
  within(screen.getByRole("tree"))
    .getAllByRole("treeitem")
    .find((row) => row.querySelector(".session")?.textContent === name) as HTMLElement;

describe("an announced chat's row (#4759)", () => {
  it("says new while the person has not looked at it, and is heard saying so", () => {
    draw([ASKER, started(2, "fix the login", "handoff", { unseen: true })]);

    const row = rowOf("fix the login");
    expect(row.querySelector('[data-mark="new"]')).toHaveTextContent("new");
    expect(row).toHaveTextContent(/fix the login.*new/);
  });

  it("says nothing new once it has been looked at, nor on a chat nobody announced", () => {
    draw([ASKER, started(2, "fix the login", "handoff", { unseen: false })]);

    expect(rowOf("fix the login").querySelector('[data-mark="new"]')).toBeNull();
    expect(rowOf("steward 1").querySelector('[data-mark="new"]')).toBeNull();
  });
});

describe("a task that can't report asks, on its row (#4759)", () => {
  it("keeps the faint hand, named for what it means", () => {
    draw([ASKER, started(2, "check the queue", "task", { cantReport: true, unseen: true })]);

    const hand = rowOf("check the queue").querySelector('[data-mark="cant-report"]');
    expect(hand).toHaveAccessibleName("Can't report asks: look at it from time to time");
  });

  it("draws no faint hand on a task whose harness reports its asks", () => {
    draw([ASKER, started(2, "count the rows", "task")]);

    expect(rowOf("count the rows").querySelector('[data-mark="cant-report"]')).toBeNull();
  });
});

describe("a chat in no pane with a note in the Notices centre, on its row (#4759)", () => {
  it("is marked, named for where the note is, and no other row is", () => {
    draw([ASKER, started(2, "fix the login", "handoff")], new Set([2]));

    expect(rowOf("fix the login").querySelector('[data-mark="noted"]')).toHaveAccessibleName(
      "A notice about it is in the Notices centre",
    );
    expect(rowOf("steward 1").querySelector('[data-mark="noted"]')).toBeNull();
  });
});
