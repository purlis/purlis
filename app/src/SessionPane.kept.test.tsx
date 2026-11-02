// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { Terminal } from "@xterm/xterm";
import { SessionPane } from "./SessionPane";
import { DEFAULT_THEME, drawIn, xtermTheme } from "./theme/theme";
import { PURLIS_LIGHT } from "./theme/handed.testkit";

/**
 * **Switching back to one of the last three chats shown is instant** (#4735, part of #4724).
 *
 * A tab switch draws the pane again for another chat. It used to dispose the terminal, close
 * its view, and start a new terminal that opened a new view and was sent the screen again. The
 * last three chats shown keep theirs, off the window, so switching back to one starts nothing.
 *
 * Held by counts (ADR 0086): terminals built, and `watch_session` and `unwatch_session` sent.
 * The real xterm, wrapped only so the test can reach what the pane made.
 */
const made: Terminal[] = [];
vi.mock("@xterm/xterm", async (real) => {
  const xterm = await real<typeof import("@xterm/xterm")>();
  class Recorded extends xterm.Terminal {
    constructor(...args: ConstructorParameters<typeof xterm.Terminal>) {
      super(...args);
      made.push(this);
    }
  }
  return { ...xterm, Terminal: Recorded };
});

// The renderer loads WebGL code jsdom has no context for; the pane draws with the DOM meanwhile.
vi.mock("./renderer", async (real) => ({
  ...(await real<typeof import("./renderer")>()),
  draw: () => new Promise(() => {}),
}));

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const PLANE = "/planes/one";

type Output = { onmessage: (text: string) => void };
/** Each view the core opened, in order: whose, and the channel its output goes down. */
let views: { session: number; view: number; output: Output }[] = [];
/** Each view the window closed, by number. */
let closed: number[] = [];

beforeEach(() => {
  made.length = 0;
  views = [];
  closed = [];
  mockIPC((cmd, args) => {
    const given = args as { session: number; output: Output; view: number };
    if (cmd === "watch_session") {
      const view = views.length + 1;
      views.push({ session: given.session, view, output: given.output });
      return { view, columns: 80, rows: 24, scrollback: 5000, newline: null };
    }
    if (cmd === "unwatch_session") closed.push(given.view);
    return null;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

/** The pane in front, as a tab draws it: the same pane, drawn again for the chat shown. */
function window() {
  const shown = (session: number) => (
    <SessionPane plane={PLANE} session={session} focused={false} onFocus={() => {}} />
  );
  const drawn = render(shown(1));
  return { show: (session: number) => drawn.rerender(shown(session)) };
}

/** Waits until chat `session`'s view is open. */
const opened = (session: number) =>
  waitFor(() => expect(views.some((one) => one.session === session)).toBe(true));

/** What the terminal shows on its screen, line by line. */
function screenOf(terminal: Terminal): string {
  const buffer = terminal.buffer.active;
  return Array.from({ length: buffer.length }, (_, row) =>
    buffer.getLine(row)?.translateToString(true),
  ).join("\n");
}

describe("the last three chats shown keep their terminals", () => {
  it("starts no terminal and opens no view switching back to one of them", async () => {
    const { show } = window();
    await opened(1);
    show(2);
    await opened(2);
    show(3);
    await opened(3);

    show(1);
    show(2);
    show(3);

    expect(made).toHaveLength(3);
    expect(views).toHaveLength(3);
    expect(closed).toEqual([]);
  });

  it("lets go of the least recently shown when a fourth is shown", async () => {
    const { show } = window();
    for (const session of [1, 2, 3]) {
      show(session);
      await opened(session);
    }
    // The second was shown again, so the first is the least recently shown.
    show(2);
    show(3);

    show(4);
    await opened(4);

    expect(closed).toEqual([views.find((one) => one.session === 1)?.view]);
    // Shown again, it is a terminal of its own, sent the screen as it is.
    show(1);
    await waitFor(() => expect(views.filter((one) => one.session === 1)).toHaveLength(2));
    expect(made).toHaveLength(5);
  });

  it("shows its current screen on return, having taken what was written while away", async () => {
    const { show } = window();
    await opened(1);
    const first = views[0];
    first.output.onmessage("before\r\n");
    show(2);
    await opened(2);

    first.output.onmessage("while away\r\n");
    await waitFor(() => expect(screenOf(made[0])).toContain("while away"));
    show(1);

    expect(made).toHaveLength(2);
    expect(views).toHaveLength(2);
    // The very terminal that took it is the one on screen.
    expect(screen.getByTestId("pane").contains(made[0].element ?? null)).toBe(true);
    expect(screenOf(made[0])).toContain("before");
  });

  it("comes back in a theme drawn while it was away", async () => {
    const { show } = window();
    await opened(1);
    show(2);
    await opened(2);

    act(() => drawIn(PURLIS_LIGHT));
    show(1);

    expect(made[0].options.theme).toEqual(xtermTheme(PURLIS_LIGHT));
    act(() => drawIn(DEFAULT_THEME));
  });
});
