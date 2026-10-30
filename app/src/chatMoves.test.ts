// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { ChatMove, PoolsShown } from "./bindings";
import { nothingKnown, type ChatStates, type State } from "./chatState";
import {
  chatMovesOf,
  heardMove,
  moveCancellable,
  moveOffers,
  moveSaid,
  movesDue,
  movesWith,
  poolsToMoveTo,
} from "./chatMoves";

const ready = (session: number, to: string | null = "builds"): ChatMove => ({
  session,
  from: null,
  to,
  stage: "ready",
  why: null,
  with: [],
  sign_in: null,
});

describe("Move to runner and Move here (#2048)", () => {
  it("a move waiting on its profile's sign-in says so, may be cancelled, and is not due (#3477)", () => {
    const one: ChatMove = {
      ...ready(1),
      stage: "signing-in",
      sign_in: { profile: "work", kind: "claude", harness: "Claude Code" },
    };
    expect(moveSaid(one)).toEqual({
      words: "Moves to pool builds once work is signed in there. It keeps working here meanwhile.",
      trouble: false,
    });
    expect(moveCancellable(one)).toBe(true);
    expect(movesDue({ 1: one }, { ...nothingKnown, bySession: { 1: "waiting" } })).toEqual([]);
  });

  it("switches a prepared move only at the chat's pause between turns, never mid-turn", () => {
    const moves = {
      1: ready(1),
      2: ready(2),
      3: { ...ready(3), stage: "preparing" },
      4: { ...ready(4), stage: "failed", why: "the runner did not answer" },
    };
    const states: ChatStates = {
      ...nothingKnown,
      bySession: { 1: "waiting", 2: "running", 3: "waiting", 4: "waiting" },
    };
    expect(movesDue(moves, states)).toEqual([1]);
  });

  it("waits for the work of a chat waiting on its own work, which a move would cut off (#3887)", () => {
    const moves = { 1: ready(1), 2: ready(2) };
    const waiting: Record<number, State> = { 1: "waiting", 2: "waiting" };
    const monitor = { helpers: 0, workflows: 0, monitors: 1, shells: 0 };
    expect(
      movesDue(moves, { ...nothingKnown, bySession: waiting, ownWork: { 1: monitor } }),
    ).toEqual([2]);
    expect(movesDue(moves, { ...nothingKnown, bySession: waiting })).toEqual([1, 2]);
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
      { session: 1, workspace: "app", pool: null, label: null, name: "one", harness: "claude" },
      {
        session: 2,
        workspace: "app",
        pool: null,
        label: "steward 1",
        name: "two",
        harness: "claude",
      },
      { session: 3, workspace: "app", pool: "builds", label: null, name: "three", harness: "x" },
      { session: 4, workspace: "web", pool: null, label: null, name: "four", harness: "x" },
      { session: 5, workspace: null, pool: null, label: null, name: "five", harness: "x" },
      { session: 6, workspace: null, pool: null, label: null, name: "six", harness: "x" },
      // A shell does not move, so a row never names it (#3814).
      { session: 7, workspace: "app", pool: null, label: null, name: "zsh", harness: null },
    ];
    expect(movesWith(1, open)).toEqual(["steward 1"]);
    expect(movesWith(3, open)).toEqual([]);
    expect(movesWith(5, open)).toEqual([]);
    expect(movesWith(9, open)).toEqual([]);
  });

  it("forgets a move on every tab when the core says it is over (#3814)", () => {
    const plane = "p-over";
    heardMove(plane, { ...ready(1), with: ["two"] });
    heardMove(plane, { ...ready(2), with: ["one"] });
    heardMove(plane, { ...ready(1), stage: "over" });
    heardMove(plane, { ...ready(2), stage: "over" });
    expect(chatMovesOf(plane)).toEqual({});
  });

  it("offers Cancel only while a move is prepared or ready, never while it is made (#3814)", () => {
    expect(moveCancellable(ready(1))).toBe(true);
    expect(moveCancellable({ ...ready(1), stage: "preparing" })).toBe(true);
    expect(moveCancellable({ ...ready(1), stage: "switching" })).toBe(false);
    expect(moveSaid({ ...ready(1), stage: "switching", with: ["two"] })).toEqual({
      words: "Moving to pool builds now, with two.",
      trouble: false,
    });
  });
});
