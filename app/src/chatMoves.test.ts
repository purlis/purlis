// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatMove, PoolsShown } from "./bindings";
import type { State } from "./chatState";
import { moveOffers, moveSaid, movesDue, movesWith, poolsToMoveTo } from "./chatMoves";

const ready = (session: number, to: string | null = "builds"): ChatMove => ({
  session,
  from: null,
  to,
  stage: "ready",
  why: null,
  with: [],
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

  it("says a failed move left the chat where it was, naming where it runs once (#3646)", () => {
    const said = moveSaid({
      ...ready(1),
      stage: "failed",
      why: "It did not start on pool builds: the runner refused it.",
    });
    expect(said).toEqual({
      words:
        "Did not move to pool builds, so it still runs on this machine. It did not start on pool builds: the runner refused it.",
      trouble: true,
    });
    expect(said.words.split("this machine").length - 1).toBe(1);
    expect(moveSaid({ ...ready(1, null), from: "builds", stage: "failed", why: "No." }).words).toBe(
      "Did not move to this machine, so it still runs on pool builds. No.",
    );
    expect(moveSaid(ready(1, null)).words).toBe("Moves to this machine at its next pause.");
  });

  it("offers to move only to pools whose runners are shown to be the person's own (D-2048-13)", () => {
    const pools = [{ name: "builds" }, { name: "big" }] as unknown as PoolsShown["pools"];
    expect(poolsToMoveTo({ pools, yours: true })).toEqual(["builds", "big"]);
    expect(poolsToMoveTo({ pools, yours: false })).toEqual([]);
    expect(poolsToMoveTo({ pools })).toEqual([]);
    expect(poolsToMoveTo(undefined)).toEqual([]);
  });

  it("names the chats of its workspace that move with it (#3453)", () => {
    expect(moveSaid({ ...ready(1), with: ["two", "three"] }).words).toBe(
      "Moves to pool builds with two and three once each is at a pause.",
    );
    expect(moveSaid({ ...ready(1), stage: "preparing", with: ["two"] }).words).toContain(
      "with two, which share its workspace's clones",
    );
  });

  it("finds the chats of a chat's workspace running where it runs, and none at the root", () => {
    const open = [
      { session: 1, workspace: "app", pool: null, label: null, name: "one" },
      { session: 2, workspace: "app", pool: null, label: "steward 1", name: "two" },
      { session: 3, workspace: "app", pool: "builds", label: null, name: "three" },
      { session: 4, workspace: "web", pool: null, label: null, name: "four" },
      { session: 5, workspace: null, pool: null, label: null, name: "five" },
      { session: 6, workspace: null, pool: null, label: null, name: "six" },
    ];
    expect(movesWith(1, open)).toEqual(["steward 1"]);
    expect(movesWith(3, open)).toEqual([]);
    expect(movesWith(5, open)).toEqual([]);
    expect(movesWith(9, open)).toEqual([]);
  });
});
