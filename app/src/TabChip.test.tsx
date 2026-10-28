// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { drawnWith } from "./cascade.testkit";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { ChatRow } from "./chatsTree";
import { TabTasks } from "./TabChip";
import type { Shown } from "./shownState";
import { stateClock } from "./stateClock";
import { REST_MS, type Ended, type Needing } from "./tabTasks";
import type { TasksUsed } from "./bindings";
import type { UsedAsk, UsedReader } from "./tasksUsed";
import { eventually, tipOf } from "./ui/tooltip/tooltip.testkit";

/**
 * **A tab's chip, drawn alone** (#1487): what it does that the window has no way to bring
 * about in a test, a tab being carried, a touch, and the footer later tickets fill.
 * `TabChip.window.test.tsx` holds it in the window.
 */

function row(session: number, level: number, name: string): ChatRow {
  return {
    session,
    name,
    persona: null,
    workspace: "alpha",
    shell: false,
    parent: level === 1 ? null : 1,
    mode: level === 1 ? null : "task",
    from: null,
    tab: level === 1,
    branch: null,
    report: level === 1 ? null : "owed",
    outcome: null,
    asking: null,
    harness: "Claude Code",
    level,
    posinset: 1,
    setsize: 1,
    orphaned: false,
  };
}

const ROWS = [row(1, 1, "steward 1"), row(2, 2, "talk")];

const DONE: Shown = { kind: "done", word: "done", shape: "tick", token: "text.muted" };
const FAILED: Shown = { kind: "failed", word: "failed", shape: "cross", token: "state.failed" };
const CANCELLED: Shown = {
  kind: "cancelled",
  word: "cancelled",
  shape: "dash",
  token: "text.muted",
};

/** A finished task of the session, as the window hands one to the chip. */
function finished(name: string, more: Partial<Ended> = {}): Ended {
  return {
    key: `finished:${name}`,
    asker: 1,
    name,
    persona: null,
    shown: DONE,
    folds: true,
    bucket: more.shown?.kind === "failed" ? "failed" : "done",
    elsewhere: null,
    ...more,
  };
}

/** The pointer comes onto `on` and moves once: what a hand does, and all that starts a rest. */
function comeTo(on: Element, pointerType = "mouse") {
  fireEvent.pointerEnter(on, { pointerType, clientX: 40, clientY: 12 });
  fireEvent.pointerMove(on, { pointerType, clientX: 42, clientY: 12 });
}

function chip(
  over: {
    rows?: ChatRow[];
    ended?: Ended[];
    needs?: Needing[];
    dragging?: () => boolean;
    limits?: ReactNode;
    used?: UsedReader;
  } = {},
) {
  const onShow = vi.fn();
  render(
    <ChatsHere.Provider
      value={fixedChats({ ...nothingKnown, bySession: { 1: "running", 2: "running" } })}
    >
      <TabTasks
        id="the-counts"
        name="steward 1"
        rows={over.rows ?? ROWS}
        ended={over.ended ?? []}
        current={1}
        needs={over.needs ?? []}
        asked={0}
        clock={stateClock()}
        dragging={over.dragging ?? (() => false)}
        onShow={onShow}
        limits={over.limits}
        used={over.used}
      />
    </ChatsHere.Provider>,
  );
  return onShow;
}

const counts = () => screen.getByRole("button", { name: /^Tasks of steward 1/ });
const menu = () => screen.queryByRole("menu");
/** The surface the open menu stands on, which holds its footer under its list. */
const surface = () =>
  (menu() as HTMLElement).closest('[data-scope="popover"][data-part="root"]') as HTMLElement;

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

describe("a tab's chip", () => {
  it("is nothing at all for a session with no tasks", () => {
    chip({ rows: [row(1, 1, "steward 1")] });

    expect(document.body.querySelector(".tab-tasks")).toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("says its counts in words, under the id its tab is described by", () => {
    chip();

    expect(counts().getAttribute("aria-label")).toBe("Tasks of steward 1: 1 working");
    expect(counts().id).toBe("the-counts");
    // No tooltip of its own: it would come up over the menu the same rest opened.
    expect(tipOf(counts())).toBeNull();
    // The drawn count is decoration to a screen reader: the name has said it.
    expect(counts().querySelector("[data-count]")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("opens no menu while a tab is being carried, by a rest or by the press that ends it", () => {
    vi.useFakeTimers();
    chip({ dragging: () => true });

    comeTo(counts());
    act(() => void vi.advanceTimersByTime(REST_MS * 4));
    expect(menu()).toBeNull();

    fireEvent.click(counts());
    expect(menu()).toBeNull();
  });

  it("does not open when a drag begins while the pointer is resting on it", () => {
    vi.useFakeTimers();
    let carried = false;
    chip({ dragging: () => carried });

    comeTo(counts());
    act(() => void vi.advanceTimersByTime(REST_MS - 10));
    carried = true;
    act(() => void vi.advanceTimersByTime(REST_MS));

    expect(menu()).toBeNull();
  });

  it("opens under a finger only when pressed: a touch does not rest", () => {
    vi.useFakeTimers();
    chip();

    comeTo(counts(), "touch");
    act(() => void vi.advanceTimersByTime(REST_MS * 4));
    expect(menu()).toBeNull();

    expect(counts()).not.toHaveAttribute("data-open");
    fireEvent.click(counts());
    expect(menu()).not.toBeNull();
    // The family draws the counts open while their menu is (#3524).
    expect(counts()).toHaveAttribute("data-open", "");
  });

  it("opens for a pointer the person moved onto it and then held still", () => {
    vi.useFakeTimers();
    chip();

    comeTo(counts());
    act(() => void vi.advanceTimersByTime(REST_MS));

    expect(menu()).not.toBeNull();
  });

  it("opens nothing when it comes under a pointer that has not moved", () => {
    vi.useFakeTimers();
    chip();

    // The chip appeared, widened or slid under a parked pointer: the engine says the pointer
    // came on, and then that it is still at the same point. Nobody's hand moved.
    fireEvent.pointerEnter(counts(), { pointerType: "mouse", clientX: 40, clientY: 12 });
    act(() => void vi.advanceTimersByTime(5_000));
    expect(menu()).toBeNull();
    fireEvent.pointerMove(counts(), { pointerType: "mouse", clientX: 40, clientY: 12 });
    act(() => void vi.advanceTimersByTime(5_000));

    expect(menu()).toBeNull();
  });

  it("opens nothing for a pointer moving over it with a button held", () => {
    vi.useFakeTimers();
    chip();

    fireEvent.pointerEnter(counts(), { pointerType: "mouse", clientX: 40, clientY: 12 });
    fireEvent.pointerMove(counts(), { pointerType: "mouse", clientX: 44, clientY: 12, buttons: 1 });
    act(() => void vi.advanceTimersByTime(5_000));

    expect(menu()).toBeNull();
  });

  it("draws a count for each of the four ways its tasks stand, in order, none for a zero", () => {
    render(
      <ChatsHere.Provider
        value={fixedChats({
          ...nothingKnown,
          bySession: { 2: "running", 3: "waiting", 4: "waiting" },
          needsYou: [4],
        })}
      >
        <TabTasks
          id="the-counts"
          name="steward 1"
          rows={[ROWS[0], row(2, 2, "talk"), row(3, 2, "idle one"), row(4, 2, "asks you")]}
          ended={[finished("probe", { shown: FAILED, folds: false }), finished("old")]}
          current={1}
          needs={[]}
          asked={0}
          clock={stateClock()}
          dragging={() => false}
          onShow={() => undefined}
        />
      </ChatsHere.Provider>,
    );

    expect(counts().getAttribute("aria-label")).toBe(
      "Tasks of steward 1: 1 working, 2 waiting, 1 failed, 1 done",
    );
    expect(
      [...counts().querySelectorAll("[data-count]")].map(
        (count) => `${count.getAttribute("data-count")} ${count.textContent}`,
      ),
    ).toEqual(["working 1", "waiting 2", "failed 1", "done 1"]);
  });

  it("opens the report of a finished task under its line, and keeps the menu and the keyboard", () => {
    const onShow = chip({
      ended: [finished("probe", { shown: FAILED, folds: false, report: "The deploy is red." })],
    });
    fireEvent.click(counts());
    const line = within(menu() as HTMLElement).getByRole("menuitem", { name: /probe/ });
    expect(line.getAttribute("aria-disabled")).toBeNull();
    expect(line.getAttribute("aria-expanded")).toBe("false");

    line.focus();
    fireEvent.keyDown(line, { key: "Enter" });

    expect(menu()).not.toBeNull();
    expect(document.activeElement).toBe(line);
    expect(line.getAttribute("aria-expanded")).toBe("true");
    // Under its words, and the line's description while it is open.
    expect(line.querySelector(".tasks-menu-report")?.textContent).toBe("The deploy is red.");
    expect(line).toHaveAccessibleDescription(/The deploy is red\./);
    expect(onShow).not.toHaveBeenCalled();

    // And again closes it.
    fireEvent.keyDown(line, { key: "Enter" });
    expect(line.querySelector(".tasks-menu-report")).toBeNull();
    expect(menu()).not.toBeNull();
  });

  it("reaches a finished line with the arrows like any other", async () => {
    chip({ ended: [finished("probe", { shown: FAILED, folds: false })] });
    fireEvent.click(counts());
    const items = within(menu() as HTMLElement).getAllByRole("menuitem");
    expect(items.map((item) => item.querySelector(".name")?.textContent)).toEqual([
      "steward 1",
      "talk",
      "probe",
    ]);

    items[1].focus();
    fireEvent.keyDown(items[1], { key: "ArrowDown" });

    await waitFor(() => expect(document.activeElement).toBe(items[2]));
  });

  it("says what the core says of how a task ended, beside its state", () => {
    chip({
      ended: [
        finished("halt", { shown: CANCELLED, folds: false, qualifier: "closed by the person" }),
      ],
    });
    fireEvent.click(counts());

    const line = within(menu() as HTMLElement).getByRole("menuitem", { name: /halt/ });
    expect(line.textContent?.replace(/\s+/g, " ").trim()).toBe(
      "halt cancelled closed by the person",
    );
    // It does not fold: it is not behind Finished (n).
    expect(within(menu() as HTMLElement).queryByRole("menuitem", { name: /Finished/ })).toBeNull();
  });

  it("opens and closes the Finished fold with Right and Left, as a tree's row does", () => {
    chip({ ended: [finished("old"), finished("older")] });
    fireEvent.click(counts());
    const fold = within(menu() as HTMLElement).getByRole("menuitem", { name: "Finished (2)" });
    fold.focus();

    fireEvent.keyDown(fold, { key: "ArrowRight" });
    expect(fold.getAttribute("aria-expanded")).toBe("true");
    expect(within(menu() as HTMLElement).getByRole("menuitem", { name: /older/ })).toBeTruthy();

    fireEvent.keyDown(fold, { key: "ArrowLeft" });
    expect(fold.getAttribute("aria-expanded")).toBe("false");
    expect(within(menu() as HTMLElement).queryByRole("menuitem", { name: /older/ })).toBeNull();
    expect(menu()).not.toBeNull();
  });

  it("says in words who asked for a task under a task, which its indent alone would not", () => {
    chip({ rows: [ROWS[0], ROWS[1], { ...row(3, 3, "deep"), parent: 2 }] });
    fireEvent.click(counts());

    const line = within(menu() as HTMLElement).getByRole("menuitem", { name: /deep/ });
    expect(line.textContent).toContain(", asked by talk");
    expect(line.querySelector(".hidden-words")?.textContent).toBe(", asked by talk");
    // And the chat shown is said in words, and marked for the eye without a second saying.
    const own = within(menu() as HTMLElement).getByRole("menuitem", { name: /steward 1/ });
    expect(own.textContent).toContain(", shown now");
    expect(own.getAttribute("aria-current")).toBeNull();
    expect(own.hasAttribute("data-current")).toBe(true);
  });

  it("goes to the chat that waits from its hand, and opens no menu for that", () => {
    const onShow = chip({ needs: [{ session: 2, name: "talk" }] });

    fireEvent.click(screen.getByRole("button", { name: "talk needs you. Go to talk" }));

    expect(onShow).toHaveBeenCalledWith(2);
    expect(menu()).toBeNull();
  });

  it("draws no footer in its menu until something has a line for it", () => {
    chip();
    fireEvent.click(counts());

    expect(surface().querySelector(".tasks-menu-foot")).toBeNull();
  });

  it("draws the limits it is handed and what the tasks used at the foot of its menu, under the rows", async () => {
    chip({ limits: "4 of 6 running", used: reader(USED) });
    fireEvent.click(counts());

    const foot = surface().querySelector(".tasks-menu-foot") as HTMLElement;
    expect(foot.querySelector(".tasks-menu-limits")?.textContent).toBe("4 of 6 running");
    await waitFor(() =>
      expect(foot.querySelector(".tasks-menu-totals")?.textContent).toBe(
        "2 tasks · at least 70k tokens · 6m",
      ),
    );
    expect(
      foot.querySelector(".tasks-menu-totals")
        ? tipOf(foot.querySelector(".tasks-menu-totals"))
        : undefined,
    ).toBe(USED.total?.explained);
    // After every row, and not a row: under the list, where nothing in it is picked.
    expect((menu() as HTMLElement).contains(foot)).toBe(false);
    expect(
      (menu() as HTMLElement).compareDocumentPosition(foot) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(within(foot).queryByRole("menuitem")).toBeNull();
  });
});

/** What the core answers for the tab of `ROWS` and one finished task. */
const USED: TasksUsed = {
  chats: [
    { session: 1, tokens: "100k in, 20k out", unsaid: null },
    { session: 2, tokens: "60k in, 10k out", unsaid: null },
  ],
  finished: [{ id: "report", tokens: null, unsaid: "nothing" }],
  total: {
    tasks: 2,
    said: "2 tasks · at least 70k tokens · 6m",
    explained: "Tokens the tasks used, as each harness reported them: 70k.",
  },
};

function reader(answer: TasksUsed) {
  return vi.fn<(ask: UsedAsk) => Promise<TasksUsed | undefined>>(() => Promise.resolve(answer));
}

describe("what a session's tasks used (#1500)", () => {
  it("is read as the menu opens, for the session, its open tasks and its ended ones, and not before", async () => {
    const read = reader(USED);
    chip({ used: read, ended: [finished("report", { folds: false, shown: FAILED })] });
    expect(read).not.toHaveBeenCalled();

    fireEvent.click(counts());

    await waitFor(() => expect(read).toHaveBeenCalled());
    expect(read.mock.calls[0][0]).toEqual({ own: 1, chats: [2], finished: ["report"] });
  });

  it("says each task's tokens on its line's hover, and a dash and why where there are none", async () => {
    chip({
      used: reader(USED),
      ended: [finished("report", { folds: false, shown: FAILED })],
    });
    fireEvent.click(counts());

    const talk = await screen.findByRole("menuitem", { name: /^talk/ });
    await eventually(() => expect(tipOf(talk)).toBe("talk\nTokens: 60k in, 10k out"));
    const report = screen.getByRole("menuitem", { name: /^report/ });
    expect(tipOf(report)).toBe("report\nTokens: — (nothing reported)");
    expect(tipOf(report)).not.toContain("0");
  });

  it("says nothing of tokens where the window hands it no reader", () => {
    chip();
    fireEvent.click(counts());

    expect((menu() as HTMLElement).querySelector(".tasks-menu-totals")).toBeNull();
    // No tokens line: its tooltip only repeats its name, for a pointer where the name is cut
    // short, and the keyboard's focus opens none.
    expect(tipOf(screen.getByRole("menuitem", { name: /^talk/ }))).toBeNull();
  });
});

describe("the chip's pill (#3264, D-3264-3)", () => {
  /** A tab's chip as `TabTasks` draws it, in its strip: the hand's box, then the counts' pill. */
  function chip(open: boolean) {
    const strip = document.createElement("div");
    strip.className = "tabs";
    strip.innerHTML = `<div class="tab"><span class="tab-tasks"${open ? " data-open" : ""}>
      <span class="tab-tasks-mark"><button class="tab-tasks-hand"></button></span>
      <span class="tab-tasks-pill"><button class="tab-tasks-counts" data-scope="button"
        data-part="root" data-variant="quiet" data-size="sm"${open ? " data-open" : ""}></button></span>
    </span></div>`;
    document.body.append(strip);
    const [mark, pill] = ["tab-tasks-mark", "tab-tasks-pill"].map(
      (one) => strip.querySelector(`.${one}`) as HTMLElement,
    );
    return { strip, mark, pill };
  }

  it("is the sunken ground, and the counts draw their open menu from the family (#3524)", () => {
    const shut = chip(false);
    expect(drawnWith(shut.pill, "background")).toBe("var(--surface-sunken)");
    expect(drawnWith(shut.pill, "border-radius")).toBe("var(--radius-full)");
    // It clips the quiet button's hover fill, and its open one, to its ends.
    expect(drawnWith(shut.pill, "overflow")).toBe("hidden");
    const counts = (one: { pill: HTMLElement }) => one.pill.querySelector("button") as HTMLElement;
    expect(drawnWith(counts(shut), "background")).toBe("transparent");
    // Open, the pill is as it was, and the button family draws the counts as under the pointer.
    const open = chip(true);
    expect(drawnWith(open.pill, "background")).toBe("var(--surface-sunken)");
    expect(drawnWith(counts(open), "background")).toBe("var(--surface-hover)");
    expect(drawnWith(counts(open), "color")).toBe("var(--text-primary)");
    shut.strip.remove();
    open.strip.remove();
  });

  it("draws the keyboard's ring on the pill and the hand's box, inside them, as a strip's are", () => {
    const { strip, mark, pill } = chip(false);
    for (const box of [pill, mark]) {
      // The state React Aria writes on the button inside, as the cascade testkit reads it.
      box.querySelector("button")?.setAttribute("data-cascade-focus", "");
      expect(drawnWith(box, "outline-offset", { focusVisible: true })).toBe(
        "var(--focus-ring-inset)",
      );
      expect(drawnWith(box, "outline", { focusVisible: true })).toBe(
        "var(--focus-ring-width) solid var(--focus-ring)",
      );
    }
    // Without the keyboard's focus inside, no ring.
    for (const box of [pill, mark])
      box.querySelector("button")?.removeAttribute("data-cascade-focus");
    expect(drawnWith(pill, "outline")).toBeUndefined();
    strip.remove();
  });
});
