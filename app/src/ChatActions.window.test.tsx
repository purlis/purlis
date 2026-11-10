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

/** A chat waiting on the person's reply, which its Answer opens a box for. */
const REPLY: Shown = {
  session: 6,
  ask: "question:6",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 6"],
  answer: { via: "in-its-pane" },
  last_message: "Which branch?",
};

function draw(onStop?: (session: number) => void, asks: Shown[] = ASKS) {
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
      asks={asks}
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
  it("in the one order: a decision's answers on its row, then Go to chat, Snooze, Stop", () => {
    draw(vi.fn());
    const named = within(firstRow())
      .getAllByRole("button")
      .map((one) => one.textContent)
      .filter((words) => words !== "");
    // A decision is answered on its row (#5998): its answers are its Answer.
    expect(named).toEqual(["Allow", "Deny", "Go to chat", "Snooze", "Stop"]);
  });

  it("in the one order: Answer, Go to chat, Snooze, Stop, where it is answered in a form", () => {
    draw(vi.fn(), [REPLY]);
    const named = within(screen.getByRole("row", { name: /Waiting on your reply/ }))
      .getAllByRole("button")
      .map((one) => one.textContent)
      .filter((words) =>
        ["Answer", "Cancel", "Go to chat", "Snooze", "Stop"].includes(words ?? ""),
      );
    // Its Answer reads Cancel while its form is open.
    expect(named.slice(1)).toEqual(["Go to chat", "Snooze", "Stop"]);
    expect(["Answer", "Cancel"]).toContain(named[0]);
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
