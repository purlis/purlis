// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import type { Offer } from "./actions";
import type { ChatCard, ClosedChat, FinishedTask, PlaneId } from "./bindings";
import { findRow, queryRow, queryTree, rowNamed, treeNamed } from "./ui/tree/tree.testkit";

/**
 * **The Chats list's filter finds a closed chat** (#6484): by its number, one it had before a
 * restart, or its title, under *Closed chats* after every open chat, and choosing it opens its
 * card. The core is mocked; what it reads, and which number each chat was found by (#7125), is
 * `chatcard::closed_chats_of`'s and `lookup_by_id`'s own tests'.
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
  id: "01K6CHATS0000000000000000C",
  earlier: [702],
  found_by: null,
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
  found_by: null,
};

/**
 * **A chat that closed under 702** (#6740's records): another chat's record also answers to 702,
 * the number it had before a restart, as the counter dealt 702 again.
 */
const CLOSED_UNDER_702: ClosedChat = {
  number: 702,
  id: "01K6CHATS0000000000000000E",
  earlier: [],
  found_by: null,
  title: "Sort the inbox",
  place: "smart-ide",
  ended: "2026-11-11T11:00:00Z",
};

const CARD_702: ChatCard = {
  ...CARD,
  number: 702,
  chat: CLOSED_UNDER_702.id,
  title: "Sort the inbox",
  ended: CLOSED_UNDER_702.ended,
};

/**
 * What the core answers `closed_chats` for the number typed: the chats that had it, 731 saying
 * it was found by 702, the number it had before a restart; or every one, for words.
 */
function closedFor(typed: number | null): ClosedChat[] {
  if (typed === null || typed === 731) return [CLOSED];
  if (typed === 702) return [{ ...CLOSED, found_by: 702 }];
  return [];
}

function draw(
  answers: {
    closed?: (typed: number | null) => unknown;
    byId?: (id: string, typed: number | null) => unknown;
    /** Each open chat's finished tasks, by its number. */
    finished?: ReadonlyMap<number, FinishedTask[]>;
  } = {},
) {
  const asked: string[] = [];
  const sent: { cmd: string; args: unknown }[] = [];
  const answered = { closed: 0 };
  mockIPC(async (cmd, args) => {
    asked.push(cmd);
    sent.push({ cmd, args });
    if (cmd === "closed_chats") {
      const { typed } = args as { typed: number | null };
      try {
        return answers.closed === undefined ? closedFor(typed) : answers.closed(typed);
      } finally {
        await Promise.resolve();
        answered.closed += 1;
      }
    }
    if (cmd === "chat_by_id") {
      const { id, typed } = args as { id: string; typed: number | null };
      if (answers.byId !== undefined) return answers.byId(id, typed);
      if (id === CARD.chat)
        return { is: "ended", card: { ...CARD, found_by: typed === 702 ? 702 : null } };
      if (id === CARD_702.chat) return { is: "ended", card: CARD_702 };
      return { is: "no_chat" };
    }
    // A number answers with the newest record that answers to it: never what a row opens.
    if (cmd === "chat_by_number") return { is: "ended", card: { ...CARD, found_by: 702 } };
    return null;
  });
  const pressed: Offer[] = [];
  const list = (open: ListedChat[]) => (
    <ChatsHere.Provider value={{ ...fixedChats(), plane: PLANE }}>
      <ChatsSection
        rows={chatsTree(open)}
        front={1}
        onOpen={() => {}}
        onPress={(offer) => pressed.push(offer)}
        finished={answers.finished}
      />
    </ChatsHere.Provider>
  );
  const { rerender } = render(list([OPEN, { ...OPEN, session: 2, name: "steward 2" }]));
  /** Chat 2 closes: the list is drawn with chat 1 alone. */
  const closeChat2 = () => rerender(list([OPEN]));
  return { asked, sent, pressed, answered, closeChat2 };
}

function theTree(): Promise<HTMLElement> {
  return screen.findByRole("treegrid", { name: "Chats of this project" });
}

async function typeInTheFilter(text: string) {
  await userEvent.type(screen.getByRole("searchbox", { name: /^Filter chats/ }), text);
}

/**
 * **The task 731 was**, finished in an earlier launch under chat 1: its row has no number
 * (`chat` is null), only its chat's ULID.
 */
const TIDIED: FinishedTask = {
  id: "01K6DISPATCH0000000000000T",
  asker: 1,
  name: "Tidy the changelog",
  chat: null,
  chat_id: CLOSED.id,
  persona: "steward",
  how: "done",
  outcome: "done",
  folds: false,
  report: "Tidied.",
  changed: null,
  ended: CLOSED.ended,
  place: "smart-ide",
  branch: null,
  reopens: true,
  not_reopened: null,
  did_not_start: false,
  attempts: 0,
  waits: null,
};

describe("a closed chat in the Chats list's filter (#6484)", () => {
  it("is not read, nor drawn, until something is typed", async () => {
    const { asked } = draw();
    // The list has drawn, and asked whatever it asks as it draws.
    await theTree();
    await act(() => Promise.resolve());
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
    expect(pressed[0].does).toEqual({ verb: "reopenClosedChat", chat: CLOSED.id });
  });

  it("is not drawn twice where its finished task's row is found, matched by its ULID (#7145)", async () => {
    draw({ finished: new Map([[1, [TIDIED]]]) });
    await typeInTheFilter("tidy");
    const tree = await theTree();
    await findRow(tree, /^Tidy the changelog$/);

    expect(queryRow(tree, /^#731 · Tidy the changelog/)).toBeNull();
    expect(queryRow(tree, /^Closed chats/)).toBeNull();
  });

  it("is drawn where its finished task's row is not found, as by a number (#7145)", async () => {
    draw({ finished: new Map([[1, [TIDIED]]]) });
    await typeInTheFilter("731");

    expect(await findRow(await theTree(), /^#731 · Tidy the changelog/)).toBeInTheDocument();
  });

  it("is found by a number it had before a restart, and its row and card say that number (#6685)", async () => {
    draw();
    await typeInTheFilter("702");
    const row = await findRow(await theTree(), /^#731 · Tidy the changelog \(was #702\)/);

    await userEvent.click(row);

    expect(
      await screen.findByRole("dialog", { name: "#731 · Tidy the changelog (was #702)" }),
    ).toBeInTheDocument();
  });

  it("asks the core with the number typed, and draws its row and card as the core says (#7125)", async () => {
    const { sent } = draw({
      closed: () => [{ ...CLOSED, found_by: 702 }],
      // The core's answer for the card is what the card says, whatever the row said.
      byId: () => ({ is: "ended", card: CARD }),
    });
    await typeInTheFilter("702");
    const row = await findRow(await theTree(), /^#731 · Tidy the changelog \(was #702\)/);
    expect(sent).toContainEqual({ cmd: "closed_chats", args: { plane: PLANE, typed: 702 } });

    await userEvent.click(row);

    expect(
      await screen.findByRole("dialog", { name: "#731 · Tidy the changelog" }),
    ).toBeInTheDocument();
    expect(sent).toContainEqual({
      cmd: "chat_by_id",
      args: { plane: PLANE, id: CLOSED.id, typed: 702 },
    });
  });

  it("each row found by a number two records answer to opens its own chat's card (#6685)", async () => {
    draw({
      closed: (typed) => (typed === 702 ? [{ ...CLOSED, found_by: 702 }, CLOSED_UNDER_702] : []),
    });
    await typeInTheFilter("702");
    const tree = await theTree();
    const restarted = await findRow(tree, /^#731 · Tidy the changelog \(was #702\)/);
    const closedUnder = await findRow(tree, /^#702 · Sort the inbox/);
    expect(closedUnder).not.toHaveTextContent("was");

    await userEvent.click(closedUnder);
    expect(
      await screen.findByRole("dialog", { name: "#702 · Sort the inbox" }),
    ).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await userEvent.click(restarted);
    expect(
      await screen.findByRole("dialog", { name: "#731 · Tidy the changelog (was #702)" }),
    ).toBeInTheDocument();
  });

  it("Reopen on each card of two found by one number reopens that card's own chat (#7123)", async () => {
    const { pressed } = draw({
      closed: (typed) => (typed === 702 ? [{ ...CLOSED, found_by: 702 }, CLOSED_UNDER_702] : []),
    });
    await typeInTheFilter("702");
    const tree = await theTree();
    const reopenFrom = async (row: RegExp, card: string) => {
      await userEvent.click(await findRow(tree, row));
      const shown = await screen.findByRole("dialog", { name: card });
      await userEvent.click(within(shown).getByRole("button", { name: "Reopen" }));
      await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    };

    await reopenFrom(/^#702 · Sort the inbox/, "#702 · Sort the inbox");
    await reopenFrom(/^#731 · Tidy the changelog/, "#731 · Tidy the changelog (was #702)");

    expect(pressed.map((offer) => offer.does)).toEqual([
      { verb: "reopenClosedChat", chat: CLOSED_UNDER_702.id },
      { verb: "reopenClosedChat", chat: CLOSED.id },
    ]);
  });

  it("found by the number it closed under, says no other (#6685)", async () => {
    draw();
    await typeInTheFilter("731");
    const row = await findRow(await theTree(), /^#731 · Tidy the changelog/);
    expect(row).not.toHaveTextContent("was");
  });

  it("asks nothing for a number past any a chat is dealt, and says no failure (#7125's review)", async () => {
    const { sent } = draw({
      // As the core refuses it: its `typed` is a u32, so a larger number is no argument.
      closed: (typed) => {
        if (typed !== null && typed > 0xffff_ffff) throw new Error("invalid args `typed`");
        return [];
      },
    });
    await typeInTheFilter("99999999999");
    await act(() => Promise.resolve());

    const typed = sent
      .filter((one) => one.cmd === "closed_chats")
      .map((one) => (one.args as { typed: number | null }).typed);
    expect(typed.filter((number) => number !== null && number > 0xffff_ffff)).toEqual([]);
    expect(screen.queryByText(/could not read the closed chats/)).toBeNull();
  });

  it("draws two closed chats that share a number as two rows (#6740)", async () => {
    draw({
      closed: (typed) =>
        typed === 702 ? [CLOSED_UNDER_702, { ...CLOSED_UNDER_702, id: CLOSED.id }] : [],
    });
    await typeInTheFilter("702");
    const tree = await theTree();
    await waitFor(() =>
      expect(within(tree).getAllByRole("row", { name: /^#702 · Sort the inbox/ })).toHaveLength(2),
    );
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
      byId: () => {
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
    draw({ byId: () => ({ is: "no_chat" }) });
    await typeInTheFilter("changelog");
    await userEvent.click(await findRow(await theTree(), /^#731 · Tidy the changelog/));
    expect(await screen.findByText(/Chat #731 is no longer kept/)).toBeInTheDocument();
  });

  it("is not found by words it does not hold, and the open chats stay as the filter finds them", async () => {
    const { answered } = draw();
    await typeInTheFilter("steward");
    const tree = await theTree();
    expect(await findRow(tree, /^steward 1/)).toBeInTheDocument();
    // What the core answered has reached the list before it is looked for.
    await waitFor(() => expect(answered.closed).toBeGreaterThan(0));
    await act(() => Promise.resolve());
    expect(queryRow(tree, /Closed chats/)).toBeNull();
  });

  it("is read again when a chat closes while the filter asks", async () => {
    const { answered, closeChat2 } = draw();
    await typeInTheFilter("changelog");
    await waitFor(() => expect(answered.closed).toBe(1));

    closeChat2();

    await waitFor(() => expect(answered.closed).toBe(2));
  });
});
