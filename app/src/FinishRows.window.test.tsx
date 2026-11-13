// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import type { FinishedTurn } from "./bindings";
import { fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { finishUpdates, useFinishedTurns } from "./finishRows";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **The turn's finish row, against the core's count** (T1-31, R-178): the window asks the core
 * for each chat's finishes as a chat's turn ends, and draws "· 1 blocked this turn" only where
 * the core counted a block still unresolved at the turn's end. The core is faked at its command
 * boundary with what it counts (`eventlog::finishes`): a block then a success, 0; a block then
 * an Allow, 0; a block alone, 1.
 */

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.parse("2026-11-13T11:00:00Z") / 1000);

/** Chat 3's three turns, as the core counts them. */
const COUNTED: FinishedTurn[] = [
  { session: 3, key: "01TURNSUCCESS", at: AT - 2, failed: false, blocked: 0 },
  { session: 3, key: "01TURNALLOW", at: AT - 1, failed: false, blocked: 0 },
  { session: 3, key: "01TURNUNRESOLVED", at: AT, failed: false, blocked: 1 },
];

const WAITING: ChatStates = { ...nothingKnown, bySession: { 3: "waiting" }, movedAt: { 3: 7 } };
/** Chat 3, its turn ended: one store, as a project holds one. */
const CHATS = fixedChats(WAITING);

let asked: unknown[] = [];

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-11-13T12:00:00Z"));
  asked = [];
  mockIPC((cmd, args) => {
    if (cmd !== "finished_turns") return null;
    asked.push(args);
    return COUNTED;
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

function Mine() {
  const turns = useFinishedTurns(PLANE, CHATS);
  const rows = finishUpdates(turns, () => "steward 3").map((one) => ({
    update: { ...one, read: false },
    dismiss: () => undefined,
  }));
  return (
    <Inbox
      part="updates"
      plane={PLANE}
      asks={[]}
      onGo={() => undefined}
      onLeave={() => undefined}
      updates={{ rows, onMarkAllRead: () => undefined, onDismissAll: () => undefined }}
    />
  );
}

describe("the turn's finish row (T1-31)", () => {
  it("the finish row gains the blocked suffix only for an unresolved block", async () => {
    render(<Mine />);
    await waitFor(() => expect(screen.getAllByText(/Finished its turn/)).toHaveLength(3));
    expect(asked).toEqual([{ plane: PLANE, sessions: [3] }]);
    // One of the three turns ended with a block unresolved: its row alone has the suffix.
    expect(screen.getAllByText(/· 1 blocked this turn/)).toHaveLength(1);
    expect(screen.getAllByText(/^Finished its turn$/)).toHaveLength(2);
  });
});
