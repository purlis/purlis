// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useRef, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  CHAT_ROW,
  dropPane,
  GRIP,
  PaneGrip,
  PlacingHere,
  STRIP,
  TAB,
  usePaneDrag,
} from "./paneDrag";
import { keystroke, shortcut } from "./keys/registry";
import { type DropContext, dropOn, judgeDrop } from "./paneDrop";
import { onAMac } from "./tabKeys";
import {
  contentsOf,
  insertPane,
  type Layout,
  noTabs,
  openTab,
  selectTab,
  type Tabs,
  without,
} from "./tabs";

/**
 * **Placement mode in the window** (#4764): the keyboard and single-press route for every drag.
 * jsdom lays nothing out, so each element says its own box (`data-box`), as in
 * `paneDrag.test.tsx`; what a screen reader makes of it is a hardware follow-up.
 */

const ctx: DropContext = {
  filedIn: () => "ops",
  name: (content) => (content.kind === "session" ? `chat ${content.session}` : "the empty pane"),
};

/** One tab: chat 1 left, chat 3 right, each 800 × 600; then a second tab, chat 4. */
function start(): Tabs {
  let tabs = openTab(noTabs(), 1, "1", "steward");
  const tab = tabs.order[0];
  tabs = insertPane(tabs, {
    tab,
    pane: contentsOf(tabs, tab)[0].pane,
    direction: "row",
    side: "after",
    content: { kind: "session", session: 3, chat: "3" },
  });
  tabs = openTab(tabs, 4, "4", "steward");
  return selectTab(tabs, tab);
}

function shape(layout: Layout): string {
  if (layout.kind === "pane")
    return layout.content.kind === "session" ? String(layout.content.session) : "-";
  return `${layout.direction}(${shape(layout.children[0])} ${shape(layout.children[1])})`;
}

function Window({
  first,
  heard,
  width = 800,
}: {
  first: Tabs;
  heard: (tabs: Tabs) => void;
  width?: number;
}) {
  const [tabs, setTabs] = useState(first);
  const now = useRef(first);
  const change = useCallback(
    (how: (tabs: Tabs) => Tabs) => {
      const next = how(now.current);
      now.current = next;
      setTabs(next);
      heard(next);
      return next;
    },
    [heard],
  );
  const hold = useCallback((id: number) => change((t) => selectTab(t, id)), [change]);
  const own = useCallback((session: number) => String(session), []);
  const drag = usePaneDrag({ tabs, change, ctx, onHoldTab: hold, chatNameOf: own });
  const front = tabs.inFront ?? tabs.order[0];
  const panes = contentsOf(tabs, front);
  return (
    <PlacingHere placing={drag.placing}>
      <div {...{ [STRIP]: "" }} data-box="0,0,1600,30">
        {tabs.order.map((id, at) => (
          <div key={id} {...{ [TAB]: String(id) }} data-box={`${at * 100},0,100,30`}>
            <span
              role="tab"
              tabIndex={0}
              aria-selected={id === tabs.inFront}
              onClick={() => change((t) => selectTab(t, id))}
            >{`tab ${id}`}</span>
          </div>
        ))}
      </div>
      {panes.map(({ pane }, at) => (
        <div key={pane} {...dropPane(front, pane)} data-box={`${at * width},40,${width},600`}>
          <div {...{ [GRIP]: "" }} data-box={`${at * width},40,${width},20`}>
            <PaneGrip tab={front} pane={pane} />
            {`head ${pane}`}
          </div>
          <textarea aria-label={`terminal ${pane}`} />
        </div>
      ))}
      <button type="button" {...{ [CHAT_ROW]: "7" }} data-box="0,700,200,20">
        chat 7
      </button>
      <button type="button">elsewhere</button>
      <button
        type="button"
        onClick={() =>
          change((t) => {
            const tab = t.byId[front];
            const left = without(tab.layout, panes[0].pane);
            if (left === undefined) return t;
            return {
              ...t,
              byId: { ...t.byId, [front]: { ...tab, layout: left, focused: panes[1].pane } },
            };
          })
        }
      >
        close the first pane
      </button>
      {drag.layer}
      {drag.undoLine}
    </PlacingHere>
  );
}

const boxOf = (el: Element) => {
  const [left, top, width, height] = (el.getAttribute("data-box") ?? "0,0,0,0")
    .split(",")
    .map(Number);
  return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top };
};

beforeEach(() => {
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    return { ...boxOf(this), toJSON: () => ({}) } as DOMRect;
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function shown(width?: number): { tabs: () => Tabs; first: Tabs } {
  const first = start();
  let last = first;
  render(<Window first={first} heard={(tabs) => (last = tabs)} width={width} />);
  return { tabs: () => last, first };
}

/** ⌘⇧M, or Ctrl+Shift+M off a Mac: the registry's own chord, on what has the keyboard. */
function pressPlace() {
  const chord = shortcut("pane.place").chords[onAMac() ? "mac" : "other"][0];
  act(() => {
    (document.activeElement ?? document.body).dispatchEvent(keystroke(chord));
  });
}

const said = () => document.querySelector("[data-pane-drag-said]")?.textContent ?? "";
const PICKED_1 =
  "Picked up chat 1. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.";

describe("placement mode: the keyboard route for every drag (#4764)", () => {
  it("picks a pane up on Enter on its grip, says so, and shows five targets on every other pane and a New tab", async () => {
    const user = userEvent.setup();
    shown();
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    const groups = screen.getAllByRole("group", { name: /^Places by / });
    expect(groups.map((g) => g.getAttribute("aria-label"))).toEqual(["Places by chat 3"]);
    expect(within(groups[0]).getAllByRole("button")).toHaveLength(5);
    expect(screen.getByRole("button", { name: "A new tab with chat 1." })).toBeTruthy();
    // The keyboard is on the middle of the first other pane.
    await waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-label")).toBe("Swap chat 1 and chat 3."),
    );
  });

  it("picks a pane up on Space on its grip, and on a single press of it", async () => {
    const user = userEvent.setup();
    shown();
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard(" ");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Move chat 3" }));
    await waitFor(() => expect(said()).toMatch(/^Picked up chat 3\./));
  });

  it("walks the targets with the arrows and Tab, each named by what dropping there would do", async () => {
    const user = userEvent.setup();
    shown();
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard("{Enter}");
    const focusedName = () => document.activeElement?.getAttribute("aria-label");
    await waitFor(() => expect(focusedName()).toBe("Swap chat 1 and chat 3."));
    await user.keyboard("{ArrowRight}");
    expect(focusedName()).toBe("chat 1 right of chat 3.");
    await user.keyboard("{ArrowLeft}{ArrowUp}");
    expect(focusedName()).toBe("chat 1 above chat 3.");
    await user.keyboard("{Tab}");
    expect(focusedName()).toBe("A new tab with chat 1.");
    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(focusedName()).toBe("Swap chat 1 and chat 3.");
  });

  it("drops on Enter by the pointer's own rules, says so, and offers the same Undo", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.keyboard("{ArrowDown}{Enter}");
    const to = {
      kind: "pane",
      tab: first.order[0],
      pane: contentsOf(first, first.order[0])[1].pane,
      place: "bottom",
    } as const;
    const verdict = judgeDrop(
      first,
      { kind: "pane", tab: first.order[0], pane: contentsOf(first, first.order[0])[0].pane },
      to,
      ctx,
    );
    expect(verdict.ok).toBe(true);
    expect(said()).toBe(`Dropped. ${verdict.ok ? verdict.did : ""}`);
    const expected = dropOn(
      first,
      { kind: "pane", tab: first.order[0], pane: contentsOf(first, first.order[0])[0].pane },
      to,
      ctx,
    );
    expect(shape(tabs().byId[first.order[0]].layout)).toBe(
      shape(expected.byId[first.order[0]].layout),
    );
    expect(shape(tabs().byId[first.order[0]].layout)).toBe("column(3 1)");
    expect(screen.queryAllByRole("group", { name: /^Places by / })).toEqual([]);
    expect(screen.getByRole("button", { name: "Undo" })).toBeTruthy();
    // The keyboard follows the pane it moved: its grip has it.
    await waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-label")).toBe("Move chat 1"),
    );
  });

  it("drops with a single press of a target", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    await user.click(screen.getByRole("button", { name: "Move chat 1" }));
    await user.click(await screen.findByRole("button", { name: "A new tab with chat 1." }));
    expect(tabs().order).toHaveLength(first.order.length + 1);
    expect(said()).toBe("Dropped. Made a new tab with chat 1.");
  });

  it("puts it back on Esc: nothing moves, it says so, and the grip has the keyboard again", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    const grip = screen.getByRole("button", { name: "Move chat 1" });
    grip.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.keyboard("{ArrowRight}{Tab}{Escape}");
    expect(said()).toBe("Put back. Nothing moved.");
    expect(tabs()).toBe(first);
    expect(screen.queryAllByRole("group", { name: /^Places by / })).toEqual([]);
    await waitFor(() => expect(document.activeElement).toBe(grip));
  });

  it("says why a refused target takes nothing, and stays picked up", async () => {
    const user = userEvent.setup();
    // Panes 400 wide: a split beside would leave halves under the floor.
    const { tabs, first } = shown(400);
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.keyboard("{ArrowRight}");
    expect(document.activeElement?.getAttribute("aria-label")).toBe("Right of chat 3");
    await user.keyboard("{Enter}");
    expect(said()).toBe(
      "Not dropped: Too narrow: a pane here would be under about 40 columns. Drop it on a wider pane.",
    );
    expect(tabs()).toBe(first);
    expect(screen.getAllByRole("group", { name: /^Places by / })).toHaveLength(1);
  });

  it("picks up the pane whose terminal has the keyboard on ⌘⇧M", async () => {
    shown();
    act(() => screen.getByRole("textbox", { name: "terminal 1" }).focus());
    pressPlace();
    await waitFor(() => expect(said()).toBe(PICKED_1));
  });

  it("picks up a sidebar chat on ⌘⇧M on its row, and Esc gives the row the keyboard back", async () => {
    const user = userEvent.setup();
    shown();
    const row = screen.getByRole("button", { name: "chat 7" });
    act(() => row.focus());
    pressPlace();
    await waitFor(() => expect(said()).toMatch(/^Picked up chat 7\./));
    // Every pane of the tab in front takes it: none of them shows it yet.
    expect(screen.getAllByRole("group", { name: /^Places by / })).toHaveLength(2);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(row));
  });

  it("picks up a tab on ⌘⇧M on it, which goes beside a pane of another tab", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    const second = first.order[1];
    act(() => screen.getByRole("tab", { name: `tab ${second}` }).focus());
    pressPlace();
    await waitFor(() => expect(said()).toMatch(/^Picked up chat 4\./));
    await user.keyboard("{ArrowRight}{Enter}");
    expect(tabs().order).toEqual([first.order[0]]);
    // The keyboard started on chat 1's middle: the tab's own pane goes right of it.
    expect(shape(tabs().byId[first.order[0]].layout)).toBe("row(row(1 4) 3)");
  });

  it("puts it back when the pointer presses anywhere but a target", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    await user.click(screen.getByRole("button", { name: "Move chat 1" }));
    await waitFor(() => expect(said()).toBe(PICKED_1));
    fireEvent.pointerDown(screen.getByRole("button", { name: "elsewhere" }), { button: 0 });
    expect(said()).toBe("Put back. Nothing moved.");
    expect(tabs()).toBe(first);
    expect(screen.queryAllByRole("group", { name: /^Places by / })).toEqual([]);
  });
});

describe("placement mode while the window changes under it (#4764)", () => {
  it("says a second refusal again, though its words are the same", async () => {
    const user = userEvent.setup();
    shown(400);
    screen.getByRole("button", { name: "Move chat 1" }).focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.keyboard("{ArrowRight}{Enter}");
    const first = document.querySelector("[data-pane-drag-said] > *");
    await user.keyboard("{Enter}");
    const second = document.querySelector("[data-pane-drag-said] > *");
    expect(second?.textContent).toMatch(/^Not dropped: Too narrow/);
    // A new node, which a screen reader reads as new words.
    expect(second).not.toBe(first);
  });

  it("offers the tab brought forward while a pane is held, with the keyboard on its first target", async () => {
    const user = userEvent.setup();
    const { tabs, first } = shown();
    await user.click(screen.getByRole("button", { name: "Move chat 1" }));
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.click(screen.getByRole("tab", { name: `tab ${first.order[1]}` }));
    // Pressing a tab on the strip brings it forward, as holding a drag over it does.
    expect(tabs().inFront).toBe(first.order[1]);
    expect(said()).toBe(PICKED_1);
    await waitFor(() =>
      expect(document.activeElement?.getAttribute("aria-label")).toBe("Swap chat 1 and chat 4."),
    );
    await user.keyboard("{Enter}");
    expect(said()).toBe("Dropped. Swapped chat 1 and chat 4.");
  });

  it("puts nothing back when the pane it holds closes, and says so", async () => {
    const user = userEvent.setup();
    shown();
    await user.click(screen.getByRole("button", { name: "Move chat 1" }));
    await waitFor(() => expect(said()).toBe(PICKED_1));
    fireEvent.click(screen.getByRole("button", { name: "close the first pane" }));
    await waitFor(() => expect(said()).toBe("Not placed: that pane has closed."));
    expect(screen.queryAllByRole("group", { name: /^Places by / })).toEqual([]);
  });

  it("shows a tab picked up in front no targets on its own panes, only the strip's", async () => {
    shown();
    act(() => screen.getByRole("tab", { name: /^tab /, selected: true }).focus());
    pressPlace();
    await waitFor(() => expect(said()).toMatch(/^Picked up chat 1\./));
    expect(screen.queryAllByRole("group", { name: /^Places by / })).toEqual([]);
    expect(screen.getByRole("button", { name: "Move the tab here." })).toBeTruthy();
  });

  it("picks another pane up on a press of its grip while one is held", async () => {
    const user = userEvent.setup();
    shown();
    await user.click(screen.getByRole("button", { name: "Move chat 1" }));
    await waitFor(() => expect(said()).toBe(PICKED_1));
    await user.click(screen.getByRole("button", { name: "Move chat 3" }));
    await waitFor(() => expect(said()).toMatch(/^Picked up chat 3\./));
    expect(
      screen
        .getAllByRole("group", { name: /^Places by / })
        .map((g) => g.getAttribute("aria-label")),
    ).toEqual(["Places by chat 1"]);
  });
});
