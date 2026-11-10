// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import type { Shown } from "./bindings";
import type { Snoozing } from "./AsksSnooze";
import { NOTHING_TO_STOP } from "./ChatActions";

/**
 * **The same actions on every chat row** (ADR 0121 decision 21, #6013): a row of Asks offers
 * Answer (when it asks), Go to chat, Snooze and Stop, in that order, as a row of All chats does.
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

const ASKS: Shown[] = [
  {
    session: 3,
    ask: "01J0A",
    says: "Run cargo test",
    options: [
      { id: "allow", label: "Allow", allows: true },
      { id: "deny", label: "Deny", allows: false },
    ],
    source: "permission",
    chain: ["steward 3"],
    answer: { via: "hook" },
  },
  {
    session: 3,
    ask: "01J0B",
    says: "Run npm ci",
    options: [{ id: "deny", label: "Deny", allows: false }],
    source: "permission",
    chain: ["steward 3"],
    answer: { via: "hook" },
  },
];

function draw(onStop?: (session: number) => void) {
  const snoozing: Snoozing = {
    snoozed: [],
    snooze: vi.fn(() => Promise.resolve(undefined)),
    wake: vi.fn(() => Promise.resolve(undefined)),
  };
  const onGo = vi.fn();
  render(
    <Inbox
      part="asks"
      plane={PLANE}
      asks={ASKS}
      onGo={onGo}
      onLeave={vi.fn()}
      snoozing={snoozing}
      onStop={onStop}
    />,
  );
  return { snoozing, onGo };
}

/** The first ask's row of the list. */
const firstRow = () => screen.getByRole("row", { name: /Run cargo test/ });

describe("a row of Asks offers the chat's actions (#6013)", () => {
  it("in the one order: Answer, Go to chat, Snooze, Stop", () => {
    draw(vi.fn());
    const named = within(firstRow())
      .getAllByRole("button")
      .map((one) => one.textContent)
      .filter((words) => ["Answer", "Go to chat", "Snooze", "Stop"].includes(words ?? ""));
    expect(named).toEqual(["Answer", "Go to chat", "Snooze", "Stop"]);
  });

  it("snoozes the chat with every ask it holds", async () => {
    const { snoozing } = draw(vi.fn());
    await userEvent.click(within(firstRow()).getByRole("button", { name: "Snooze steward 3" }));
    expect(snoozing.snooze).toHaveBeenCalledWith([{ session: 3, asks: ["01J0A", "01J0B"] }]);
  });

  it("stops the chat", async () => {
    const onStop = vi.fn();
    draw(onStop);
    await userEvent.click(within(firstRow()).getByRole("button", { name: "Stop steward 3" }));
    expect(onStop).toHaveBeenCalledWith(3);
  });

  it("has Stop off, and says why, where nothing can stop the chat from here", () => {
    draw();
    // Off, and still reached, so its hint can say why (`pressable.tsx`).
    const stop = within(firstRow()).getByRole("button", { name: "Stop steward 3" });
    expect(stop).toHaveAttribute("aria-disabled", "true");
    expect(stop).toHaveAccessibleDescription(NOTHING_TO_STOP);
  });
});
