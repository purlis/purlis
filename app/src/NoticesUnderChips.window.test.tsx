// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, type UpdateRow, type Updates } from "./Inbox";
import type { Elsewhere } from "./InboxElsewhere";
import { Notice } from "./Notice";
import { forgetInbox, noteAnswered } from "./inboxRules";
import { forgetAsksScope, pickAsksScope, type AsksScope } from "./asksScope";
import { forgetProjectViews } from "./projectViews";
import type { InboxUpdate, Shown } from "./bindings";

/**
 * **Notices and updates under the chips follow them** (ADR 0121, amended 2026-11-12, decision
 * 9; #6345): one about a chat is listed in every scope that holds its chat, one about no chat
 * only under Project. The Panel's Notices view lists what it did, whatever the chip.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-11-13T12:00:00Z"));
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  forgetAsksScope(PLANE);
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.parse("2026-11-13T11:00:00Z") / 1000);

/** Chats 3, 4 and 8 work in alpha, chat 5 in beta. */
const FILED: Record<number, string> = { 3: "alpha", 4: "alpha", 8: "alpha", 5: "beta" };
/** Chat 3 has focus and dispatched 4, which dispatched 8; chat 5 is nobody's task. */
const PARENT: Record<number, number> = { 4: 3, 8: 4 };
const FRAME = {
  here: "alpha",
  workspaceOf: (session: number) => FILED[session],
  workspaceNamed: (key: string) => key,
  chat: { session: 3, name: "steward 3", tasks: 2 },
  askedBy: (session: number) => PARENT[session],
};

const permission = (session: number): Shown => ({
  session,
  ask: `permission:${session}`,
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

/** One ask in Chat's set and one in beta: every chip is drawn. */
const ASKS = [permission(3), permission(5)];

const elsewhere = (quiet: Elsewhere["quiet"]): Elsewhere => ({
  asking: [],
  quiet,
  onPress: vi.fn(),
  onOpen: vi.fn(),
  waitingHere: [{ plane: PLANE, project: "plane", count: ASKS.length }],
});

const quiet = (session: number, plane = PLANE, project = "plane") => ({
  name: `shell ${session}`,
  project,
  plane,
  session,
});

/** What a quiet chat's Notice says. */
const quietSaid = (session: number) => new RegExp(`shell ${session}.*can.t tell purlis`);

const update = (key: string, says: string, session: number | null): UpdateRow => {
  const one: InboxUpdate = {
    key,
    kind: session === null ? "doctor" : "task-done",
    at: AT,
    session,
    chain: session === null ? [] : [`steward ${session}`],
    says,
    read: false,
  };
  return { update: one, dismiss: vi.fn() };
};

const updates = (rows: UpdateRow[]): Updates => ({
  rows,
  onMarkAllRead: vi.fn(),
  onDismissAll: vi.fn(),
});

/** A Notice of the doctor's: about no chat. */
const DOCTOR = "personas: reviewer has no charter";
const doctor = (
  <Notice cause="doctor:personas" fixes={[{ label: "Got it", onPress: () => {} }]}>
    {DOCTOR}
  </Notice>
);

function draw(more: Partial<Parameters<typeof Inbox>[0]> = {}) {
  return render(
    <Inbox plane={PLANE} asks={ASKS} onGo={vi.fn()} onLeave={vi.fn()} {...FRAME} {...more} />,
  );
}

/** Whether an element says `text`, though its words are split among its parts. */
function says(text: RegExp | string): boolean {
  const holds = (said: string | null) =>
    said !== null && (typeof text === "string" ? said.includes(text) : text.test(said));
  return (
    screen.queryAllByText(
      (_, element) =>
        element !== null &&
        holds(element.textContent) &&
        ![...element.children].some((child) => holds(child.textContent)),
    ).length > 0
  );
}

/** Whether `text` is listed under each chip, picked in turn. */
function listedUnder(text: RegExp | string): Record<AsksScope, boolean> {
  const under = {} as Record<AsksScope, boolean>;
  for (const scope of ["chat", "workspace", "project"] as const) {
    act(() => pickAsksScope(PLANE, scope));
    under[scope] = says(text);
  }
  return under;
}

const EVERY = { chat: true, workspace: true, project: true };
const PROJECT_ONLY = { chat: false, workspace: false, project: true };

describe("Notices under the chips", () => {
  it("lists a Notice about a chat the focused chat dispatched under Chat, Workspace and Project", () => {
    draw({ part: "asks", elsewhere: elsewhere([quiet(8)]) });
    expect(listedUnder(quietSaid(8))).toEqual(EVERY);
  });

  it("lists a Notice about a chat in another workspace only under Project", () => {
    draw({ part: "asks", elsewhere: elsewhere([quiet(5)]) });
    expect(listedUnder(quietSaid(5))).toEqual(PROJECT_ONLY);
  });

  it("lists what an answer said under the scopes that hold its chat", () => {
    noteAnswered(PLANE, {
      at: Date.now(),
      session: 5,
      chain: ["steward 5"],
      says: "Use the vault",
      answer: "Keep blocked",
      said: "Its persona already holds this vault.",
    });
    noteAnswered(PLANE, {
      at: Date.now() - 1000,
      session: 4,
      chain: ["steward 4"],
      says: "Use the vault",
      answer: "Keep blocked",
      said: "Its task already holds this vault.",
    });
    draw({ part: "asks" });
    expect(listedUnder(/persona already holds this vault/)).toEqual(PROJECT_ONLY);
    expect(listedUnder(/task already holds this vault/)).toEqual(EVERY);
  });

  it("lists a Notice about no chat, or another project's chat, only under Project", () => {
    draw({ elsewhere: elsewhere([quiet(7, "/home/dev/ops", "ops")]), notices: doctor });
    expect(listedUnder(DOCTOR)).toEqual(PROJECT_ONLY);
    expect(listedUnder(quietSaid(7))).toEqual(PROJECT_ONLY);
  });

  it("lists updates by the same rule: a chat's where its chat is, a chat-less one under Project", () => {
    draw({
      updates: updates([
        update("task:a", "Fix the build: done", 4),
        update("task:b", "Ship the docs: done", 5),
        update("doctor:x", "The doctor found a stale clone", null),
      ]),
    });
    expect(listedUnder("Fix the build: done")).toEqual(EVERY);
    expect(listedUnder("Ship the docs: done")).toEqual(PROJECT_ONLY);
    expect(listedUnder("The doctor found a stale clone")).toEqual(PROJECT_ONLY);
  });

  it("leaves the Panel's Notices view as it was, whatever the chip", () => {
    draw({ part: "notices", notices: doctor });
    expect(listedUnder(DOCTOR)).toEqual(EVERY);
  });
});
