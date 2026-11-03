// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import { LayoutPanes } from "./LayoutPanes";
import type { Layout } from "./tabs";

/**
 * **A pane that shows nothing, and a tab's own splits, as `LayoutPanes` draws them** (#4755).
 * Only empty panes are drawn here, so only what an empty pane and a split read is handed in; a
 * chat's pane and a view's are the window tests' (`*.window.test.tsx`).
 */

afterEach(cleanup);

const NEW_CHAT: Offer = {
  id: "chat.new",
  title: "New tab",
  available: true,
  reason: "",
  does: { verb: "chat.new" },
};

const empty = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "empty", workspace: "ops" },
});

const split: Layout = { kind: "split", direction: "row", children: [empty(1), empty(2)] };

type Props = Parameters<typeof LayoutPanes>[0];

/** What an empty pane and a split read. The rest is a chat's or a view's, and never read here. */
function propsOf(layout: Layout, tab: number, onPaneDoes: Props["onPaneDoes"] = vi.fn()): Props {
  const read = {
    plane: "/home/op/project",
    layout,
    tab,
    focused: 1,
    onFocus: vi.fn(),
    offerFor: (id: string) => (id === "chat.new" ? NEW_CHAT : undefined),
    closeOf: () => undefined,
    onPaneDoes,
    runningIn: (workspace: string) => (workspace === "ops" ? 3 : 0),
    split: { of: () => undefined, moved: vi.fn() },
  };
  return read as unknown as Props;
}

describe("a pane that shows nothing (#4755)", () => {
  it("says so, how many chats run in its workspace, and offers New chat on the pane", async () => {
    const onPaneDoes = vi.fn();
    render(<LayoutPanes {...propsOf(empty(1), 1, onPaneDoes)} />);

    expect(screen.getByText("Nothing in this pane")).toBeInTheDocument();
    expect(screen.getByText("3 chats are running here.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "New chat" }));

    expect(onPaneDoes).toHaveBeenCalledWith(1, NEW_CHAT);
  });

  it("is named apart as one side of a split", () => {
    render(<LayoutPanes {...propsOf(split, 1)} />);

    expect(screen.getAllByText("An empty pane")).toHaveLength(2);
  });
});

describe("a tab's splits are its own (#4755)", () => {
  // The library reads a side's size once, as it mounts: a split kept across a tab switch would
  // draw the last tab's sizes. Its drawn sizes are for real hardware (jsdom lays nothing out).
  const group = () => document.querySelector("[data-group]");

  it("draws another tab's split as a new one, never the last tab's carried over", () => {
    const { rerender } = render(<LayoutPanes {...propsOf(split, 1)} />);
    const first = group();

    rerender(<LayoutPanes {...propsOf(split, 2)} />);

    expect(group()).not.toBe(first);
  });

  it("keeps the same tab's split as it is, so a drag in it is not undone", () => {
    const { rerender } = render(<LayoutPanes {...propsOf(split, 1)} />);
    const first = group();

    rerender(<LayoutPanes {...propsOf(split, 1)} />);

    expect(group()).toBe(first);
  });
});
