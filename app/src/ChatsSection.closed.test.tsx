// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import type { Offer } from "./actions";
import type { ChatCard, ClosedChat, PlaneId } from "./bindings";
import { findRow, queryRow, queryTree, rowNamed, treeNamed } from "./ui/tree/tree.testkit";

/**
 * **The Chats list's filter finds a closed chat** (#6484): by its number, one it had before a
 * restart, or its title, under *Closed chats* after every open chat, and choosing it opens its
 * card. The core is mocked; what it reads is `chatlookup::closed_now`'s own tests'.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/projects/alpha" as unknown as PlaneId;

const OPEN: ListedChat = {
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

const CLOSED: ClosedChat = {
  number: 731,
  earlier: [702],
  title: "Tidy the changelog",
  place: "smart-ide",
  ended: "2026-11-12T11:00:00Z",
};

const CARD: ChatCard = {
  number: 731,
  chat: "01K6CHATS0000000000000000C",
  title: "Tidy the changelog",
  ended: "2026-11-12T11:00:00Z",
  place: "smart-ide",
  started_by: "you",
  screen_kept: false,
  chat_reopen: "reopens",
  task: null,
};

function draw(answers: { closed?: () => unknown; byNumber?: (number: number) => unknown } = {}) {
  const asked: string[] = [];
  mockIPC((cmd, args) => {
    asked.push(cmd);
    if (cmd === "closed_chats") return answers.closed === undefined ? [CLOSED] : answers.closed();
    if (cmd === "chat_by_number") {
      const { number } = args as { number: number };
      if (answers.byNumber !== undefined) return answers.byNumber(number);
      return number === 731 ? { is: "ended", card: CARD } : { is: "no_chat" };
    }
    return null;
  });
  const pressed: Offer[] = [];
  render(
    <ChatsHere.Provider value={{ ...fixedChats(), plane: PLANE }}>
      <ChatsSection
        rows={chatsTree([OPEN])}
        front={1}
        onOpen={() => {}}
        onPress={(offer) => pressed.push(offer)}
      />
    </ChatsHere.Provider>,
  );
  return { asked, pressed };
}

function theTree(): Promise<HTMLElement> {
  return screen.findByRole("treegrid", { name: "Chats of this project" });
}

async function typeInTheFilter(text: string) {
  await userEvent.type(screen.getByRole("searchbox", { name: /^Filter chats/ }), text);
}

describe("a closed chat in the Chats list's filter (#6484)", () => {
  it("is not read, nor drawn, until something is typed", () => {
    const { asked } = draw();
    expect(asked).not.toContain("closed_chats");
    const tree = queryTree("Chats of this project");
    expect(tree === null ? null : queryRow(tree, /Closed chats/)).toBeNull();
  });

  it("is found by its title under Closed chats, and choosing it opens its card", async () => {
    const { pressed } = draw();
    await typeInTheFilter("changelog");
    const row = await findRow(await theTree(), /^#731 · Tidy the changelog/);
    expect(rowNamed(treeNamed("Chats of this project"), /^Closed chats/)).toBeInTheDocument();
    expect(screen.getByText("Only a closed chat matches the filter.")).toBeInTheDocument();

    await userEvent.click(row);

    const card = await screen.findByRole("dialog", { name: "#731 · Tidy the changelog" });
    await userEvent.click(within(card).getByRole("button", { name: "Reopen" }));
    await waitFor(() => expect(pressed).toHaveLength(1));
    expect(pressed[0].does).toEqual({ verb: "reopenClosedChat", number: 731 });
  });

  it("is found by a number it had before a restart", async () => {
    draw();
    await typeInTheFilter("702");
    expect(await findRow(await theTree(), /^#731 · Tidy the changelog/)).toBeInTheDocument();
  });

  it("says so when the closed chats could not be read", async () => {
    draw({
      closed: () => {
        throw new Error("the disk is gone");
      },
    });
    await typeInTheFilter("changelog");
    expect(
      await screen.findByText(/purlis could not read the closed chats \(.*the disk is gone.*\)/),
    ).toBeInTheDocument();
  });

  it("says so when its card could not be read as it is chosen", async () => {
    draw({
      byNumber: () => {
        throw new Error("the disk is gone");
      },
    });
    await typeInTheFilter("changelog");
    await userEvent.click(await findRow(await theTree(), /^#731 · Tidy the changelog/));
    expect(
      await screen.findByText(/purlis could not open chat #731's card \(.*the disk is gone.*\)/),
    ).toBeInTheDocument();
  });

  it("says so when it is no longer kept as it is chosen", async () => {
    draw({ byNumber: () => ({ is: "no_chat" }) });
    await typeInTheFilter("changelog");
    await userEvent.click(await findRow(await theTree(), /^#731 · Tidy the changelog/));
    expect(await screen.findByText(/Chat #731 is no longer kept/)).toBeInTheDocument();
  });

  it("is not found by words it does not hold, and the open chats stay as the filter finds them", async () => {
    draw();
    await typeInTheFilter("steward");
    const tree = await theTree();
    expect(await findRow(tree, /^steward 1/)).toBeInTheDocument();
    expect(queryRow(tree, /Closed chats/)).toBeNull();
  });
});
