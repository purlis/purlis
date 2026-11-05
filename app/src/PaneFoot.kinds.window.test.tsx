// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Offer } from "./actions";
import { LayoutPanes } from "./LayoutPanes";
import type { Layout } from "./tabs";

/**
 * **Which panes draw a foot, as `LayoutPanes` draws them** (#5205, spec #5191 stories 20, 25 and
 * 26): a chat's does; a folded one, a view's and an empty one do not, and one that unfolds has its
 * foot back. The room is handed in, since jsdom lays nothing out.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => <div>session {session}</div>,
}));
vi.mock("./Views", async (real) => ({
  ...(await real<typeof import("./Views")>()),
  ViewPane: () => <div>a view</div>,
}));

beforeEach(() => {
  mockIPC(() => null, { shouldMockEvents: true });
});
afterEach(async () => {
  cleanup();
  // A listener still being set up lets go once it is, which the mocks must be there for.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});

const chat = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "session", session: pane, chat: `chat ${pane}` },
});
const empty = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "empty", workspace: "ops" },
});
const view = (pane: number): Layout => ({
  kind: "pane",
  pane,
  content: { kind: "view", view: { from: null, view: "saving", key: "" }, workspace: "ops" },
});
const row = (one: Layout, other: Layout): Layout => ({
  kind: "split",
  direction: "row",
  children: [one, other],
  share: 500,
});

type Props = Parameters<typeof LayoutPanes>[0];

function propsOf(layout: Layout, more: Partial<Props> = {}): Props {
  const read = {
    plane: "/home/op/project",
    layout,
    tab: 1,
    focused: 1,
    onFocus: vi.fn(),
    offerFor: () => undefined,
    closeOf: () => undefined,
    onPaneDoes: vi.fn(),
    runningIn: () => 0,
    split: { of: () => undefined, moved: vi.fn(), shared: vi.fn() },
    waiting: [],
    asks: [],
    crumbs: {},
    placed: {},
    endedTasks: {},
    endedChats: new Map(),
    unread: new Set(),
    finished: {},
    others: {},
    asked: {},
    byHand: {},
    startNotes: {},
    blocks: {},
    restartsSaid: {},
    heads: () => undefined,
    glances: {},
    handedFrom: {},
    workItems: {},
    shells: new Set(),
    restoring: new Set(),
    offers: new Map(),
    memory: {},
    chatName: () => "chat",
    faceOf: () => ({ name: "chat", icon: null }),
    ...more,
  };
  return read as unknown as Props;
}

const offer = (id: string, title: string) =>
  ({ id, title, available: true, reason: "", does: { verb: "closePane" } }) as Offer;
const SPLITS = (id: string) =>
  ({
    "pane.split.right": offer("pane.split.right", "Split right"),
    "pane.split.down": offer("pane.split.down", "Split down"),
  })[id];
const CLOSE = offer("pane.close", "Close this pane");

const feet = () => document.querySelectorAll('[data-scope="pane-foot"][data-part="root"]');

describe("a foot, for each kind of pane (#5205)", () => {
  it("is drawn on a chat's pane, under its terminal", () => {
    render(<LayoutPanes {...propsOf(chat(1))} />);

    expect(screen.getByText("session 1")).toBeInTheDocument();
    expect(feet()).toHaveLength(1);
  });

  it("is not drawn on a view's pane or an empty pane, split beside a chat's or not", () => {
    render(
      <LayoutPanes {...propsOf(row(view(1), empty(2)), { room: { width: 1600, height: 900 } })} />,
    );

    expect(screen.getByText("a view")).toBeInTheDocument();
    expect(feet()).toHaveLength(0);
  });

  it("groups a view's head as its controls alone, with no divider, whatever its width (#5203's window test)", () => {
    render(<LayoutPanes {...propsOf(view(1), { offerFor: SPLITS, closeOf: () => CLOSE })} />);

    const head = document.querySelector(".pane-head .pane-actions") as HTMLElement;
    expect(
      [...head.querySelectorAll("button")].map((one) => one.getAttribute("aria-label")),
    ).toEqual(["Split right", "Split down", "Close this pane"]);
    expect(head.querySelector('[data-scope="separator"]')).toBeNull();
  });

  it("draws a view alone in its tab with no splits or close, and one in a split with all three (#5318)", () => {
    render(
      <LayoutPanes
        {...propsOf(view(1), { alone: true, offerFor: SPLITS, closeOf: () => CLOSE })}
      />,
    );
    expect(screen.queryByRole("button", { name: /^Split |^Close / })).toBeNull();
    expect(document.querySelector(".pane-actions")).toBeNull();
    cleanup();

    render(
      <LayoutPanes
        {...propsOf(row(view(1), chat(2)), {
          alone: false,
          offerFor: SPLITS,
          closeOf: () => CLOSE,
          room: { width: 1600, height: 900 },
        })}
      />,
    );
    const frame = screen.getByText("a view").closest(".pane-frame") as HTMLElement;
    expect(
      [...frame.querySelectorAll(".pane-actions button")].map((one) =>
        one.getAttribute("aria-label"),
      ),
    ).toEqual(["Split right", "Split down", "Close this pane"]);
  });

  it("draws an empty pane or a chat alone with its two splits and no Close, nothing to Tab through (#5318)", () => {
    for (const lone of [empty(1), chat(1)]) {
      render(
        <LayoutPanes {...propsOf(lone, { alone: true, offerFor: SPLITS, closeOf: () => CLOSE })} />,
      );
      const buttons = [...document.querySelectorAll(".pane-doing button")].map((one) =>
        one.getAttribute("aria-label"),
      );
      expect(buttons).toEqual(["Split right", "Split down"]);
      cleanup();
    }
  });

  it("draws a shell's Close in a split, and none alone (#5318)", () => {
    const close = () => screen.queryByRole("button", { name: "Close this pane" });
    const shells = new Set([1]);
    const more = { offerFor: SPLITS, closeOf: () => CLOSE, shells };
    render(<LayoutPanes {...propsOf(chat(1), { ...more, alone: true })} />);
    expect(close()).toBeNull();
    cleanup();

    render(<LayoutPanes {...propsOf(chat(1), { ...more, alone: false })} />);
    expect(close()).toBeInTheDocument();
  });

  it("draws no Close on a one-job program's pane beside a chat, as it goes when its program exits (#5318)", () => {
    const oneJob = {
      kind: "pane",
      pane: 1,
      content: { kind: "session", session: 1, chat: "job", oneJob: { title: "Log in" } },
    } as unknown as Layout;
    render(
      <LayoutPanes
        {...propsOf(row(oneJob, chat(2)), {
          offerFor: SPLITS,
          closeOf: () => CLOSE,
          room: { width: 1600, height: 900 },
        })}
      />,
    );
    const frameOf = (words: string) =>
      screen.getByText(words).closest(".pane-frame") as HTMLElement;
    expect(frameOf("session 1").querySelectorAll('[aria-label="Close this pane"]')).toHaveLength(0);
    expect(frameOf("session 2").querySelectorAll('[aria-label="Close this pane"]')).toHaveLength(1);
  });

  it("is not drawn on a one-job program's pane", () => {
    const oneJob: Layout = {
      kind: "pane",
      pane: 1,
      content: { kind: "session", session: 1, chat: "job", oneJob: { title: "Log in" } },
    } as unknown as Layout;
    render(<LayoutPanes {...propsOf(oneJob)} />);

    expect(feet()).toHaveLength(0);
  });

  it("goes with a pane that folds to a rail and comes back as it unfolds (stories 25 and 26)", () => {
    const layout = row(chat(1), chat(2));
    const { rerender } = render(
      <LayoutPanes {...propsOf(layout, { room: { width: 1600, height: 900 } })} />,
    );
    expect(feet()).toHaveLength(2);

    rerender(<LayoutPanes {...propsOf(layout, { room: { width: 500, height: 900 } })} />);
    expect(screen.getByRole("group", { name: "Folded panes" })).toBeInTheDocument();
    expect(feet()).toHaveLength(1);

    rerender(<LayoutPanes {...propsOf(layout, { room: { width: 1600, height: 900 } })} />);
    expect(feet()).toHaveLength(2);
  });
});
