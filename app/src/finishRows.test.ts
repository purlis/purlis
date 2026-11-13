// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { FinishedTurn, InboxUpdate } from "./bindings";
import { BLOCKED_THIS_TURN, finishUpdates, groupedFinishes, stoppedAfterBlock } from "./finishRows";
import { kindOfUpdate } from "./InboxRow";
import { fromOf, listed } from "./inboxUpdates";
import { toneOfUpdate } from "./messageTone";
import { SOURCES } from "./notificationPrefs";
import { quietPartsSaid, quietSummaryOf } from "./quiet";

const AT = 1_700_000_000;
const names: Record<number, string> = { 3: "steward 3", 5: "reviewer 5" };
const nameOf = (session: number) => names[session] ?? String(session);

const turn = (session: number, key: string, blocked: number, failed = false): FinishedTurn => ({
  session,
  key,
  at: AT,
  failed,
  blocked,
});

const kept = (noted: ReturnType<typeof finishUpdates>): InboxUpdate[] =>
  noted.map((one) => ({ ...one, read: false }));

describe("the turn's finish row (T1-31)", () => {
  it("a turn with several blocks has one finish row", () => {
    // R-179: the core counts the turn's unresolved blocks, one a target; the window draws one
    // row for the turn, whatever its count.
    const rows = finishUpdates([turn(3, "01TURNA", 2)], nameOf);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      kind: "turn-finished",
      session: 3,
      chain: ["steward 3"],
      says: "Finished its turn · 2 blocked this turn",
    });
    expect(BLOCKED_THIS_TURN(1)).toBe("· 1 blocked this turn");
    // Noted again, it is the same row: its key is the turn's own.
    expect(finishUpdates([turn(3, "01TURNA", 2)], nameOf)[0].key).toBe(rows[0].key);
  });

  it("a finish row with a blocked suffix is never a failure notification", () => {
    // R-180: style and channel are the finished-turn ones.
    const [blocked] = finishUpdates([turn(3, "01TURNA", 1)], nameOf);
    const [plain] = finishUpdates([turn(3, "01TURNB", 0)], nameOf);
    const [failed] = finishUpdates([turn(3, "01TURNC", 0, true)], nameOf);
    expect(blocked.kind).toBe(plain.kind);
    expect(kindOfUpdate(blocked.kind)).toBe("update");
    expect(toneOfUpdate(blocked.kind)).toBe(toneOfUpdate(plain.kind));
    expect(toneOfUpdate(blocked.kind)).not.toBe(toneOfUpdate(failed.kind));
    expect(fromOf(blocked.kind)).toBe("mine");
    // No failure source notifies of it.
    expect(SOURCES as readonly string[]).not.toContain(blocked.kind);
    expect(failed.kind).toBe("chat-failed");
    expect(kindOfUpdate(failed.kind)).toBe("trouble");
  });

  it("only a blocked finish is stopped after a block", () => {
    const rows = kept(
      finishUpdates([turn(3, "01A", 1), turn(3, "01B", 0), turn(5, "01C", 1, true)], nameOf),
    );
    expect(rows.map(stoppedAfterBlock)).toEqual([true, false, false]);
  });

  it("identical finish rows group across chats, and a different one stands apart", () => {
    // R-181: two chats whose turns each ended after one unresolved block are one row.
    const rows = listed(
      kept(
        finishUpdates(
          [turn(3, "01A", 1), turn(5, "01B", 1), turn(5, "01C", 0), turn(3, "01D", 0, true)],
          nameOf,
        ),
      ),
      [],
    );
    const grouped = groupedFinishes(rows);
    const blocked = grouped.filter(({ update }) => stoppedAfterBlock(update));
    expect(blocked).toHaveLength(1);
    expect(blocked[0].update.chain).toEqual(["steward 3, reviewer 5"]);
    expect([...blocked[0].keys].sort()).toEqual(
      rows
        .filter(stoppedAfterBlock)
        .map((one) => one.key)
        .sort(),
    );
    // The plain finish and the failure each stand alone.
    expect(grouped).toHaveLength(3);
  });

  it("puts a finish row that ended after a block in Quiet's summary bucket", () => {
    // R-182: "stopped after a block", with its count.
    const rows = kept(
      finishUpdates([turn(3, "01A", 1), turn(5, "01B", 2), turn(5, "01C", 0)], nameOf),
    );
    const summary = quietSummaryOf(
      { from: AT - 10, to: AT + 10, asks: [] },
      "/p",
      [],
      rows,
      () => true,
    );
    expect(summary.blocked).toHaveLength(2);
    expect(quietPartsSaid(summary)).toContainEqual({
      part: "blocked",
      says: "2 stopped after a block",
    });
  });
});
