// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Shown } from "./bindings";
import { nothingKnown, type ChatStates, type State } from "./chatState";
import type { ListedChat } from "./chatsTree";
import { countsByState, fleetOf, inFleetOrder, type FleetChat } from "./everyChat";

/**
 * **All chats** (ADR 0121 decision 18, #6013): every chat of every open project, each with its
 * Chat state, how long it has been in it, where it works and its last line, Waiting on you
 * first, then Stuck?, then the rest.
 */

function listed(session: number, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name: `chat ${session}`,
    persona: null,
    workspace: "alpha",
    shell: false,
    parent: null,
    mode: null,
    from: null,
    tab: true,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...more,
  };
}

function states(by: Record<number, State>, needsYou: number[] = []): ChatStates {
  return { ...nothingKnown, bySession: by, needsYou };
}

function ask(session: number, key: string, says: string, more: Partial<Shown> = {}): Shown {
  return { session, ask: key, says, source: "question", ...more } as Shown;
}

const NONE: ReadonlyMap<number, number> = new Map();

describe("one project's chats, as All chats lists them", () => {
  it("names each chat's state, where it works and what it asks", () => {
    const chats = fleetOf({
      plane: "/p",
      project: "purlis",
      rows: [listed(1), listed(2, { workspace: "beta" })],
      states: states({ 1: "waiting", 2: "running" }, [1]),
      since: new Map([[1, 5_000]]),
      asks: [ask(1, "q1", "Which branch?", { last_message: "Shall I push to main?" })],
    });

    expect(chats).toEqual([
      expect.objectContaining({
        plane: "/p",
        project: "purlis",
        session: 1,
        name: "chat 1",
        workspace: "alpha",
        state: "needs",
        since: 5_000,
        line: "Shall I push to main?",
        asks: ["q1"],
        stoppable: true,
      }),
      expect.objectContaining({
        session: 2,
        workspace: "beta",
        state: "working",
        since: null,
        line: null,
        asks: [],
      }),
    ]);
  });

  it("says what a state's name leaves out, and that an ended chat has nothing to stop", () => {
    const [waiting, done] = fleetOf({
      plane: "/p",
      project: "purlis",
      rows: [listed(1), listed(2, { parent: 1, mode: "task", report: "sent", outcome: "done" })],
      states: states({ 1: "waiting", 2: "running" }),
      since: NONE,
      asks: [],
    });

    expect(waiting).toEqual(
      expect.objectContaining({ state: "waiting", line: "waiting on 1 task" }),
    );
    expect(waiting.stoppable).toBe(true);
    // A task that reported and runs a turn again, on the person's word, is working.
    expect(done).toEqual(expect.objectContaining({ state: "working", task: true }));
  });

  it("has nothing to stop once its program has ended", () => {
    const [ended] = fleetOf({
      plane: "/p",
      project: "purlis",
      rows: [listed(1)],
      states: states({ 1: "done" }),
      since: NONE,
      asks: [],
    });
    expect(ended).toEqual(expect.objectContaining({ state: "ended", stoppable: false }));
  });
});

/** A chat as another project, or another window, told it. */
function told(session: number, state: FleetChat["state"], more: Partial<FleetChat> = {}) {
  return {
    plane: "/p",
    project: "purlis",
    session,
    name: `chat ${session}`,
    persona: null,
    workspace: "alpha",
    state,
    word: state,
    since: null,
    line: null,
    asks: [],
    task: false,
    stoppable: true,
    ...more,
  } satisfies FleetChat;
}

describe("the order All chats lists them in", () => {
  it("is Waiting on you, then Stuck?, then the rest, each longest in its state first", () => {
    const order = inFleetOrder([
      told(1, "working", { since: 1_000 }),
      told(2, "ended"),
      told(3, "needs", { since: 9_000 }),
      told(4, "stuck"),
      told(5, "needs", { since: 2_000, plane: "/q" }),
      told(6, "idle"),
      told(7, "working", { since: 500 }),
    ]);
    expect(order.map((one) => `${one.plane}#${one.session}`)).toEqual([
      "/q#5",
      "/p#3",
      "/p#4",
      "/p#7",
      "/p#1",
      "/p#6",
      "/p#2",
    ]);
  });
});

describe("how many chats are in each state", () => {
  it("counts every state, none where none is in it", () => {
    const counts = countsByState([told(1, "working"), told(2, "working"), told(3, "stuck")]);
    expect(counts).toEqual({
      needs: 0,
      stuck: 1,
      failed: 0,
      working: 2,
      waiting: 0,
      idle: 0,
      ended: 0,
      "cant-tell": 0,
    });
  });
});
