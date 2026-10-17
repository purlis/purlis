// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatsReconnecting } from "./bindings";
import { RECONNECTING } from "./reconnecting";
import { SessionPane } from "./SessionPane";

/**
 * **A runner's chat says "Reconnecting" over its last screen while the runner's link is down**
 * (#2496, ADR 0078 as amended 2026-10-17): one quiet state over the terminal, cleared once the
 * core says the link is back. Never a Notice.
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

beforeEach(() => {
  // The view never opens: what is drawn over the terminal does not wait for it.
  mockIPC(() => new Promise(() => {}), { shouldMockEvents: true });
});
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
      reconnecting: true,
      ...said,
    } satisfies ChatsReconnecting);
  });

describe("a chat whose runner's link is down", () => {
  it("says Reconnecting over its screen, and nothing once the link is back", async () => {
    pane();
    expect(screen.queryByText("Reconnecting")).toBeNull();

    await told({});
    await waitFor(() => expect(screen.getByText("Reconnecting")).toBeTruthy());
    // Its last screen stays, dimmed under the word.
    expect(screen.getByTestId("pane").classList.contains("reconnecting")).toBe(true);

    await told({ reconnecting: false });
    await waitFor(() => expect(screen.queryByText("Reconnecting")).toBeNull());
    expect(screen.getByTestId("pane").classList.contains("reconnecting")).toBe(false);
  });

  it("is said of the chats named, in the plane named, and of no other", async () => {
    pane(2);

    await told({ sessions: [1, 3] });
    await told({ plane: "/planes/two", sessions: [2] });
    expect(screen.queryByText("Reconnecting")).toBeNull();

    // Heard, and taken as this chat's once it is named.
    await told({ sessions: [2] });
    await waitFor(() => expect(screen.getByText("Reconnecting")).toBeTruthy());
  });
});
