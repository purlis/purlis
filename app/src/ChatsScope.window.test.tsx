// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, moved, nothingKnown, type ChatStates } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { forgetThisLaunch } from "./regions";

/**
 * **The switch: a chat isn't a tab** (#4758): a tab holds no chats of its own to list, so the
 * Chats view's scopes are Workspace and All, and This tab is gone.
 */

function listed(session: number, workspace: string, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name: `steward ${session}`,
    persona: "steward",
    workspace,
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "claude",
    ...more,
  };
}

const task = (session: number, parent: number, workspace: string) =>
  listed(session, workspace, {
    name: `devops ${session}`,
    persona: "devops",
    parent,
    mode: "task",
    from: `steward ${parent}`,
    tab: false,
  });

/** Chat 1 with its task 2, and chat 3, in alpha. Chat 5 in beta. */
const CHATS = [listed(1, "alpha"), task(2, 1, "alpha"), listed(3, "alpha"), listed(5, "beta")];
const EVERY = chatsTree(CHATS);
/** The board, with `queue` needing the person. */
function board(queue: number[] = []): ChatStates {
  return CHATS.reduce(
    (known, chat, at) =>
      moved(known, {
        plane: "/plane",
        session: chat.session,
        state: queue.includes(chat.session) ? "waiting" : "running",
        needs_you: queue.includes(chat.session),
        queue,
        moved_at: at + 1,
        sequence: at + 1,
        reports: [],
        refusals: [],
        children: [],
      }),
    nothingKnown,
  );
}

const theList = () => screen.getByRole("tree");
const shape = () =>
  within(theList())
    .queryAllByRole("treeitem")
    .map(
      (one) => `${one.getAttribute("aria-level")} ${one.querySelector(".session")?.textContent}`,
    );
const scopeOn = () =>
  within(screen.getByRole("radiogroup", { name: "Show the chats of" }))
    .getAllByRole("radio")
    .filter((one) => (one as HTMLInputElement).checked)
    .map((one) => one.closest("label")?.textContent);

beforeEach(forgetThisLaunch);
afterEach(cleanup);

describe("the Chats view's scopes once a chat isn't a tab (#4758)", () => {
  it("offers Workspace and All, and no This tab", () => {
    render(
      <ChatsHere.Provider value={fixedChats(board())}>
        <ChatsSection rows={EVERY} here="alpha" onOpen={() => {}} />
      </ChatsHere.Provider>,
    );

    const scopes = within(screen.getByRole("radiogroup", { name: "Show the chats of" }))
      .getAllByRole("radio")
      .map((one) => one.closest("label")?.textContent);
    expect(scopes).toEqual(["Workspace", "All"]);
    expect(scopeOn()).toEqual(["Workspace"]);
    expect(shape()).toEqual(["1 steward 1", "2 devops 2", "1 steward 3"]);
  });
});
