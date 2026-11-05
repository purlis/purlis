// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { catalogued, type Offer } from "./actions";
import { PaneActions, PaneActionsOf, PaneBellOf } from "./LayoutPanes";
import { paneLinesOf } from "./paneHead";

/**
 * **The head's right side is two groups** (#5203, spec #5191, settled on the M70 prototype):
 * first the chat's actions, the state button, the bell, then ⋯ last; then, after a thin divider,
 * the pane's controls, Split right, Split down, then Close last. An overflow menu ends the group
 * of actions it overflows, and the frame's controls sit apart at the edge. A pane showing a task
 * (no Close) and a view's pane (no chat actions) keep the same grouping for what they draw. The
 * whole window's head is `PaneHead.window.test.tsx`'s; this is each kind of pane's actions.
 */

afterEach(cleanup);

const SESSION = 7;

const offer = (id: string, title: string, does: Offer["does"]): Offer => ({
  id,
  title,
  available: true,
  reason: "",
  does,
});

const OFFERS: Record<string, Offer> = {
  "pane.split.right": offer("pane.split.right", "Split right", {
    verb: "split",
    direction: "row",
  }),
  "pane.split.down": offer("pane.split.down", "Split down", {
    verb: "split",
    direction: "column",
  }),
};
const CLOSE = offer("pane.close", "Close this pane", { verb: "closePane" });

/** A chat's list, which its ⋯ opens: one row is enough for it to be drawn. */
const LIST = catalogued([
  offer(`task.stop:${SESSION}`, "Stop and get its report", {
    verb: "endTask",
    session: SESSION,
    way: "report",
  }),
]);

/** The head's controls, start to end, by their names: what a screen reader tabs through. */
const names = (actions: HTMLElement) =>
  within(actions)
    .getAllByRole("button")
    .map((one) => one.getAttribute("aria-label") ?? one.textContent ?? "");

/** The divider between the chat's actions and the pane's controls, where one is drawn. */
const divider = (actions: HTMLElement) =>
  actions.querySelector<HTMLElement>('[data-scope="separator"]');

/** Whether `a` comes before `b` in the document: the order Tab and a screen reader take. */
const before = (a: Element, b: Element) =>
  (a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;

function draw({ chat, task = false }: { chat: boolean; task?: boolean }) {
  const { container } = render(
    <PaneActionsOf.Provider
      value={paneLinesOf(chat ? "chat" : "view", undefined, { closes: !task }).actions}
    >
      <PaneBellOf.Provider value={chat ? { notes: 0, trouble: 0, asks: 0 } : null}>
        <PaneActions
          pane={1}
          state={chat ? <button type="button">Stop</button> : undefined}
          chat={chat ? { session: SESSION, offers: LIST } : undefined}
          offerFor={(id) => OFFERS[id]}
          closeOf={() => CLOSE}
          onPaneDoes={() => undefined}
        />
      </PaneBellOf.Provider>
    </PaneActionsOf.Provider>,
  );
  return container.querySelector(".pane-actions") as HTMLElement;
}

describe("a pane head's right side (#5203)", () => {
  it("reads state button, bell, ⋯, then Split right, Split down and Close last", () => {
    const actions = draw({ chat: true });

    const [state, bell, more, right, down, close, ...rest] = names(actions);
    expect(state).toBe("Stop");
    expect(bell).toMatch(/^Pane notices:/);
    expect(more).toMatch(/^More for /);
    expect([right, down, close]).toEqual(["Split right", "Split down", "Close this pane"]);
    expect(rest).toEqual([]);
  });

  it("draws a thin divider between ⋯ and Split right, hidden from a screen reader", () => {
    const actions = draw({ chat: true });

    const line = divider(actions);
    expect(line).not.toBeNull();
    expect(line).toHaveAttribute("data-orientation", "vertical");
    expect(line).toHaveAttribute("aria-hidden", "true");
    const more = screen.getByRole("button", { name: /^More for / });
    const right = screen.getByRole("button", { name: "Split right" });
    expect(before(more, line as HTMLElement)).toBe(true);
    expect(before(line as HTMLElement, right)).toBe(true);
    // Part of the pane's controls: drawn when they are, and never left alone at rest.
    expect(line?.closest(".pane-doing")).not.toBeNull();
  });

  it("takes the keyboard in the order it is drawn, and never stops on the divider", async () => {
    draw({ chat: true });

    screen.getByRole("button", { name: "Stop" }).focus();
    const said = [];
    for (let step = 0; step < 5; step++) {
      await userEvent.tab();
      said.push(document.activeElement?.getAttribute("aria-label"));
    }
    expect(said[0]).toMatch(/^Pane notices:/);
    expect(said[1]).toMatch(/^More for /);
    expect(said.slice(2)).toEqual(["Split right", "Split down", "Close this pane"]);
  });

  it("keeps the grouping on a pane showing a task, which draws no Close", () => {
    const actions = draw({ chat: true, task: true });

    expect(names(actions).slice(1)).toEqual([
      expect.stringMatching(/^Pane notices:/),
      expect.stringMatching(/^More for /),
      "Split right",
      "Split down",
    ]);
    const line = divider(actions) as HTMLElement;
    expect(before(screen.getByRole("button", { name: /^More for / }), line)).toBe(true);
    expect(before(line, screen.getByRole("button", { name: "Split right" }))).toBe(true);
  });

  it("draws a view's pane its controls alone, with no divider before them", () => {
    const actions = draw({ chat: false });

    expect(names(actions)).toEqual(["Split right", "Split down", "Close this pane"]);
    expect(divider(actions)).toBeNull();
  });
});
