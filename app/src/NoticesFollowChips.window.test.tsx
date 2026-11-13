// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, type UpdateRow, type Updates } from "./Inbox";
import type { Elsewhere } from "./InboxElsewhere";
import { Notice } from "./Notice";
import { forgetInbox, noteAnswered } from "./inboxRules";
import { forgetAsksScope } from "./asksScope";
import { forgetProjectViews } from "./projectViews";
import type { InboxUpdate, Shown } from "./bindings";

/**
 * **Notices and updates under the chips follow them** (ADR 0121, amended 2026-11-12, decision 9;
 * #6345): one about a chat is listed in every scope that holds the chat; one about no chat (the
 * doctor, a persona's tools, a stale clone), or a chat of another project, under Project alone.
 * The Panel's Notices view draws no chips and lists every one, whatever was picked in Asks.
 */

beforeEach(() => {
  mockIPC(() => null);
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  forgetAsksScope(PLANE);
});

const PLANE = "/home/dev/plane";
const OPS = "/home/dev/ops";

const permission = (session: number, ask: string): Shown => ({
  session,
  ask,
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`steward ${session}`],
  answer: { via: "hook" },
});

/** Chats 3, 4 and 6 work in alpha, chat 5 in beta. Chat 3 dispatched 4; 6 is 3's sibling. */
const FILED: Record<number, string> = { 3: "alpha", 4: "alpha", 6: "alpha", 5: "beta" };
const PARENT: Record<number, number> = { 4: 3, 3: 1, 6: 1 };
/** The strip on alpha, chat 3 focused with its one task. */
const IN_FRONT = {
  here: "alpha",
  workspaceOf: (session: number) => FILED[session],
  workspaceNamed: (key: string) => key,
  chat: { session: 3, name: "steward 3", tasks: 1 },
  askedBy: (session: number) => PARENT[session],
};
const ASKS = [permission(3, "a"), permission(6, "b"), permission(5, "c")];

const elsewhere = (over: Partial<Elsewhere> = {}): Elsewhere => ({
  asking: [],
  quiet: [],
  onPress: vi.fn(),
  waitingHere: [{ plane: PLANE, project: "plane", count: ASKS.length }],
  ...over,
});

function draw(more: Partial<Parameters<typeof Inbox>[0]> = {}) {
  return render(
    <Inbox
      part="asks"
      plane={PLANE}
      asks={ASKS}
      onGo={vi.fn()}
      onLeave={vi.fn()}
      {...IN_FRONT}
      {...more}
    />,
  );
}

const pick = (scope: RegExp) =>
  act(() =>
    within(screen.getByRole("radiogroup", { name: "Show what needs you in" }))
      .getByRole("radio", { name: scope })
      .click(),
  );

/** Which of `causes` are drawn now, in that order. */
const drawn = (causes: readonly string[]) =>
  causes.filter((cause) => document.querySelector(`[data-cause="${cause}"]`) !== null);

/** What each chip lists of `causes`, Chat, Workspace and Project in turn. */
function byScope(causes: readonly string[]): Record<string, string[]> {
  const listed: Record<string, string[]> = {};
  for (const [name, chip] of [
    ["chat", /^Chat/],
    ["workspace", /^Workspace/],
    ["project", /^Project/],
  ] as const) {
    pick(chip);
    listed[name] = drawn(causes);
  }
  return listed;
}

describe("a chat's Notice under the chips", () => {
  const said = (session: number) => {
    noteAnswered(PLANE, {
      at: 1_000 + session,
      session,
      chain: [`steward ${session}`],
      says: "Use the vault",
      answer: "Keep blocked",
      said: "steward already holds it.",
    });
    return `answer-said:${1_000 + session}:steward ${session}:Use the vault`;
  };

  it("is listed under Chat, Workspace and Project when its chat is the focused chat's task", () => {
    const task = said(4);
    draw();
    expect(byScope([task])).toEqual({ chat: [task], workspace: [task], project: [task] });
  });

  it("is listed under Workspace and Project for a sibling chat of the same workspace", () => {
    const sibling = said(6);
    draw();
    expect(byScope([sibling])).toEqual({ chat: [], workspace: [sibling], project: [sibling] });
  });

  it("is listed only under Project when its chat works in another workspace", () => {
    const beta = said(5);
    const quiet = `chat-quiet:${PLANE}#5`;
    draw({
      elsewhere: elsewhere({
        quiet: [{ name: "codex 5", project: "plane", plane: PLANE, session: 5 }],
      }),
    });
    expect(byScope([beta, quiet])).toEqual({ chat: [], workspace: [], project: [beta, quiet] });
  });

  it("follows the focused chat's quiet task under every chip", () => {
    const quiet = `chat-quiet:${PLANE}#4`;
    draw({
      elsewhere: elsewhere({
        quiet: [{ name: "codex 4", project: "plane", plane: PLANE, session: 4 }],
      }),
    });
    expect(byScope([quiet])).toEqual({ chat: [quiet], workspace: [quiet], project: [quiet] });
  });
});

describe("a Notice of no chat under the chips", () => {
  it("is listed only under Project: purlis's own answer, and another project's quiet chat", () => {
    noteAnswered(PLANE, {
      at: 7,
      session: 0,
      chain: ["purlis"],
      says: "Sign in",
      answer: "Keep blocked",
      said: "Sign in from Settings.",
    });
    const purlis = "answer-said:7:purlis:Sign in";
    const there = `chat-quiet:${OPS}#2`;
    draw({
      elsewhere: elsewhere({
        quiet: [{ name: "codex 2", project: "ops", plane: OPS, session: 2 }],
      }),
    });
    expect(byScope([purlis, there])).toEqual({ chat: [], workspace: [], project: [purlis, there] });
  });

  it("the doctor's, a persona's tools' and a stale clone's are listed only under Project", () => {
    const causes = ["doctor-finding:hooks", "persona-hosts:steward", "clone-stale:web"];
    draw({
      part: undefined,
      notices: (
        <>
          {causes.map((cause) => (
            <Notice key={cause} cause={cause} onDismiss={() => undefined}>
              {cause}
            </Notice>
          ))}
        </>
      ),
    });
    expect(byScope(causes)).toEqual({ chat: [], workspace: [], project: causes });
  });
});

describe("updates under the chips", () => {
  const update = (key: string, session: number | null): InboxUpdate => ({
    key,
    kind: "task-done",
    at: 1,
    session,
    chain: session === null ? ["purlis"] : [`steward ${session}`],
    says: `${key}: done`,
    read: false,
  });
  const updates = (list: InboxUpdate[]): Updates => ({
    rows: list.map((one): UpdateRow => ({ update: one, dismiss: vi.fn() })),
    onMarkAllRead: vi.fn(),
    onDismissAll: vi.fn(),
  });
  /** The updates listed now, by what each says. */
  const listed = () => {
    const rows = [...document.querySelectorAll<HTMLElement>('[role="row"].inbox-update')];
    return ["task", "sibling", "beta", "none"].filter((key) =>
      rows.some((row) => row.textContent?.includes(`${key}: done`)),
    );
  };

  it("follow the same rule: a chat's in every scope that holds it, one of no chat under Project", () => {
    draw({
      part: undefined,
      updates: updates([
        update("task", 4),
        update("sibling", 6),
        update("beta", 5),
        update("none", null),
      ]),
    });
    const seen: Record<string, string[]> = {};
    for (const [name, chip] of [
      ["chat", /^Chat/],
      ["workspace", /^Workspace/],
      ["project", /^Project/],
    ] as const) {
      pick(chip);
      seen[name] = listed();
    }
    expect(seen).toEqual({
      chat: ["task"],
      workspace: ["task", "sibling"],
      project: ["task", "sibling", "beta", "none"],
    });
  });
});

describe("the Panel's Notices view", () => {
  it("lists every Notice, whatever is picked in Asks", () => {
    const causes = ["doctor-finding:hooks", "clone-stale:web"];
    draw();
    pick(/^Chat/);
    cleanup();
    draw({
      part: "notices",
      notices: (
        <>
          {causes.map((cause) => (
            <Notice key={cause} cause={cause} onDismiss={() => undefined}>
              {cause}
            </Notice>
          ))}
        </>
      ),
    });
    expect(screen.queryByRole("radiogroup")).toBeNull();
    expect(drawn(causes)).toEqual(causes);
  });
});
