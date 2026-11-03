// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render as renderBare, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { Moved, OpenChat } from "./bindings";
import { forgetKeyboard } from "./paneKeyboard";
import { GRIP } from "./paneDrag";
import { stripNamed } from "./test-strips";

/**
 * **A terminal editor is a one-job program** (#4761; spec #4750 story 66, ADR 0081 as amended
 * 2026-11-02): opened from a file, it opens as a split beside the focused pane, in the same tab
 * and with focus, so what the person was reading stays on screen. Its pane has no grip and no
 * close (story 78). When the editor exits its split goes, focus goes back where it was, and the
 * core is told to close it. The whole window, because it is the core's event, the layout and
 * the pane agreeing.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);
const PLANE = "/home/dev/plane";

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

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

/** The core, holding `open` chats; what it was asked, and a way to say an event. */
function core(chats: OpenChat[]) {
  const listeners = new Map<string, number[]>();
  const closed: number[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "plugin:event|listen") {
      const { event, handler } = args as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "opened_chats") return chats;
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        personas: [],
        persona: null,
        unfiled: [],
        workspaces: [
          {
            name: "ide",
            path: `${PLANE}/workspaces/ide`,
            vision: "",
            todos: [],
            chats: chats.filter((one) => !closed.includes(one.session)),
          },
        ],
      };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "close_session") {
      closed.push((args as { session: number }).session);
      return null;
    }
    return null;
  });
  const said = async (event: string, payload: unknown) => {
    const handlers = listeners.get(event) ?? [];
    if (handlers.length === 0) throw new Error(`the window is not listening for ${event}`);
    await act(async () => {
      for (const handler of handlers)
        window.__TAURI_INTERNALS__.runCallback(handler, { event, id: 1, payload });
      await Promise.resolve();
    });
  };
  return {
    closed,
    /** The core opened a terminal editor as chat `session`. */
    opened: (session: number) =>
      said("editor-tab-opened", { plane: PLANE, session, name: "vim lib.rs", workspace: "ide" }),
    /** Chat `session`'s program exited. */
    exited: (session: number) =>
      said("chat-moved", {
        plane: PLANE,
        session,
        state: "done",
        needs_you: false,
        queue: [],
        moved_at: 9,
        sequence: 9,
        reports: [],
        refusals: [],
        children: [],
      } satisfies Moved),
  };
}

const tabNames = () =>
  within(stripNamed("Tabs"))
    .getAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);
/** The frame of the pane drawing chat `session`. */
const frameOf = (session: number) =>
  Array.from(document.querySelectorAll<HTMLElement>(".pane-frame")).find(
    (frame) => frame.querySelector("[data-testid=pane]")?.textContent === `session ${session}`,
  );
const focusedSession = () =>
  document.querySelector(".pane-frame[data-focused] [data-testid=pane]")?.textContent;

beforeEach(forgetKeyboard);
afterEach(() => {
  cleanup();
  clearMocks();
});

describe("a terminal editor opened from a file", () => {
  it("opens beside the focused pane, in the same tab, with focus", async () => {
    const { opened } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));

    await opened(2);

    await waitFor(() => expect(frameOf(2)).toBeDefined());
    expect(tabNames()).toEqual(["ide.1"]);
    expect(frameOf(1)).toBeDefined();
    expect(focusedSession()).toBe("session 2");
  });

  it("has no grip and no close on its pane, while the pane it opened beside keeps both", async () => {
    const { opened } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    await opened(2);
    await waitFor(() => expect(frameOf(2)).toBeDefined());

    const job = frameOf(2)!;
    expect(job.querySelector(`[${GRIP}]`)).toBeNull();
    expect(within(job).queryByRole("button", { name: /^(Close|End) this pane/ })).toBeNull();
    expect(frameOf(1)!.querySelector(`[${GRIP}]`)).not.toBeNull();
  });

  it("goes when the editor exits: its split goes, focus goes back, and the core closes it", async () => {
    const { opened, exited, closed } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(tabNames()).toEqual(["ide.1"]));
    await opened(2);
    await waitFor(() => expect(focusedSession()).toBe("session 2"));

    await exited(2);

    await waitFor(() => expect(frameOf(2)).toBeUndefined());
    expect(focusedSession()).toBe("session 1");
    expect(tabNames()).toEqual(["ide.1"]);
    await waitFor(() => expect(closed).toEqual([2]));
  });
});

/**
 * **A shell whose program exits leaves an empty pane, and no ended band** (#4761; spec #4750
 * story 64, ADR 0062 as amended 2026-11-02): it is not a chat to Resume. Its row goes, since the
 * core closes it, and no band is drawn on the way, not even for a frame.
 */
describe("a shell whose program exits", () => {
  it("leaves its pane empty, draws no ended band, and the core closes it", async () => {
    const { exited, closed } = core([chat(1)]);
    render(<App />);
    await waitFor(() => expect(frameOf(1)).toBeDefined());
    const bands: string[] = [];
    const watching = new MutationObserver(() => {
      for (const one of document.querySelectorAll("[role=status]"))
        if (/ended/i.test(one.textContent ?? "")) bands.push(one.textContent ?? "");
    });
    watching.observe(document.body, { childList: true, subtree: true, characterData: true });

    await exited(1);
    await act(() => new Promise((done) => setTimeout(done, 50)));

    watching.disconnect();
    expect(bands).toEqual([]);
    expect(frameOf(1)).toBeUndefined();
    await waitFor(() => expect(closed).toEqual([1]));
    expect(tabNames()).toHaveLength(1);
  });
});
