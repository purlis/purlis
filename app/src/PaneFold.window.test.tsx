// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import type { Shown } from "./bindings";
import { LayoutPanes } from "./LayoutPanes";
import { PaneRail } from "./PaneRail";
import { RAIL } from "./paneHeld";
import { namedWithoutItsWords } from "./a11y.testkit";
import type { Content, Layout } from "./tabs";
import { ChatMark } from "./ui/chat-mark/ChatMark";

/**
 * **Narrow windows fold panes to rails, as `LayoutPanes` draws them** (#4766, spec #4750 stories
 * 81 to 84). The room is handed in, since jsdom lays nothing out; the panes that keep the room
 * are empty ones, so only what an empty pane, a split and a rail read is handed in.
 */

afterEach(cleanup);

const offer = (id: string, title: string): Offer => ({
  id,
  title,
  available: true,
  reason: "",
  does: { verb: "split", direction: id === "pane.split.right" ? "row" : "column" },
});

const SPLITS: Record<string, Offer> = {
  "pane.split.right": offer("pane.split.right", "Split right"),
  "pane.split.down": offer("pane.split.down", "Split down"),
};

const empty = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "empty", workspace: "ops" },
});

const chat = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "session", session: pane, chat: `chat ${pane}` },
});

const row = (...sides: Layout[]): Layout =>
  sides.reduceRight((rest, one) => ({
    kind: "split",
    direction: "row",
    children: [one, rest],
    share: 333,
  }));

const column = (...sides: Layout[]): Layout =>
  sides.reduceRight((rest, one) => ({ kind: "split", direction: "column", children: [one, rest] }));

/** Chat `session` asks the person something, as the registry lists it. */
const asking = (session: number): Shown =>
  ({
    session,
    ask: `tool:${session}`,
    says: "Run the migration?",
    options: [],
    source: "hook",
    chain: [`chat ${session}`],
    answer: { via: "hook" },
    since: 1,
  }) as unknown as Shown;

type Props = Parameters<typeof LayoutPanes>[0];

function propsOf(layout: Layout, more: Partial<Props>): Props {
  const read = {
    plane: "/home/op/project",
    layout,
    tab: 1,
    focused: 1,
    onFocus: vi.fn(),
    offerFor: (id: string) => SPLITS[id],
    closeOf: () => undefined,
    onPaneDoes: vi.fn(),
    runningIn: () => 0,
    split: { of: () => undefined, moved: vi.fn(), shared: vi.fn() },
    waiting: [],
    ...more,
  };
  return read as unknown as Props;
}

const rails = () => screen.queryAllByRole("group", { name: "Folded panes" });

describe("a split below the minimum (#4766)", () => {
  it("folds the side without the focus to a rail, and draws the other whole", () => {
    render(
      <LayoutPanes {...propsOf(row(empty(1), chat(2)), { room: { width: 500, height: 600 } })} />,
    );

    expect(rails()).toHaveLength(1);
    expect(screen.getByRole("button", { name: "chat 2, folded" })).toBeInTheDocument();
    expect(screen.getByText("Nothing in this pane yet")).toBeInTheDocument();
  });

  it("marks a rail as no place a drop lands in, for a drag to say why over it (#5415)", () => {
    render(
      <LayoutPanes {...propsOf(row(empty(1), chat(2)), { room: { width: 500, height: 600 } })} />,
    );

    expect(rails()[0].hasAttribute(RAIL)).toBe(true);
    expect(rails()[0].closest("[data-drop-pane]")).toBeNull();
  });

  it("opens a folded pane on its press, with the keyboard, which folds the other side instead", async () => {
    // The rail's button goes as the other side folds, so the press is the window's to give the
    // keyboard to the pane it opened (`onOpenFolded`), not a focus the pane merely reports.
    const onOpenFolded = vi.fn();
    const layout = row(empty(1), empty(2));
    const props = propsOf(layout, { room: { width: 500, height: 600 }, onOpenFolded });
    const { rerender } = render(<LayoutPanes {...props} />);
    const before = structuredClone(layout);

    await userEvent.click(within(rails()[0]).getByRole("button", { name: "New chat, folded" }));
    expect(onOpenFolded).toHaveBeenCalledWith(2);

    rerender(<LayoutPanes {...props} focused={2} />);
    expect(rails()).toHaveLength(1);
    expect(document.querySelector('.pane-rail [data-pane="1"]')).not.toBeNull();
    // Drawn only: the tree, and its shares, are as they were, and no share was said.
    expect(layout).toEqual(before);
    expect(props.split.shared).not.toHaveBeenCalled();
  });

  it("names a folded view as its pane is titled", () => {
    const view: Layout = {
      kind: "pane",
      pane: 2,
      content: { kind: "view", view: { from: null, view: "saving", key: "" }, workspace: "ops" },
    };
    render(
      <LayoutPanes
        {...propsOf(row(empty(1), view), { room: { width: 500, height: 600 }, name: "Saving" })}
      />,
    );

    expect(screen.getByRole("button", { name: "Saving, folded" })).toBeInTheDocument();
  });

  it("folds to a bar when stacked, which names each pane", () => {
    render(
      <LayoutPanes
        {...propsOf(column(empty(1), chat(2)), { room: { width: 800, height: 200 } })}
      />,
    );

    expect(rails()[0]).toHaveAttribute("data-fold", "bar");
    expect(within(rails()[0]).getByText("chat 2")).toBeInTheDocument();
  });

  it("folds nothing where the room is not known yet", () => {
    render(<LayoutPanes {...propsOf(row(empty(1), empty(2)), {})} />);

    expect(rails()).toHaveLength(0);
    expect(screen.getAllByText("Nothing in this pane yet")).toHaveLength(2);
  });
});

describe("a 3 × 3 grid at 770px (#4766)", () => {
  // Tile all's grid: three rows, each a | (b | c). The first column is empty panes.
  const grid = column(
    row(empty(1), chat(2), chat(3)),
    row(empty(4), chat(5), chat(6)),
    row(empty(7), chat(8), chat(9)),
  );

  it("keeps one column per row and rails the rest", () => {
    render(<LayoutPanes {...propsOf(grid, { room: { width: 770, height: 600 } })} />);

    expect(rails()).toHaveLength(3);
    expect(rails().map((one) => within(one).getAllByRole("button").length)).toEqual([2, 2, 2]);
    expect(screen.getAllByText("Nothing in this pane yet")).toHaveLength(3);
  });

  it("puts a folded pane's ask on its row's rail, as ✋", () => {
    render(
      <LayoutPanes
        {...propsOf(grid, { room: { width: 770, height: 600 }, waiting: [asking(6)] })}
      />,
    );

    const second = rails()[1];
    const asks = within(second).getByRole("button", { name: "chat 6, folded, asks you" });
    expect(
      asks.closest(".pane-rail-item")?.querySelector('[data-mark="needs-you"]'),
    ).not.toBeNull();
    expect(within(second).getByRole("button", { name: "chat 5, folded" })).toBeInTheDocument();
  });

  it("draws one ✋ for a folded chat that needs you, its mark's, never a second beside it (#5447)", () => {
    // A chat's face as the window gives it: its mark, which wears its one state (#5340).
    const faceOf = (content: Content) =>
      content.kind !== "session"
        ? undefined
        : {
            name: content.chat,
            mark: <ChatMark state={content.session === 6 ? "needs" : "working"} />,
            stated: true,
          };
    render(
      <LayoutPanes
        {...propsOf(grid, { room: { width: 770, height: 600 }, waiting: [asking(6)], faceOf })}
      />,
    );

    const asks = within(rails()[1]).getByRole("button", { name: "chat 6, folded, asks you" });
    const item = asks.closest(".pane-rail-item") as HTMLElement;
    expect(item.querySelectorAll('[data-mark="needs-you"]')).toHaveLength(1);
    expect(item.querySelector(".pane-rail-asks")).toBeNull();
  });
});

describe("Split right and Split down below the minimum (#4766)", () => {
  it("grey out on a pane too narrow to split, with their reason, and never switch direction", () => {
    render(<LayoutPanes {...propsOf(empty(1), { room: { width: 500, height: 600 } })} />);

    expect(screen.getByRole("button", { name: /^Split right/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("button", { name: /^Split down/ })).not.toHaveAttribute(
      "aria-disabled",
    );
  });

  it("split where the pane is not measured yet", () => {
    render(<LayoutPanes {...propsOf(empty(1), {})} />);

    expect(screen.getByRole("button", { name: /^Split right/ })).not.toHaveAttribute(
      "aria-disabled",
    );
  });
});

describe("a folded pane's name (#4766)", () => {
  it("is its own words, so what is seen is what is heard", () => {
    const { container } = render(
      <PaneRail
        as="bar"
        onOpen={vi.fn()}
        panes={[
          {
            pane: 2,
            name: "devops 2",
            mark: null,
            holds: { notes: 3, trouble: 1, asks: 0 },
            asks: true,
          },
        ]}
      />,
    );

    expect(
      screen.getByRole("button", { name: "devops 2, 3 notices, folded, asks you" }),
    ).toBeInTheDocument();
    // Label in Name, asked of the button itself: axe's rule cannot fail in jsdom (#6371).
    expect(namedWithoutItsWords(container)).toEqual([]);
  });
});
