// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Answer once for all identical requests** (ADR 0121 decision 6, #6009): requests that are
 * exactly identical, from several chats, are one row: "3 chats want to reach api.example.com",
 * with Allow for all, or opened to answer each.
 *
 * Identical is the same kind and the same thing asked of it:
 *
 * - a sandbox's host, folder or service: the same target;
 * - a browser's sites: the same sites;
 * - a permission: the same line, which names its tool and argument (an edit's change and a
 *   command over several lines are each one chat's, and never grouped);
 * - a run grant: the same program, vault, persona, folder and command. ADR 0112 binds a run
 *   grant to one exact command for one chat, so runs whose commands differ are never one row.
 *
 * and the same narrowest grant on every row: Allow for all answers each ask once with it (Allow
 * once, or Allow for this chat), never wider. A high-risk ask is never grouped ({@link highRisk}).
 *
 * The group takes the place of its most urgent member in the order `asksOrder` gives (its
 * Expiring ask, or the one that blocks most, or the longest wait), and each chat's other asks
 * keep their own row after it.
 *
 * Pure: no commands and no markup. `Inbox.tsx` draws a group with `AskDecision`.
 */
import type { Offered, Shown } from "./bindings";
import { waysOf } from "./AskDecision";
import type { Placed, PlacedGroup } from "./asksOrder";

/** One request several chats make alike: their asks, most urgent first, and its one grant. */
export type Same = {
  /** What makes them identical: their kind, what they ask, and the grant. */
  key: string;
  /** Each chat's ask, in the order they stand, each with its own standing. */
  placed: readonly Placed[];
  /** The narrowest grant each row offers, which Allow for all answers each with. */
  grant: Offered;
};

/** A row of Asks: one chat's asks, or one request several chats make alike. */
export type AskRow = { kind: "chat"; group: PlacedGroup } | { kind: "same"; same: Same };

/**
 * **Whether `ask` is high-risk** (ADR 0121 decision 8, #6011): one is never grouped, so it is
 * answered alone with its risk in sight. No ask is classed yet; #6011 fills this seam.
 */
export const highRisk: (ask: Shown) => boolean = () => false;

/** **What `ask` asks, as identical requests share it**, or nothing where it is never grouped. */
function askedOf(ask: Shown): string | undefined {
  switch (ask.source) {
    case "sandbox-host":
    case "sandbox-write":
    case "sandbox-local":
      return ask.answer.via === "sandbox-block" ? ask.answer.shown.target : undefined;
    case "browser-site":
      return ask.answer.via === "browser-sites"
        ? [...ask.answer.sites].sort().join("\n")
        : undefined;
    case "permission":
      return ask.writes == null && ask.lines == null ? ask.says : undefined;
    case "run-grant": {
      const run = ask.run;
      if (run == null || run.offer !== "program" || run.fills.length > 0) return undefined;
      return JSON.stringify([run.program, run.vault, run.persona, run.cwd, run.command]);
    }
    default:
      return undefined;
  }
}

/** The key identical asks share, with the grant that answers each, or nothing. */
function sameOf(
  ask: Shown,
  offers: readonly Offered[],
  risky: (ask: Shown) => boolean,
): { key: string; grant: Offered } | undefined {
  if (risky(ask)) return undefined;
  const asked = askedOf(ask);
  const grant = waysOf(offers).main;
  if (asked === undefined || grant === undefined) return undefined;
  return { key: JSON.stringify([ask.source, asked, grant.id]), grant };
}

/**
 * **The rows of Asks, identical requests from several chats made one** (#6009). `groups` are
 * in the order `inOrder` gives; `offersOf` is what each ask's row offers (an Allow its row
 * withholds is never answered for it here); `risky` is {@link highRisk}, a parameter only so the
 * rule is tested before #6011 classes any ask.
 */
export function withSame(
  groups: readonly PlacedGroup[],
  offersOf: (ask: Shown) => readonly Offered[] = (ask) => ask.options,
  risky: (ask: Shown) => boolean = highRisk,
): AskRow[] {
  const alike = new Map<string, { grant: Offered; placed: Placed[]; chats: Set<number> }>();
  const keyOf = new Map<Placed, string>();
  for (const group of groups)
    for (const one of group.placed) {
      const same = sameOf(one.ask, offersOf(one.ask), risky);
      if (same === undefined) continue;
      const held = alike.get(same.key) ?? { grant: same.grant, placed: [], chats: new Set() };
      held.placed.push(one);
      held.chats.add(one.ask.session);
      alike.set(same.key, held);
      keyOf.set(one, same.key);
    }
  /** Only a request from more than one chat is a group. */
  const grouped = (one: Placed) => {
    const key = keyOf.get(one);
    return key !== undefined && (alike.get(key)?.chats.size ?? 0) > 1 ? key : undefined;
  };
  const rows: AskRow[] = [];
  const drawn = new Set<string>();
  for (const group of groups) {
    // A group stands where its most urgent member's chat stands, before that chat's other asks.
    for (const one of group.placed) {
      const key = grouped(one);
      const held = key === undefined ? undefined : alike.get(key);
      if (key === undefined || held === undefined || drawn.has(key)) continue;
      drawn.add(key);
      rows.push({ kind: "same", same: { key, grant: held.grant, placed: held.placed } });
    }
    const placed = group.placed.filter((one) => grouped(one) === undefined);
    if (placed.length === 0) continue;
    rows.push({
      kind: "chat",
      group: { ...group, placed, asks: placed.map(({ ask }) => ask) },
    });
  }
  return rows;
}

/** How many chats make the request. */
export const chatsIn = (same: Same): number =>
  new Set(same.placed.map(({ ask }) => ask.session)).size;

/** **What a group's row says** (#6009): how many chats, and what they all want. */
export function sameSaid(same: Same): string {
  const first = same.placed[0].ask;
  const chats = `${chatsIn(same)} chats`;
  const { answer, run } = first;
  if (answer.via === "sandbox-block") {
    const where = answer.shown.target;
    return first.source === "sandbox-write"
      ? `${chats} want to write in ${where}`
      : `${chats} want to reach ${where}`;
  }
  if (answer.via === "browser-sites")
    return `${chats} want their browser to reach ${answer.sites.join(", ")}`;
  if (run != null) {
    const program = run.program.split("/").pop() ?? run.program;
    return `${chats} want to run ${program} with vault ${run.vault}`;
  }
  return `${chats} ask for the same: ${first.says}`;
}
