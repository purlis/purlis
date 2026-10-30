// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { nothingKnown, type ChatStates, type State } from "./chatState";
import { owedAfterEnded, restartsDue, withRestartOwed } from "./restartAtPause";

/** The chats in `bySession`'s states, those in `onItsOwnWork` waiting on a monitor of their own. */
const states = (bySession: Record<number, State>, onItsOwnWork: number[] = []): ChatStates => ({
  ...nothingKnown,
  bySession,
  ownWork: Object.fromEntries(
    onItsOwnWork.map((session) => [session, { helpers: 0, workflows: 0, monitors: 1, shells: 0 }]),
  ),
});

describe("a restart owed for a grant (#1986)", () => {
  it("waits while the chat is mid-turn and is made at its next pause", () => {
    const owed = withRestartOwed([], 3);
    expect(restartsDue(owed, states({ 3: "running" }))).toEqual({ restart: [], ended: [] });
    expect(restartsDue(owed, states({ 3: "waiting" }))).toEqual({ restart: [3], ended: [] });
  });

  it("is one restart for two grants made before the pause", () => {
    const owed = withRestartOwed(withRestartOwed([], 3), 3);
    expect(owed).toEqual([3]);
    expect(restartsDue(owed, states({ 3: "waiting" })).restart).toEqual([3]);
  });

  it("restarts nothing when the chat ended first, and is no longer owed", () => {
    const owed = withRestartOwed(withRestartOwed([], 3), 4);
    expect(restartsDue(owed, states({ 3: "done", 4: "failed" }))).toEqual({
      restart: [],
      ended: [3, 4],
    });
  });

  it("is never made behind the person's back for a harness that says nothing of its turns", () => {
    expect(restartsDue(withRestartOwed([], 5), states({}))).toEqual({
      restart: [],
      ended: [],
    });
  });

  it("keeps each chat's restart to its own pause", () => {
    const owed = withRestartOwed(withRestartOwed([], 3), 4);
    expect(restartsDue(owed, states({ 3: "running", 4: "waiting" }))).toEqual({
      restart: [4],
      ended: [],
    });
  });
});

describe("a restart owed to a chat waiting on its own work (#3887)", () => {
  it("waits for that work, which a restart would cut off, and is made once the chat's turn has nothing in flight", () => {
    const owed = withRestartOwed(withRestartOwed([], 3), 4);
    expect(restartsDue(owed, states({ 3: "waiting", 4: "waiting" }, [3]))).toEqual({
      restart: [4],
      ended: [],
    });
    expect(restartsDue(owed, states({ 3: "waiting", 4: "waiting" })).restart).toEqual([3, 4]);
  });
});

describe("Restart now (#1986)", () => {
  it("restarts a chat mid-turn at once, without waiting for its pause", () => {
    const owed = withRestartOwed([], 3);
    expect(restartsDue(owed, states({ 3: "running" }), { now: 3 }).restart).toEqual([3]);
  });

  it("restarts a chat waiting on its own work at once: the person chose to cut it off (#3887)", () => {
    const owed = withRestartOwed([], 3);
    expect(restartsDue(owed, states({ 3: "waiting" }, [3]), { now: 3 }).restart).toEqual([3]);
  });

  it("restarts a chat whose harness says nothing of its turns", () => {
    expect(restartsDue(withRestartOwed([], 5), states({}), { now: 5 }).restart).toEqual([5]);
  });

  it("restarts nothing for a chat that ended", () => {
    expect(restartsDue(withRestartOwed([], 3), states({ 3: "done" }), { now: 3 })).toEqual({
      restart: [],
      ended: [3],
    });
  });
});

describe("a chat reported ended (#1986)", () => {
  it("keeps its restart owed while the core still owes it: its hooks said the session ended, and its program runs on", () => {
    // A SessionEnd hook moves the chat to done before, or without, its program's exit; only the
    // program's end, which the core hears, drops what it is owed.
    expect(owedAfterEnded([3, 4], [3], [3, 4])).toEqual([3, 4]);
  });

  it("is owed nothing once the core, which heard its program end, owes it nothing", () => {
    expect(owedAfterEnded([3, 4], [3], [4])).toEqual([4]);
  });

  it("leaves every other chat's restart owed whatever the core lists", () => {
    expect(owedAfterEnded([3, 4], [3], [])).toEqual([4]);
  });
});
