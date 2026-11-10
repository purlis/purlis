// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Shown } from "./bindings";
import { inOrder, standingSaid, type Standing } from "./asksOrder";

/**
 * **Asks are ordered by facts** (ADR 0121 decision 5, #6006): an Expiring ask first, then one
 * that blocks other chats, then the longest wait; one chat's asks together.
 */

/** The clock every case reads: 1,000 s past 1970, in ms. */
const NOW = 1_000_000;

type Fact = { expires?: number; blocks?: number; since: number; timedOut?: number };

const ask = (session: number, key: string, { expires, blocks, since, timedOut }: Fact): Shown => ({
  session,
  ask: key,
  says: key,
  options: [],
  source: "permission",
  chain: [`chat ${session}`],
  answer: { via: "in-its-pane" },
  since,
  expires,
  blocks,
  timed_out: timedOut,
});

/** An ask's age, as the Inbox reads it: its `since`, in ms, and its place in the list. */
const age = (one: Shown) => [(one.since ?? 0) * 1000, 0] as const;

const order = (asks: readonly Shown[]) =>
  inOrder(asks, age, NOW).flatMap((group) => group.placed.map(({ ask }) => ask.ask));

describe("the order of Asks", () => {
  it.each<{ name: string; asks: Shown[]; listed: string[] }>([
    {
      name: "the longest wait first, where nothing else is known",
      asks: [ask(1, "new", { since: 900 }), ask(2, "old", { since: 100 })],
      listed: ["old", "new"],
    },
    {
      name: "an ask that blocks other chats before a longer wait",
      asks: [ask(1, "old", { since: 100 }), ask(2, "blocks", { since: 900, blocks: 1 })],
      listed: ["blocks", "old"],
    },
    {
      name: "the more chats an ask blocks, the sooner",
      asks: [ask(1, "one", { since: 100, blocks: 1 }), ask(2, "three", { since: 900, blocks: 3 })],
      listed: ["three", "one"],
    },
    {
      name: "an Expiring ask before one that blocks others",
      asks: [
        ask(1, "blocks", { since: 100, blocks: 3 }),
        ask(2, "expires", { since: 900, expires: 1_040 }),
      ],
      listed: ["expires", "blocks"],
    },
    {
      name: "the Expiring ask that ends soonest first",
      asks: [
        ask(1, "later", { since: 100, expires: 1_050 }),
        ask(2, "sooner", { since: 900, expires: 1_020 }),
      ],
      listed: ["sooner", "later"],
    },
    {
      name: "an ask whose time has passed is no longer Expiring",
      asks: [ask(1, "old", { since: 100 }), ask(2, "passed", { since: 900, expires: 990 })],
      listed: ["old", "passed"],
    },
    {
      name: "one chat's asks together, where its first ask stands, its Expiring one first",
      asks: [
        ask(1, "a-old", { since: 100 }),
        ask(2, "b", { since: 200 }),
        ask(1, "a-expires", { since: 900, expires: 1_030 }),
      ],
      listed: ["a-expires", "a-old", "b"],
    },
    {
      name: "a timed-out ask after a live Expiring ask and before one that blocks others",
      asks: [
        ask(1, "blocks", { since: 100, blocks: 3 }),
        ask(2, "timed-out", { since: 900, timedOut: 960 }),
        ask(3, "expires", { since: 950, expires: 1_040 }),
      ],
      listed: ["expires", "timed-out", "blocks"],
    },
    {
      name: "timed-out asks by the longest wait",
      asks: [
        ask(1, "newer", { since: 800, timedOut: 990 }),
        ask(2, "older", { since: 300, timedOut: 999 }),
      ],
      listed: ["older", "newer"],
    },
    {
      name: "one chat's timed-out ask after its Expiring one and before its others",
      asks: [
        ask(1, "a-old", { since: 100 }),
        ask(1, "a-timed-out", { since: 500, timedOut: 980 }),
        ask(1, "a-expires", { since: 900, expires: 1_030 }),
      ],
      listed: ["a-expires", "a-timed-out", "a-old"],
    },
    {
      name: "a new ask takes its place by its facts, never the top for being new",
      asks: [
        ask(1, "old", { since: 100 }),
        ask(2, "blocks", { since: 200, blocks: 1 }),
        ask(3, "new", { since: 999 }),
      ],
      listed: ["blocks", "old", "new"],
    },
  ])("lists $name", ({ asks, listed }) => {
    expect(order(asks)).toEqual(listed);
  });

  it("is the same order however the registry listed them", () => {
    const asks = [
      ask(1, "old", { since: 100 }),
      ask(2, "blocks", { since: 500, blocks: 2 }),
      ask(3, "expires", { since: 900, expires: 1_030 }),
      ask(4, "new", { since: 950 }),
    ];
    expect(order([...asks].reverse())).toEqual(order(asks));
  });

  it("says why each ask stands where it does", () => {
    const groups = inOrder(
      [
        ask(1, "a-expires", { since: 900, expires: 1_040 }),
        ask(1, "a-other", { since: 800 }),
        ask(2, "b", { since: 100, blocks: 2 }),
        ask(3, "c", { since: 300 }),
        ask(4, "d", { since: 200, timedOut: 990 }),
        ask(1, "a-timed-out", { since: 700, timedOut: 950 }),
      ],
      age,
      NOW,
    );
    expect(groups.flatMap((group) => group.placed.map(({ why }) => why))).toEqual<Standing[]>([
      { by: "expires", at: 1_040_000 },
      { by: "timed-out", at: 950_000 },
      { by: "its-chat" },
      { by: "timed-out", at: 990_000 },
      { by: "blocks", chats: 2 },
      { by: "wait", since: 300_000 },
    ]);
  });
});

describe("what a row's reason line says", () => {
  it.each<{ why: Standing; said: string }>([
    { why: { by: "expires", at: NOW + 40_000 }, said: "Expires in 40 s" },
    { why: { by: "expires", at: NOW + 400 }, said: "Expires in 1 s" },
    { why: { by: "expires", at: NOW - 1 }, said: "Expiring now" },
    { why: { by: "expires", at: NOW + 150_000 }, said: "Expires in 3 min" },
    { why: { by: "blocks", chats: 1 }, said: "Blocks 1 chat" },
    { why: { by: "blocks", chats: 3 }, said: "Blocks 3 chats" },
    { why: { by: "its-chat" }, said: "With its chat's ask that expires" },
    {
      why: { by: "timed-out", at: NOW - 5_000 },
      said: "Timed out. Answer, and the chat is told to try again.",
    },
    { why: { by: "wait", since: NOW - 30_000 }, said: "Waiting under a minute" },
    { why: { by: "wait", since: NOW - 12 * 60_000 }, said: "Waiting 12 min" },
    { why: { by: "wait", since: NOW - 3 * 3_600_000 }, said: "Waiting 3 h" },
    { why: { by: "wait", since: Number.MAX_SAFE_INTEGER }, said: "Waiting" },
  ])("$said", ({ why, said }) => {
    expect(standingSaid(why, NOW)).toBe(said);
  });
});
