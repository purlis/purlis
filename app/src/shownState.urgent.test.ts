// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { urgentOf, type MarkState } from "./shownState";

/**
 * **A split tab's mark is its most urgent pane's** (#5342, spec #5339): the panes' chats in
 * reading order, each with its one state, and the one whose state comes first in the fixed order
 * wins. A pane with no state (a view, an empty pane, a harness that has said nothing) is never
 * chosen. Of two in the same state, the first in reading order wins, so the mark only moves for a
 * state that outranks it.
 */
type Pane = { chat: string; mark: MarkState | null };
const pane = (chat: string, mark: MarkState | null): Pane => ({ chat, mark });

const table: readonly [string, Pane[], string | undefined][] = [
  ["one pane is its own mark", [pane("a", "idle")], "a"],
  ["a pane that needs you beats the working one", [pane("a", "working"), pane("b", "needs")], "b"],
  ["a failed task beats a working pane", [pane("a", "working"), pane("b", "failed")], "b"],
  ["working beats waiting on tasks", [pane("a", "waiting"), pane("b", "working")], "b"],
  ["waiting on tasks beats idle", [pane("a", "idle"), pane("b", "waiting")], "b"],
  ["idle beats ended", [pane("a", "ended"), pane("b", "idle")], "b"],
  ["of two in one state, the first in reading order", [pane("a", "idle"), pane("b", "idle")], "a"],
  ["needs you wins from the third of four panes", [
    pane("a", "working"),
    pane("b", "failed"),
    pane("c", "needs"),
    pane("d", "needs"),
  ], "c"],
  ["a pane with no state is passed over", [pane("view", null), pane("b", "ended")], "b"],
  ["no pane with a state is no mark", [pane("view", null), pane("empty", null)], undefined],
  ["no panes is no mark", [], undefined],
];

describe("a split tab's mark", () => {
  it.each(table)("%s", (_, panes, wins) => {
    expect(urgentOf(panes, (one) => one.mark)?.chat).toBe(wins);
  });
});
