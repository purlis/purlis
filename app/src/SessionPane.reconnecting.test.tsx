// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatsReconnecting } from "./bindings";
import { RECONNECTING } from "./reconnecting";
import { SessionPane } from "./SessionPane";

/**
 * **A runner's chat says "Reconnecting…" over its last screen while the runner's link is
 * down** (#2496, ADR 0078 as amended 2026-10-18): one quiet state over the terminal, cleared once
 * the core says the chat is reconnected. Never a Notice.
 */

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
const SAID = "Reconnecting…";

/** The core, as a pane meets it: the chats it says are reconnecting now, and no view opening. */
function core(reconnecting: number[] = []) {
  mockIPC(
    (command) => {
      if (command === "chats_reconnecting") return reconnecting;
      // The view never opens: what is drawn over the terminal does not wait for it.
      return new Promise(() => {});
    },
    { shouldMockEvents: true },
  );
}

afterEach(() => {
  cleanup();
  clearMocks();
});

const pane = (session = 1) =>
  render(<SessionPane plane={PLANE} session={session} focused={false} onFocus={() => {}} />);

const told = (said: Partial<ChatsReconnecting>) =>
  act(async () => {
    await emit(RECONNECTING, {
      plane: PLANE,
      sessions: [1],
      reconnection: "reconnecting",
      ...said,
    } satisfies ChatsReconnecting);
  });

describe("a chat whose runner's link is down", () => {
  it("says Reconnecting… over its dimmed screen, and nothing once it is reconnected", async () => {
    core();
    pane();
    // The status is there all along, so a screen reader hears the word as it comes.
    const status = screen.getByRole("status");
    expect(status.textContent).toBe("");

    await told({});
    await waitFor(() => expect(status.textContent).toBe(SAID));
    expect(screen.getByTestId("pane").classList.contains("reconnecting")).toBe(true);

    await told({ reconnection: "reconnected" });
    await waitFor(() => expect(status.textContent).toBe(""));
    expect(screen.getByTestId("pane").classList.contains("reconnecting")).toBe(false);
    expect(screen.getByRole("status")).toBe(status);
  });

  it("is said of the chats named, in the plane named, and of no other", async () => {
    core();
    pane(2);

    await told({ sessions: [1, 3] });
    await told({ plane: "/planes/two", sessions: [2] });
    expect(screen.queryByText(SAID)).toBeNull();

    // Heard, and taken as this chat's once it is named.
    await told({ sessions: [2] });
    await waitFor(() => expect(screen.getByText(SAID)).toBeTruthy());
  });

  it("starts dimmed when it is drawn while its runner's link is down", async () => {
    core([1]);
    pane();

    await waitFor(() => expect(screen.getByText(SAID)).toBeTruthy());
    expect(screen.getByTestId("pane").classList.contains("reconnecting")).toBe(true);
  });
});
