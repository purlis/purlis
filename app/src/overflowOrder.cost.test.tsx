// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Moved, OpenChat } from "./bindings";
import App from "./App";
import { counts, drawnOutside } from "./reactCommits.testkit";
import { measuring, type Room } from "./stripRoom.testkit";

/**
 * **A busy chat the strip is not drawing costs the window nothing but its overflow** (#4726).
 *
 * The show-more menu lists the tabs the strip has no room for, most recently moved first (ADR
 * 0039). The order was read at the top of the project view, so every `chat-moved` of a hidden
 * chat that changed it drew the whole view again: with fifty busy chats, most of them hidden,
 * that is the project view drawn again on nearly every move. The order is read where the
 * overflow is drawn, and the strip holds only how many it hides, which a move does not change.
 *
 * **Held by what React drew, named**, in the real window (ADR 0086, as the project-switch cost
 * test holds a switch): a move of a hidden chat draws nothing outside the overflow.
 */

await vi.hoisted(async () => {
  const { countReactsCommits } = await import("./reactCommits.testkit");
  countReactsCommits();
});

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const PLANE = "/home/dev/plane";

function chat(session: number): OpenChat {
  return {
    session,
    name: `ide.${session}`,
    cwd: `${PLANE}/workspaces/ide`,
    harness: null,
    in_front: session === 1,
    resumed: null,
    fresh: null,
    profile: null,
    persona: null,
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
  };
}

/** A core holding `open` chats and no sidebar, so every chat is on one strip. */
function core(open: OpenChat[]): (moved: Moved) => void {
  const listeners = new Map<string, number>();
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, handler);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return open;
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    return null;
  });
  return (moved) => {
    const handler = listeners.get("chat-moved");
    if (handler === undefined) throw new Error("the window is not listening for moves");
    act(() => {
      window.__TAURI_INTERNALS__.runCallback(handler, {
        event: "chat-moved",
        id: 1,
        payload: moved,
      });
    });
  };
}

/** A busy chat's move: still running, and later than before. */
function running(session: number, at: number): Moved {
  return {
    plane: PLANE,
    session,
    state: "running",
    needs_you: false,
    queue: [],
    moved_at: at,
    sequence: at,
    reports: [],
    refusals: [],
    children: [],
  };
}

/** What the show-more menu lists, top to bottom. */
const menuNames = () =>
  screen.getAllByRole("menuitem").map((row) => row.querySelector(".tab-name")?.textContent);

describe("a move of a chat the strip is not drawing", () => {
  let room: Room;

  beforeEach(() => {
    room = measuring();
  });

  afterEach(() => {
    room.stop();
    cleanup();
    clearMocks();
  });

  it("draws nothing outside the overflow, and the overflow takes the new order", async () => {
    const move = core([chat(1), chat(2), chat(3), chat(4)]);
    render(<App />);
    await waitFor(() => expect(screen.getAllByTestId("pane")).toHaveLength(1));
    room.roomFor(1);
    await screen.findByRole("button", { name: "Show 3 tabs the strip is not showing" });
    // Three busy chats behind the edge, 4 the latest of them to have moved. The chat in front
    // moved last of all: the project's latest move, which the window is told of apart (#4727),
    // is not what this move changes.
    move(running(2, 10));
    move(running(3, 11));
    move(running(4, 12));
    move(running(1, 100));

    const drawn = drawnOutside("HiddenTabs");
    counts.rendered = 0;
    // Chat 2 moves again, from the bottom of the menu to the top of it.
    move(running(2, 13));

    expect(Object.fromEntries(drawn)).toEqual({});
    // And it was the overflow that took the move, not nothing at all.
    expect(counts.rendered).toBeGreaterThan(0);
    await userEvent.click(
      screen.getByRole("button", { name: "Show 3 tabs the strip is not showing" }),
    );
    expect(menuNames()).toEqual(["ide.2", "ide.4", "ide.3"]);
  });
});
