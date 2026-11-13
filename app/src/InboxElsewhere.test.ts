// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import type { Shown } from "./bindings";
import { askingUntold, elsewhereCount, theirAsks, type Elsewhere } from "./InboxElsewhere";
import type { Needing } from "./NeedsYou";

/**
 * **Every other open project's asks, as Asks lists them and their arrival is said** (#1921, ADR
 * 0121 decision 4): this window's other projects and the other windows', ask by ask where their
 * registry was read and told, and a chat at a time where it was not.
 */

const ask = (session: number, key: string): Shown => ({
  session,
  ask: key,
  says: key,
  options: [],
  source: "permission",
  chain: [`chat ${session}`],
  answer: { via: "hook" },
});

const chat = (plane: string, session: number): Needing => ({
  plane,
  project: plane,
  session,
  name: `chat ${session}`,
  workspace: "alpha",
});

const elsewhere = (over: Partial<Elsewhere>): Elsewhere => ({
  asking: [],
  quiet: [],
  onPress: vi.fn(),
  ...over,
});

describe("another window's asks, told ask by ask (#1921)", () => {
  const told = elsewhere({
    // Chat 7 of /ops asks twice; that window told both, and /far told nothing yet.
    asking: [chat("/ops", 7), chat("/far", 2)],
    waitingHere: [
      { plane: "/here", project: "here", count: 1, asks: [ask(1, "a")] },
      { plane: "/web", project: "web", count: 1, asks: [ask(4, "w")] },
    ],
    there: [{ plane: "/ops", project: "ops", count: 2, asks: [ask(7, "x"), ask(7, "y")] }],
  });

  it("counts a told window's asks, not its chats, and an untold window's chats", () => {
    expect(elsewhereCount("/here", told)).toBe(1 + 2 + 1);
  });

  it("lists every other project's asks, this window's and the others', and not its own", () => {
    expect(theirAsks("/here", told).map((one) => [one.plane, one.here, one.asks.length])).toEqual([
      ["/web", true, 1],
      ["/ops", false, 2],
    ]);
  });

  it("draws a chat at a time only where its window told no asks", () => {
    expect(askingUntold(told).map((one) => one.plane)).toEqual(["/far"]);
  });

  it("never lists a project of this window twice, once a window it moved from still tells it", () => {
    const moved = elsewhere({
      waitingHere: [{ plane: "/web", project: "web", count: 1, asks: [ask(4, "w")] }],
      there: [{ plane: "/web", project: "web", count: 1, asks: [ask(4, "w")] }],
    });
    expect(theirAsks("/here", moved)).toHaveLength(1);
    expect(elsewhereCount("/here", moved)).toBe(1);
  });
});
