// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatNoticed } from "./bindings";
import { leftWorkOf, moved, noticed, noticedUnder, nothingKnown, stuckOf } from "./chatState";

const told = (more: Partial<ChatNoticed>): ChatNoticed => ({
  plane: "/p" as ChatNoticed["plane"],
  session: 3,
  stuck: null,
  left_work: null,
  mode: null,
  ...more,
});

describe("what the core noticed of a chat (#6014)", () => {
  it("keeps why it reads Stuck? until the core says it no longer does", () => {
    const stuck = noticed(nothingKnown, told({ stuck: "no new command or output for 15 minutes" }));
    expect(stuckOf(stuck, 3)).toBe("no new command or output for 15 minutes");
    expect(stuckOf(noticed(stuck, told({})), 3)).toBeUndefined();
  });

  it("keeps whether its folder holds work left behind, and says nothing where unknown", () => {
    expect(leftWorkOf(noticed(nothingKnown, told({ left_work: true })), 3)).toBe(true);
    expect(leftWorkOf(noticed(nothingKnown, told({ left_work: false })), 3)).toBe(false);
    expect(leftWorkOf(noticed(nothingKnown, told({})), 3)).toBeUndefined();
  });

  it("answers the same states where nothing changed, so no reader is drawn again", () => {
    const once = noticed(nothingKnown, told({ stuck: "x", left_work: true }));
    expect(noticed(once, told({ stuck: "x", left_work: true }))).toBe(once);
  });

  it("puts a first answer under what the core has told since", () => {
    const since = noticed(nothingKnown, told({}));
    const first = [told({ stuck: "older" }), told({ session: 4, stuck: "y" })];
    const both = noticedUnder(since, first);
    expect(stuckOf(both, 3)).toBeUndefined();
    expect(stuckOf(both, 4)).toBe("y");
  });
});

describe("a move and what was noticed (#6014)", () => {
  it("keeps what was noticed of every chat, which no move says", () => {
    const stuck = noticed(nothingKnown, told({ stuck: "x" }));
    const after = moved(stuck, {
      plane: "/p" as ChatNoticed["plane"],
      session: 3,
      state: "running",
      needs_you: false,
      queue: [],
      moved_at: 1,
      sequence: 1,
      reports: [],
      refusals: [],
      children: [],
    });
    expect(after.noticed).toBe(stuck.noticed);
  });
});
