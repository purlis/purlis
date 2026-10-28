// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatMove, PoolsShown } from "./bindings";
import type { State } from "./chatState";
import { moveOffers, moveSaid, movesDue, poolsToMoveTo } from "./chatMoves";

const ready = (session: number, to: string | null = "builds"): ChatMove => ({
  session,
  to,
  stage: "ready",
  why: null,
});

describe("Move to runner and Move here (#2048)", () => {
  it("switches a prepared move only at the chat's pause between turns, never mid-turn", () => {
    const moves = {
      1: ready(1),
      2: ready(2),
      3: { ...ready(3), stage: "preparing" },
      4: { ...ready(4), stage: "failed", why: "the runner did not answer" },
    };
    const states: Record<number, State> = {
      1: "waiting",
      2: "running",
      3: "waiting",
      4: "waiting",
    };
    expect(movesDue(moves, (session) => states[session] ?? "unknown")).toEqual([1]);
  });

  it("offers Move here on a runner, one row per pool here, and nothing for a shell", () => {
    expect(moveOffers({ harness: "claude-code", pool: "builds" }, ["builds", "big"])).toEqual([
      null,
    ]);
    expect(moveOffers({ harness: "claude-code", pool: null }, ["builds", "big"])).toEqual([
      "builds",
      "big",
    ]);
    expect(moveOffers({ harness: null, pool: null }, ["builds"])).toEqual([]);
    expect(moveOffers({ harness: "claude-code", pool: null }, [])).toEqual([]);
    expect(moveOffers(undefined, ["builds"])).toEqual([]);
  });

  it("says a failed move left the chat where it was, with the core's reason", () => {
    const said = moveSaid({
      session: 1,
      to: "builds",
      stage: "failed",
      why: "The runner of pool builds did not answer.",
    });
    expect(said).toEqual({
      words:
        "Did not move to pool builds, so it runs where it was. The runner of pool builds did not answer.",
      trouble: true,
    });
    expect(moveSaid(ready(1, null)).words).toBe("Moves to this machine at its next pause.");
  });

  it("offers to move only to pools whose runners are shown to be the person's own (D-2048-13)", () => {
    const pools = [{ name: "builds" }, { name: "big" }] as unknown as PoolsShown["pools"];
    expect(poolsToMoveTo({ pools, yours: true })).toEqual(["builds", "big"]);
    expect(poolsToMoveTo({ pools, yours: false })).toEqual([]);
    expect(poolsToMoveTo({ pools })).toEqual([]);
    expect(poolsToMoveTo(undefined)).toEqual([]);
  });
});
