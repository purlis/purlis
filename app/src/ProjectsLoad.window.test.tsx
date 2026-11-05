// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ProjectReady } from "./bindings";
import { stripNamed } from "./test-strips";
import { themeInForce } from "./theme/inForce.testkit";

/**
 * **Projects load independently at a launch** (#5323, the spec #5319): every remembered project
 * has its row on the rail at once, marked as loading; the one in front is asked first, at most
 * four are opening at a time, and one the person chooses while it waits goes next; the rows keep
 * their order; each row clears its mark when the core says its project is back (`project-ready`,
 * #5322), says "N of M chats back" when only some came back, and "Still loading…" after 15 s.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

declare global {
  interface Window {
    __TAURI_INTERNALS__: { runCallback: (id: number, payload: unknown) => void };
  }
}

const at = (name: string) => `/home/dev/${name}`;

/**
 * A core that remembers `remembered` from the last quit, `active` in front, and answers each
 * project's open only when the test says so (`answer`), unless it is in `quick`.
 */
function core(remembered: string[], active: number, quick: string[] = []) {
  const asked: string[] = [];
  const opens = new Map<string, () => void>();
  const listeners = new Map<string, number[]>();
  mockIPC((cmd, args) => {
    const given = (args ?? {}) as Record<string, unknown>;
    if (cmd === "plugin:event|listen") {
      const { event, handler } = given as unknown as { event: string; handler: number };
      listeners.set(event, [...(listeners.get(event) ?? []), handler]);
      return 1;
    }
    const plane = given.plane as string | undefined;
    if (cmd === "plane_at_launch") return { plane: null, from: null, why: null };
    if (cmd === "planes_to_restore")
      return {
        windows: [{ planes: remembered.map(at), active }],
        dropped: [],
        gone: [],
      };
    if (cmd === "open_plane") {
      const path = given.path as string;
      asked.push(path);
      if (quick.map(at).includes(path)) return { plane: path, ask: null };
      return new Promise((resolve) => opens.set(path, () => resolve({ plane: path, ask: null })));
    }
    if (cmd === "plane_sidebar")
      return { root: plane, workspaces: [], personas: [], persona: null, unfiled: [] };
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    if (cmd === "theme_in_force") return themeInForce(args);
    return null;
  });
  return {
    /** The paths the window asked the core to open, in the order it asked. */
    asked,
    /** Answers the open of the project called `name`. */
    answer: async (name: string) => {
      const open = opens.get(at(name));
      if (open === undefined) throw new Error(`${name} was not asked to open`);
      await act(async () => open());
    },
    /** The core saying how the project called `name` came out (#5322). */
    ready: (
      name: string,
      outcome: ProjectReady["outcome"] = { kind: "ready", back: 1, wanted: 1 },
    ) =>
      act(() => {
        for (const handler of listeners.get("project-ready") ?? [])
          window.__TAURI_INTERNALS__.runCallback(handler, {
            event: "project-ready",
            id: 1,
            payload: { plane: at(name), outcome } satisfies ProjectReady,
          });
      }),
  };
}

/** The rail's projects in its order, `…` after each that is loading. */
const rows = () =>
  within(stripNamed("Projects"))
    .getAllByRole("tab")
    .map(
      (tab) =>
        `${tab.getAttribute("aria-label")}${tab.getAttribute("aria-busy") === "true" ? "…" : ""}`,
    );

const row = (name: string) => within(stripNamed("Projects")).getByRole("tab", { name });

describe("projects loading independently at a launch (#5323)", () => {
  it("draws every project's row at once, and each is ready on its own while one is slow", async () => {
    const { answer, ready } = core(["one", "two", "three"], 0);

    render(<App />);

    // `one`'s open has not answered, and the others are on the rail all the same.
    await waitFor(() => expect(rows()).toEqual(["one…", "two…", "three…"]));
    await answer("two");
    await answer("three");
    ready("two");
    ready("three");
    await waitFor(() => expect(rows()).toEqual(["one…", "two", "three"]));

    await answer("one");
    ready("one");
    await waitFor(() => expect(rows()).toEqual(["one", "two", "three"]));
  });

  it("asks the front project first, no more than four at once, and a chosen waiting one next", async () => {
    const { asked, answer } = core(["a", "b", "c", "d", "e", "f"], 2);

    render(<App />);

    await waitFor(() => expect(asked).toEqual(["c", "a", "b", "d"].map(at)));
    fireEvent.click(row("f"));
    await answer("a");
    await waitFor(() => expect(asked).toEqual(["c", "a", "b", "d", "f"].map(at)));
    // And once it is open, the project chosen is the one in front.
    await answer("f");
    await waitFor(() => expect(row("f")).toHaveAttribute("aria-selected", "true"));
  });

  it("keeps the rows in their order whichever project answers first", async () => {
    const { answer } = core(["one", "two", "three"], 0);

    render(<App />);

    await waitFor(() => expect(rows()).toHaveLength(3));
    await answer("three");
    await answer("two");
    await answer("one");
    await waitFor(() =>
      expect(
        within(stripNamed("Projects"))
          .getAllByRole("tab")
          .map((tab) => tab.getAttribute("aria-label")),
      ).toEqual(["one", "two", "three"]),
    );
  });

  it("says how many chats came back when only some did", async () => {
    const { ready } = core(["one"], 0, ["one"]);

    render(<App />);

    await waitFor(() => expect(rows()).toEqual(["one…"]));
    ready("one", { kind: "ready", back: 2, wanted: 3 });
    await waitFor(() => expect(rows()).toEqual(["one"]));
    expect(row("one")).toHaveAccessibleDescription(/2 of 3 chats back$/);
  });

  it("says a project is still loading after 15 s without an answer", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    core(["one"], 0);

    render(<App />);

    await waitFor(() => expect(rows()).toEqual(["one…"]));
    expect(row("one")).toHaveAccessibleDescription("Loading…");
    await act(async () => vi.advanceTimersByTime(15_000));
    await waitFor(() => expect(row("one")).toHaveAccessibleDescription("Still loading…"));
  });

  it("announces how many projects are still loading", async () => {
    const { answer, ready } = core(["one", "two"], 0);

    render(<App />);

    await waitFor(() =>
      expect(screen.getByText("Loading 2 projects…")).toHaveAttribute("role", "status"),
    );
    await answer("one");
    ready("one");
    await waitFor(() => expect(screen.getByText("Loading 1 project…")).toBeInTheDocument());
    await answer("two");
    ready("two");
    await waitFor(() => expect(screen.queryByText(/^Loading \d project/)).not.toBeInTheDocument());
  });
});
