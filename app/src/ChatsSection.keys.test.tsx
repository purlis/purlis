// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Chats list's keyboard, focus and names, pinned** (#3198): what a person reaches and
 * hears in the list, read by what the rows carry (`data-session`) and what they say, so it holds
 * whatever draws the tree. Each of the list's own keys (Space, Delete, F2, Escape), its one Tab
 * stop, the arrows, Enter and a press, the row's menu, its drag mark, and what a reader hears.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { aCardIsUp, forgetCards } from "./chatCard";
import { CARD_DELAY_MS, ChatsSection } from "./ChatsSection";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { chatsTree, type ListedChat } from "./chatsTree";
import { besideId, renameId, taskStopId, ONLY_A_TASK_OPENS_BESIDE, type Offer } from "./actions";
import type { ChatBrowser, FinishedTask } from "./bindings";
import { CHAT_ROW } from "./paneDrag";
import { heard, keysOn, queryRow, rowNamed, rowsOf, treeNamed } from "./ui/tree/tree.testkit";

afterEach(cleanup);

const chat = (more: Partial<ListedChat> & Pick<ListedChat, "session" | "name">): ListedChat => ({
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
  ...more,
});

/** Two sessions; the first asked for a task. */
const ROWS: ListedChat[] = [
  chat({ session: 1, name: "steward 1" }),
  chat({
    session: 2,
    name: "fix the login",
    persona: "devops",
    parent: 1,
    mode: "task",
    tab: false,
  }),
  chat({ session: 3, name: "steward 3" }),
];

const WORKING: ChatStates = {
  ...nothingKnown,
  bySession: { 1: "running", 2: "running", 3: "done" },
};

/** A catalogue row the list presses, available or off with its reason. */
function offer(id: string, reason = ""): Offer {
  return { id, title: id, available: reason === "", reason, does: { verb: "newShell" } } as Offer;
}

function draw({
  front = 1,
  offers = [] as Offer[],
  finished,
  browsers,
}: {
  front?: number;
  offers?: Offer[];
  finished?: ReadonlyMap<number, FinishedTask[]>;
  browsers?: ReadonlyMap<number, ChatBrowser[]>;
} = {}) {
  const opened: number[] = [];
  const pressed: string[] = [];
  const browsed: string[] = [];
  render(
    <ChatsHere.Provider value={fixedChats(WORKING)}>
      <input aria-label="before" />
      <ChatsSection
        rows={chatsTree(ROWS)}
        front={front}
        onOpen={(session) => opened.push(session)}
        offers={new Map(offers.map((one) => [one.id, one]))}
        onPress={(one) => pressed.push(one.id)}
        finished={finished}
        browsers={
          browsers &&
          new Map(
            [...browsers].map(([session, all]) => [
              session,
              all.map((browser) => ({ browser, chat: "steward 1" })),
            ]),
          )
        }
        onBrowser={browsers && ((shown) => browsed.push(shown.browser.name))}
      />
    </ChatsHere.Provider>,
  );
  return { opened, pressed, browsed };
}

/** Chat `session`'s row. */
const rowOf = (session: number) => {
  const row = document.querySelector<HTMLElement>(`[data-session="${session}"]`);
  if (row === null) throw new Error(`no row for chat ${session}`);
  return row;
};

/** The chat whose row has the keyboard. */
const focusedChat = () =>
  Number((document.activeElement as HTMLElement | null)?.getAttribute("data-session") ?? NaN);

describe("the Chats list's one Tab stop", () => {
  it("is the row of the chat in front, said as the current one", async () => {
    draw({ front: 3 });
    expect(rowOf(3)).toHaveAttribute("aria-current", "true");
    expect(rowOf(1)).not.toHaveAttribute("aria-current");

    screen.getByLabelText("Filter chats by name, persona, workspace or state").focus();
    // The filter, then the chips, then the list.
    await userEvent.tab();
    await userEvent.tab();
    await waitFor(() => expect(focusedChat()).toBe(3));
  });

  // **Decided with the move (#3198)**: the tree family's rule, as the Explorer has it and a
  // treegrid's roving stop does. Before, a chat's row the keyboard left went back to the chat
  // in front; now the list keeps the row the keyboard was last on, once it has been in.
  it("is left on the row the keyboard was last on, once it has been in the list", async () => {
    draw({ front: 1 });
    act(() => rowOf(1).focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(focusedChat()).toBe(2);
    await userEvent.tab({ shift: true });
    await userEvent.tab();
    await waitFor(() => expect(focusedChat()).toBe(2));
  });
});

describe("the arrows in the Chats list", () => {
  it("move down and up the rows, and Home and End go to the first and the last", async () => {
    draw();
    act(() => rowOf(1).focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(focusedChat()).toBe(2);
    await userEvent.keyboard("{ArrowDown}");
    expect(focusedChat()).toBe(3);
    await userEvent.keyboard("{ArrowUp}");
    expect(focusedChat()).toBe(2);
    await userEvent.keyboard("{End}");
    expect(focusedChat()).toBe(3);
    await userEvent.keyboard("{Home}");
    expect(focusedChat()).toBe(1);
  });

  it("fold a row with ← and open it again with →, and its rows go and come back", async () => {
    draw();
    act(() => rowOf(1).focus());
    expect(rowOf(1)).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(rowOf(1)).toHaveAttribute("aria-expanded", "false"));
    expect(document.querySelector('[data-session="2"]')).toBeNull();
    expect(focusedChat()).toBe(1);
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(rowOf(1)).toHaveAttribute("aria-expanded", "true"));
    expect(rowOf(2)).toBeInTheDocument();
  });

  it("say nothing of folding on a row with nothing under it", () => {
    draw();
    expect(rowOf(3)).not.toHaveAttribute("aria-expanded");
  });
});

describe("a row's own keys", () => {
  it("open its chat on Enter and on a press", async () => {
    const { opened } = draw();
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{Enter}");
    await userEvent.click(within(rowOf(1)).getByText("steward 1"));
    expect(opened).toEqual([3, 1]);
  });

  it("open a task beside its session on Space, and say why not on a chat that is no task", async () => {
    const { pressed, opened } = draw({ offers: [offer(besideId(2))] });
    act(() => rowOf(2).focus());
    await userEvent.keyboard(" ");
    expect(pressed).toEqual([besideId(2)]);

    act(() => rowOf(3).focus());
    await userEvent.keyboard(" ");
    expect(pressed).toEqual([besideId(2)]);
    expect(opened).toEqual([]);
    await waitFor(() => expect(screen.getByText(ONLY_A_TASK_OPENS_BESIDE)).toBeInTheDocument());
  });

  it("ask to stop a task on Delete, and do nothing on a chat that is no task", async () => {
    const { pressed } = draw({ offers: [offer(taskStopId(2)), offer(taskStopId(3))] });
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{Delete}");
    expect(pressed).toEqual([]);
    act(() => rowOf(2).focus());
    await userEvent.keyboard("{Delete}");
    expect(pressed).toEqual([taskStopId(2)]);
  });

  it("rename its chat on F2, and say why where it cannot", async () => {
    const { pressed } = draw({
      offers: [offer(renameId(1)), offer(renameId(3), "It is being stopped.")],
    });
    act(() => rowOf(1).focus());
    await userEvent.keyboard("{F2}");
    expect(pressed).toEqual([renameId(1)]);
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{F2}");
    expect(pressed).toEqual([renameId(1)]);
    await waitFor(() => expect(screen.getByText("It is being stopped.")).toBeInTheDocument());
  });

  it("take the filter off on Escape", async () => {
    draw();
    await userEvent.type(screen.getByLabelText(/^Filter chats/), "steward 3");
    await waitFor(() => expect(document.querySelector('[data-session="1"]')).toBeNull());
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(rowOf(1)).toBeInTheDocument());
    expect(screen.getByLabelText(/^Filter chats/)).toHaveValue("");
  });
});

describe("a row, as a reader hears it and the window carries it", () => {
  it("is heard by its chat's name first, then its persona and its state", () => {
    draw();
    expect(heard(rowOf(2))).toMatch(/^fix the login\b.*devops.*working/);
    expect(heard(rowOf(3))).toMatch(/^steward 3\b.*steward/);
  });

  it("carries its chat's number for a drag onto a pane and for F2", () => {
    draw();
    expect(rowOf(2)).toHaveAttribute(CHAT_ROW, "2");
    expect(rowOf(2)).toHaveAttribute("data-renames-on-f2");
  });

  it("opens its chat's list on Shift+F10, and the menu key", async () => {
    draw({ offers: [offer(renameId(3))] });
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    expect(await screen.findByRole("menu")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    fireEvent.keyDown(rowOf(3), { key: "ContextMenu" });
    expect(await screen.findByRole("menu")).toBeInTheDocument();
  });
});

const finishedTask = (
  id: string,
  name: string,
  more: Partial<FinishedTask> = {},
): FinishedTask => ({
  id,
  asker: 1,
  name,
  chat: null,
  did_not_start: false,
  attempts: 0,
  waits: null,
  persona: "devops",
  how: "done",
  outcome: "done",
  folds: true,
  report: `${name}: all good.`,
  changed: null,
  ended: null,
  place: "alpha",
  branch: null,
  reopens: true,
  not_reopened: null,
  ...more,
});

/** What the keyboard is put on to work finished task `id`'s line. */
const finishedLine = (id: string) => {
  const at = document.querySelector<HTMLElement>(`[data-task-id="${id}"]`);
  if (at === null) throw new Error(`no line for task ${id}`);
  return at.matches('[role="row"]') ? at : (at.querySelector<HTMLElement>("button") ?? at);
};

describe("a chat's finished tasks, from the keyboard", () => {
  const FINISHED = new Map([
    [
      3,
      [
        finishedTask("t-1", "check the logs", { how: "failed", outcome: "failed", folds: false }),
        finishedTask("t-2", "tidy up"),
      ],
    ],
  ]);

  it("show a task's report on Enter, as its own words", async () => {
    draw({ finished: FINISHED });
    act(() => finishedLine("t-1").focus());
    await userEvent.keyboard("{Enter}");
    expect(
      await screen.findByRole("region", { name: "Report of check the logs" }),
    ).toHaveTextContent("check the logs: all good.");
  });

  it("fold the done ones into one line the keyboard opens, with Clear finished", async () => {
    draw({ finished: FINISHED });
    const fold = screen.getByText(/^Finished \(1\)/).closest<HTMLElement>("[aria-expanded]");
    expect(fold).toHaveAttribute("aria-expanded", "false");
    expect(document.querySelector('[data-task-id="t-2"]')).toBeNull();
    act(() => fold?.focus());
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(document.querySelector('[data-task-id="t-2"]')).not.toBeNull());
    expect(screen.getByRole("button", { name: "Clear finished" })).toBeInTheDocument();
  });
});

describe("a chat's browsers, from the keyboard", () => {
  const B1: ChatBrowser = {
    session: 3,
    name: "b1",
    program: "B1",
    unended: false,
    pinned: false,
    was: null,
    login: null,
    driver: null,
    pool: null,
    secrets: [],
  };

  it("open a browser's view on Enter on its line, which says which browser it is", async () => {
    const { browsed } = draw({ browsers: new Map([[3, [B1]]]) });
    const line = screen.getByText("Browser b1").closest<HTMLElement>('[role="row"], button');
    act(() => line?.focus());
    await userEvent.keyboard("{Enter}");
    expect(browsed).toEqual(["b1"]);
  });
});

/**
 * **The five keyboard decisions of the move onto the tree family** (#3198), each held here.
 */
describe("the Chats list on the tree family (#3198)", () => {
  const FINISHED = new Map([
    [
      3,
      [
        finishedTask("t-1", "check the logs", { how: "failed", outcome: "failed", folds: false }),
        finishedTask("t-2", "tidy up"),
      ],
    ],
  ]);
  const B1: ChatBrowser = {
    session: 1,
    name: "b1",
    program: "B1",
    unended: false,
    pinned: false,
    was: null,
    login: null,
    driver: null,
    pool: null,
    secrets: [],
  };

  it("1. makes a finished task a row the arrows reach, whose buttons → reaches and whose report Enter shows and says", async () => {
    draw({ finished: FINISHED });
    const tree = treeNamed("Chats of this project");
    const task = rowNamed(tree, "check the logs");
    act(() => rowOf(3).focus());
    await userEvent.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(task);
    // Its report, under its line, said as its description.
    await userEvent.keyboard("{Enter}");
    expect(within(task).getByRole("region", { name: "Report of check the logs" })).toBeTruthy();
    expect(task.getAttribute("aria-description")).toContain("check the logs: all good.");
    // Its buttons are the row's own: out of the Tab order, reached by →.
    await userEvent.keyboard("{ArrowRight}");
    expect(task.contains(document.activeElement)).toBe(true);
    expect(document.activeElement).not.toBe(task);
    expect(document.activeElement).toHaveAttribute("tabindex", "-1");
    // "Finished (1)" is a row whose rows are the folded ones, with Clear finished at its end.
    const fold = rowNamed(tree, "Finished (1)");
    expect(fold).toHaveAttribute("aria-expanded", "false");
    expect(within(fold).getByRole("button", { name: "Clear finished" })).toBeTruthy();
  });

  it("2. makes a chat's browsers its rows, so a chat with browsers folds them", async () => {
    draw({ browsers: new Map([[3, [{ ...B1, session: 3 }]]]) });
    const tree = treeNamed("Chats of this project");
    expect(rowOf(3)).toHaveAttribute("aria-expanded", "true");
    const browser = rowNamed(tree, "Browser b1");
    expect(browser).toHaveAttribute("aria-level", "2");
    await keysOn(rowOf(3), "{ArrowLeft}");
    await waitFor(() => expect(rowOf(3)).toHaveAttribute("aria-expanded", "false"));
    expect(queryRow(tree, "Browser b1")).toBeNull();
  });

  it("3. raises a chat's card from the row's own element, hover and focus, and describes the row by it", async () => {
    draw();
    // The pointer resting on the row brings up its card (the row's own hover).
    await userEvent.hover(within(rowOf(3)).getByText("steward 3"));
    const card = await screen.findByTestId("chat-card-3", {}, { timeout: CARD_DELAY_MS + 1000 });
    expect(rowOf(3)).toHaveAttribute("aria-describedby", within(card).getByRole("tooltip").id);
    await userEvent.unhover(within(rowOf(3)).getByText("steward 3"));
    await waitFor(() => expect(screen.queryByTestId("chat-card-3")).toBeNull());
    // The keyboard resting on the row brings it up too (the row's own focus), and leaving takes
    // it down, and the row's description back.
    await userEvent.keyboard("{Shift}");
    act(() => rowOf(3).focus());
    const again = await screen.findByTestId("chat-card-3", {}, { timeout: CARD_DELAY_MS + 1000 });
    expect(rowOf(3)).toHaveAttribute("aria-describedby", within(again).getByRole("tooltip").id);
    act(() => rowOf(1).focus());
    await waitFor(() => expect(screen.queryByTestId("chat-card-3")).toBeNull());
    expect(rowOf(3)).not.toHaveAttribute("aria-describedby");
  });

  it("4. hears Space and Delete before the tree's own keys: nothing is chosen, and a control in the row keeps its keys", async () => {
    const { pressed, opened } = draw({
      offers: [offer(besideId(2)), offer(taskStopId(2)), offer(renameId(2))],
    });
    act(() => rowOf(2).focus());
    await userEvent.keyboard(" ");
    expect(rowOf(2)).not.toHaveAttribute("aria-selected");
    await userEvent.keyboard("{Delete}");
    expect(pressed).toEqual([besideId(2), taskStopId(2)]);
    // On the row's own control (its ⋯), Space presses the control and Delete is not the row's.
    const more = within(rowOf(2)).getByRole("button", { name: /^More for / });
    act(() => more.focus());
    await userEvent.keyboard(" ");
    expect(await screen.findByRole("menu")).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    act(() => more.focus());
    await userEvent.keyboard("{Delete}");
    expect(pressed).toEqual([besideId(2), taskStopId(2)]);
    expect(opened).toEqual([]);
  });

  it("5. nests the rows it holds still, and opens what `opens` says is open", async () => {
    draw();
    const tree = treeNamed("Chats of this project");
    const shape = () =>
      rowsOf(tree).map(
        (row) => `${row.getAttribute("aria-level")} ${row.getAttribute("aria-label")}`,
      );
    expect(shape()).toEqual(["1 steward 1", "2 fix the login", "1 steward 3"]);
    // A fold set by hand is `opens`, which the tree is told as its open rows; and it is held
    // while the pointer is over the list, as the list holds its rows still (V100-47).
    await userEvent.hover(tree);
    await keysOn(rowOf(1), "{ArrowLeft}");
    await waitFor(() => expect(shape()).toEqual(["1 steward 1", "1 steward 3"]));
    expect(rowOf(1)).toHaveAttribute("aria-expanded", "false");
    await keysOn(rowOf(1), "{ArrowRight}");
    await waitFor(() => expect(shape()).toEqual(["1 steward 1", "2 fix the login", "1 steward 3"]));
  });

  it("clears the done tasks from the Finished (n) row's menu, as from its button", async () => {
    const cleared: string[][] = [];
    render(
      <ChatsHere.Provider value={fixedChats(WORKING)}>
        <ChatsSection
          rows={chatsTree(ROWS)}
          front={1}
          onOpen={() => {}}
          finished={FINISHED}
          onClearFinished={(ids) => cleared.push(ids)}
        />
      </ChatsHere.Provider>,
    );
    const fold = rowNamed(treeNamed("Chats of this project"), "Finished (1)");
    act(() => fold.focus());
    await userEvent.keyboard("{Shift>}{F10}{/Shift}");
    await userEvent.click(await screen.findByRole("menuitem", { name: "Clear finished" }));
    expect(cleared).toEqual([["t-2"]]);
  });
});

describe("a row's card and the keyboard leaving (#6073)", () => {
  afterEach(() => {
    vi.useRealTimers();
    forgetCards();
  });

  // **The card comes up just as the keyboard leaves** (#6073): the rest's wait ends and the
  // card is up in the window's one record (`chatCard.ts`) before React has drawn the row again,
  // and the keyboard leaves the row in that moment. The row's leaving must take the card down
  // by that record, not by what the row last drew, or the card stays up with no keyboard and
  // no pointer on the row (WebKit's `RolledUpHand` picture).
  it("takes the card down when the keyboard leaves the row the moment its card came up", async () => {
    draw();
    // A key first, so the focus below is the keyboard resting on the row, not a press's.
    await userEvent.keyboard("{Shift}");
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    act(() => rowOf(3).focus());
    // The wait ends, outside React's turn: the record says up, the row has not drawn it yet.
    vi.advanceTimersByTime(CARD_DELAY_MS);
    expect(aCardIsUp()).toBe(true);
    rowOf(3).blur();
    vi.useRealTimers();
    await act(async () => {});
    expect(aCardIsUp()).toBe(false);
    expect(screen.queryByTestId("chat-card-3")).toBeNull();
  });
});
