// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, NOTHING_WAITS } from "./Inbox";
import { ChatsHere, fixedChats, nothingKnown, type State } from "./chatState";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **Asks when nothing waits** (ADR 0121 decision 23, #6020): "Nothing waits on you · N chats
 * working", each part a way to the chats. All chats (#6013) is not built, so each part opens the
 * Chats sidebar; Stuck? (#6014) is not built either, so it is not said.
 */

beforeEach(() => {
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
});

const PLANE = "/home/dev/plane";

function draw(states: Record<number, State>, onAllChats = vi.fn()) {
  render(
    <ChatsHere.Provider value={fixedChats({ ...nothingKnown, bySession: states })}>
      <Inbox
        part="asks"
        plane={PLANE}
        asks={[]}
        onGo={vi.fn()}
        onLeave={vi.fn()}
        onAllChats={onAllChats}
      />
    </ChatsHere.Provider>,
  );
  return onAllChats;
}

describe("Asks with nothing waiting (ADR 0121 decision 23)", () => {
  it("says nothing waits and how many chats are working, each part opening the chats", async () => {
    const onAllChats = draw({ 1: "running", 2: "running", 3: "done", 4: "waiting" });

    expect(NOTHING_WAITS).toBe("Nothing waits on you");
    expect(document.querySelector(".inbox-none")).toHaveTextContent(
      "Nothing waits on you · 2 chats working",
    );

    await userEvent.click(screen.getByRole("button", { name: "Nothing waits on you" }));
    expect(onAllChats).toHaveBeenLastCalledWith("all");
    await userEvent.click(screen.getByRole("button", { name: "2 chats working" }));
    expect(onAllChats).toHaveBeenLastCalledWith("working");
  });

  it("says one chat working in the singular", () => {
    draw({ 1: "running" });
    expect(screen.getByRole("button", { name: "1 chat working" })).toBeInTheDocument();
  });

  it("leaves the working part out where no chat works, and never says stuck", () => {
    draw({ 1: "done" });
    expect(screen.getByRole("button", { name: NOTHING_WAITS })).toBeInTheDocument();
    expect(screen.queryByText(/working/)).toBeNull();
    expect(screen.queryByText(/stuck/i)).toBeNull();
  });

  it("is plain words where nothing above it opens the chats", () => {
    render(<Inbox part="asks" plane={PLANE} asks={[]} onGo={vi.fn()} onLeave={vi.fn()} />);
    expect(screen.getByText(NOTHING_WAITS)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: NOTHING_WAITS })).toBeNull();
  });
});
