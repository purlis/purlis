// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import type { Offered, Shown } from "./bindings";
import { inOrder } from "./asksOrder";
import { highRisk, sameSaid, withSame, type AskRow } from "./asksSame";

/**
 * **Identical requests are one row** (ADR 0121 decision 6, #6009): the same kind and the same
 * host, command, or tool and argument, from several chats, answered once for all.
 */

const NOW = 1_000_000;

const allow = (id: string, label: string): Offered => ({ id, label, allows: true });
const keep: Offered = { id: "keep", label: "Keep blocked", allows: false };
const HOST_OPTIONS = [
  allow("you", "Allow on this machine"),
  allow("chat", "Allow for this chat"),
  keep,
];
const PERMISSION_OPTIONS = [
  allow("once", "Allow once"),
  allow("always", "Always allow"),
  { id: "deny", label: "Deny", allows: false },
];

type Facts = { since?: number; expires?: number; blocks?: number };

const host = (session: number, target: string, facts: Facts = {}): Shown => ({
  session,
  ask: `host:${session}:${target}`,
  says: `The sandbox refused ${target}`,
  options: HOST_OPTIONS,
  source: "sandbox-host",
  chain: [`chat ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "network", what: "host", target },
  },
  since: facts.since ?? 100,
  expires: facts.expires,
  blocks: facts.blocks,
});

const permission = (session: number, says: string, facts: Facts = {}): Shown => ({
  session,
  ask: `perm:${session}:${says}`,
  says,
  options: PERMISSION_OPTIONS,
  source: "permission",
  chain: [`chat ${session}`],
  answer: { via: "hook" },
  since: facts.since ?? 100,
  expires: facts.expires,
});

const runGrant = (session: number, command: string): Shown => ({
  session,
  ask: `run:${session}:${command}`,
  says: `Run ${command} with vault team`,
  options: [allow("once", "Allow once"), allow("chat", "Allow for this chat"), keep],
  source: "run-grant",
  chain: [`chat ${session}`],
  answer: { via: "run-grant", id: session },
  since: 100,
  run: {
    persona: "steward",
    vault: "team",
    program: "/usr/bin/curl",
    values: [],
    hosts: [],
    every_host: false,
    not_its_vault: false,
    command,
    cwd: "/work",
    fills: [],
    offer: "program",
    ahead: false,
  },
});

const reply = (session: number): Shown => ({
  session,
  ask: `reply:${session}`,
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: [`chat ${session}`],
  answer: { via: "in-its-pane" },
  since: 100,
});

const age = (one: Shown) => [(one.since ?? 0) * 1000, 0] as const;

/** Each row as a line: `same <keys>` for a group, `chat <keys>` for a chat's asks. */
const rowsOf = (asks: readonly Shown[]) =>
  withSame(inOrder(asks, age, NOW)).map((row: AskRow) =>
    row.kind === "same"
      ? `same ${row.same.placed.map(({ ask }) => ask.ask).join(" ")}`
      : `chat ${row.group.placed.map(({ ask }) => ask.ask).join(" ")}`,
  );

describe("identical requests from several chats", () => {
  it.each<{ name: string; asks: Shown[]; rows: string[] }>([
    {
      name: "the same host from three chats is one row",
      asks: [host(1, "api.example.com"), host(2, "api.example.com"), host(3, "api.example.com")],
      rows: ["same host:1:api.example.com host:2:api.example.com host:3:api.example.com"],
    },
    {
      name: "different hosts stay apart",
      asks: [
        host(1, "api.example.com", { since: 100 }),
        host(2, "cdn.example.com", { since: 200 }),
      ],
      rows: ["chat host:1:api.example.com", "chat host:2:cdn.example.com"],
    },
    {
      name: "the same target asked by different kinds stays apart",
      asks: [
        host(1, "api.example.com", { since: 100 }),
        permission(2, "The sandbox refused api.example.com", { since: 200 }),
      ],
      rows: ["chat host:1:api.example.com", "chat perm:2:The sandbox refused api.example.com"],
    },
    {
      name: "the same tool and argument is one row",
      asks: [
        permission(1, "Bash: curl api.example.com"),
        permission(2, "Bash: curl api.example.com"),
      ],
      rows: ["same perm:1:Bash: curl api.example.com perm:2:Bash: curl api.example.com"],
    },
    {
      name: "one chat asking alone is its own row",
      asks: [host(1, "api.example.com")],
      rows: ["chat host:1:api.example.com"],
    },
    {
      name: "a run grant's identical command from two chats is one row",
      asks: [runGrant(1, "curl -s api.example.com"), runGrant(2, "curl -s api.example.com")],
      rows: ["same run:1:curl -s api.example.com run:2:curl -s api.example.com"],
    },
    {
      name: "run grants bound to different commands are never grouped (ADR 0112)",
      asks: [runGrant(1, "curl -s api.example.com"), runGrant(2, "curl -s other.example.com")],
      rows: ["chat run:1:curl -s api.example.com", "chat run:2:curl -s other.example.com"],
    },
    {
      name: "an ask answered only in its chat is never grouped",
      asks: [reply(1), reply(2)],
      rows: ["chat reply:1", "chat reply:2"],
    },
    {
      name: "a chat's other asks keep their row, after the group",
      asks: [
        host(1, "api.example.com", { since: 100 }),
        reply(1),
        host(2, "api.example.com", { since: 300 }),
      ],
      rows: ["same host:1:api.example.com host:2:api.example.com", "chat reply:1"],
    },
    {
      name: "the group takes the place of its most urgent member",
      asks: [
        reply(1),
        host(2, "api.example.com", { since: 900 }),
        host(3, "api.example.com", { since: 900, expires: 1_030 }),
      ],
      rows: ["same host:3:api.example.com host:2:api.example.com", "chat reply:1"],
    },
    {
      name: "a group stands after a more urgent chat",
      asks: [
        permission(1, "Bash: ls", { since: 500, expires: 1_010 }),
        host(2, "api.example.com", { since: 100 }),
        host(3, "api.example.com", { since: 100 }),
      ],
      rows: ["chat perm:1:Bash: ls", "same host:2:api.example.com host:3:api.example.com"],
    },
  ])("$name", ({ asks, rows }) => {
    expect(rowsOf(asks)).toEqual(rows);
  });

  it("answers each with its narrowest grant, the same for all", () => {
    const [row] = withSame(inOrder([host(1, "a.example"), host(2, "a.example")], age, NOW));
    expect(row.kind === "same" && row.same.grant).toEqual(allow("chat", "Allow for this chat"));
  });

  it("never groups an ask whose row offers no grant", () => {
    const asks = [permission(1, "Bash: ls"), permission(2, "Bash: ls")];
    const rows = withSame(inOrder(asks, age, NOW), (ask) =>
      ask.options.filter((one) => !one.allows),
    );
    expect(rows.map((row) => row.kind)).toEqual(["chat", "chat"]);
  });

  it("never groups a high-risk ask", () => {
    const risk = {
      class: "production-host",
      why: "Reaches api.prod.example, a host that looks like production",
      confirm: "Allow reaching api.prod.example?",
    } as const;
    const asks = [
      { ...host(1, "api.prod.example"), risk },
      { ...host(2, "api.prod.example"), risk },
    ];
    expect(highRisk(asks[0])).toBe(true);
    expect(highRisk(host(1, "a.example"))).toBe(false);
    const rows = withSame(inOrder(asks, age, NOW));
    expect(rows.map((row) => row.kind)).toEqual(["chat", "chat"]);
  });
});

describe("what a group's row says", () => {
  const said = (asks: Shown[]) => {
    const [row] = withSame(inOrder(asks, age, NOW));
    return row.kind === "same" ? sameSaid(row.same) : undefined;
  };
  it.each<{ name: string; asks: Shown[]; says: string }>([
    {
      name: "a host",
      asks: [host(1, "api.example.com"), host(2, "api.example.com"), host(3, "api.example.com")],
      says: "3 chats want to reach api.example.com",
    },
    {
      name: "a command",
      asks: [permission(1, "Bash: curl x"), permission(2, "Bash: curl x")],
      says: "2 chats ask for the same: Bash: curl x",
    },
    {
      name: "a program with a vault",
      asks: [runGrant(1, "curl x"), runGrant(2, "curl x")],
      says: "2 chats want to run curl with vault team",
    },
  ])("$name: $says", ({ asks, says }) => {
    expect(said(asks)).toBe(says);
  });
});
