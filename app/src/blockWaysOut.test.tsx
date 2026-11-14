// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatLogRow, FinishedBlock, FinishedTurn } from "./bindings";
import { ChatLogRows } from "./ChatLogTab";
import { Inbox } from "./Inbox";
import { finishUpdates, finishWaysOf, groupedFinishes } from "./finishRows";
import { BlockWaysOut, FinishRowWays } from "./BlockWaysOut";
import {
  GO_TO_CHAT,
  SHOW_THIS_CHECK,
  SHOW_THIS_GUARD,
  TELL_THE_CHAT,
  TOLD,
  TYPE_IT,
} from "./blockWays";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";

/**
 * **A block's ways out** (T1-33; R-184, R-189, R-191 to R-194, R-199, R-300): a guard block
 * offers Tell the chat… and Show this guard, and no Allow; a block by the harness's own check
 * offers Tell the chat… and Show this check, says only that check decides, and offers no Copy
 * command; a Can't-tell block offers Go to chat. Each is on the block's Chat log row and on its
 * turn's finish row, and never on the marker.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
});

type Blocked = Extract<ChatLogRow, { kind: "blocked" }>;

const blocked = (over: Partial<Blocked> = {}): Blocked => ({
  kind: "blocked",
  at: "2026-11-13T09:10:00Z",
  tool: "Bash",
  source: "guard",
  qualifier: null,
  target_kind: "guard_arm",
  target: "A3b",
  harness: "claude",
  reason: null,
  resolved: false,
  ...over,
});

const GUARD = blocked();
const CHECK = blocked({ source: "harness_check", target_kind: null, target: null });
const CANT_TELL = blocked({ source: "sandbox", qualifier: "cant_tell", target: null });

/** The group A3b is in, as ADR 0124 decision 2.2 names it. */
const GROUP = "Project folder";

/** What a press of Tell the chat… or Go to chat reached, in order. */
let pressed: string[] = [];

const WAYS = {
  tell: () => {
    pressed.push("tell");
    return Promise.resolve({ typed: TOLD });
  },
  go: () => pressed.push("go"),
};

/** The one Chat log row drawn for `row`, with its ways out. */
function rowOf(row: Blocked): HTMLElement {
  pressed = [];
  render(<ChatLogRows name="steward 3" rows={[row]} ways={WAYS} />);
  const [item] = within(screen.getByRole("list", { name: "Chat log of steward 3" })).getAllByRole(
    "listitem",
  );
  return item;
}

/** The names of the buttons in `place`. */
const buttonsOf = (place: HTMLElement) =>
  within(place)
    .queryAllByRole("button")
    .map((one) => one.textContent?.trim() ?? "");

/** The ways out a row carries, by their words: the buttons of its ways' group. */
const waysOf = (place: HTMLElement) => {
  const group = within(place).queryByRole("group", { name: /^Ways out/ });
  return group === null ? [] : buttonsOf(group);
};

describe("a guard block's ways out", () => {
  it("a guard block offers Tell the chat and Show this guard", () => {
    expect(waysOf(rowOf(GUARD))).toEqual([TELL_THE_CHAT, SHOW_THIS_GUARD]);
    expect([TELL_THE_CHAT, SHOW_THIS_GUARD]).toEqual(["Tell the chat…", "Show this guard"]);
  });

  it("a guard block row has no Allow", () => {
    const row = rowOf(GUARD);
    expect(buttonsOf(row).filter((one) => /allow/i.test(one))).toEqual([]);
    expect(row.querySelector('[data-variant="grants"]')).toBeNull();
    // An unverified guard report offers the same, and no Allow either.
    cleanup();
    const unverified = rowOf(blocked({ qualifier: "unverified" }));
    expect(waysOf(unverified)).toEqual([TELL_THE_CHAT, SHOW_THIS_GUARD]);
    expect(buttonsOf(unverified).filter((one) => /allow/i.test(one))).toEqual([]);
  });

  it("the guard group name appears only behind Show this guard", async () => {
    const row = rowOf(GUARD);
    // Not on the row, nor in its detail, which names the source alone.
    expect(row.textContent).not.toContain(GROUP);
    await userEvent.click(within(row).getByRole("button", { name: /reset that would destroy/ }));
    expect(row.textContent).not.toContain(GROUP);
    // Behind Show this guard: a read-only detail, with the group's name and nothing to press.
    const show = within(row).getByRole("button", { name: SHOW_THIS_GUARD });
    expect(show).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(show);
    expect(show).toHaveAttribute("aria-expanded", "true");
    const detail = within(row).getByRole("region", { name: "This guard" });
    expect(detail.textContent).toContain(GROUP);
    expect(within(detail).queryAllByRole("button")).toEqual([]);
    expect(within(detail).queryAllByRole("switch")).toEqual([]);
    expect(within(detail).queryAllByRole("checkbox")).toEqual([]);
    // And gone again as it closes.
    await userEvent.click(show);
    expect(row.textContent).not.toContain(GROUP);
  });
});

describe("a harness-check block's ways out", () => {
  it("a harness-check block offers Tell the chat and Show this check and no Allow", async () => {
    const row = rowOf(CHECK);
    expect(waysOf(row)).toEqual([TELL_THE_CHAT, SHOW_THIS_CHECK]);
    expect(buttonsOf(row).filter((one) => /allow/i.test(one))).toEqual([]);
    expect(row.querySelector('[data-variant="grants"]')).toBeNull();
    // Show this check opens a read-only detail.
    await userEvent.click(within(row).getByRole("button", { name: SHOW_THIS_CHECK }));
    const detail = within(row).getByRole("region", { name: "This check" });
    expect(detail.textContent).toContain("Claude Code");
    expect(within(detail).queryAllByRole("button")).toEqual([]);
  });

  it("a harness-check block says only its own check decides", () => {
    expect(rowOf(CHECK).textContent).toContain(
      "purlis can't allow this; only Claude Code's own check decides",
    );
    cleanup();
    // By the adapter's display name, whichever harness it is.
    expect(rowOf(blocked({ ...CHECK, harness: "codex" })).textContent).toContain(
      "purlis can't allow this; only Codex's own check decides",
    );
    cleanup();
    // A guard's row says nothing of the kind.
    expect(rowOf(GUARD).textContent).not.toContain("can't allow");
  });

  it("a harness-check block offers no Copy command", async () => {
    const row = rowOf(CHECK);
    await userEvent.click(within(row).getByRole("button", { name: SHOW_THIS_CHECK }));
    await userEvent.click(within(row).getByRole("button", { name: TELL_THE_CHAT }));
    const said = buttonsOf(row).join("\n");
    expect(said).not.toMatch(/copy|run it|yourself|in a terminal/i);
    expect(row.querySelector("code, pre, kbd")).toBeNull();
    expect(row.textContent).not.toMatch(/Copy command|run it yourself/i);
  });
});

describe("Tell the chat… is offered only while the block stands (Q272)", () => {
  it("a since-resolved Chat log row offers no Tell", () => {
    expect(waysOf(rowOf(blocked({ resolved: true })))).toEqual([SHOW_THIS_GUARD]);
    cleanup();
    expect(waysOf(rowOf(blocked({ ...CHECK, resolved: true })))).toEqual([SHOW_THIS_CHECK]);
    cleanup();
    // One still unresolved offers it.
    expect(waysOf(rowOf(GUARD))).toEqual([TELL_THE_CHAT, SHOW_THIS_GUARD]);
  });
});

describe("a Can't-tell block's way out", () => {
  it("a Can't-tell block offers Go to chat", async () => {
    const row = rowOf(CANT_TELL);
    expect(waysOf(row)).toEqual([GO_TO_CHAT]);
    expect(GO_TO_CHAT).toBe("Go to chat");
    await userEvent.click(within(row).getByRole("button", { name: GO_TO_CHAT }));
    expect(pressed).toEqual(["go"]);
    // And none of the others: purlis can't tell who blocked it.
    expect(buttonsOf(row)).not.toContain(TELL_THE_CHAT);
  });
});

describe("Tell the chat…", () => {
  it("asks first, then types only on the second press", async () => {
    const row = rowOf(GUARD);
    await userEvent.click(within(row).getByRole("button", { name: TELL_THE_CHAT }));
    // The ellipsis asks first (docs/ui-copy.md): what it will do, and nothing sent yet.
    expect(pressed).toEqual([]);
    const asking = within(row).getByRole("region", { name: "Tell the chat" });
    expect(asking.textContent).toMatch(/one line of purlis's own/);
    await userEvent.click(within(asking).getByRole("button", { name: "Type it in the chat" }));
    expect(pressed).toEqual(["tell"]);
  });

  it("says it typed on a plain status line, never as a press that did nothing", async () => {
    const row = rowOf(GUARD);
    await userEvent.click(within(row).getByRole("button", { name: TELL_THE_CHAT }));
    await userEvent.click(within(row).getByRole("button", { name: TYPE_IT }));
    const typed = await within(row).findByText(TOLD);
    expect(typed).toHaveAttribute("role", "status");
    // NothingDone's line is for a press that did nothing (#1693): never the success.
    expect(typed).not.toHaveClass("inbox-refused");
  });

  it("says why nothing was typed where the chat is not waiting", async () => {
    pressed = [];
    const tell = vi.fn(() =>
      Promise.resolve({
        why: "steward 3 is working, so nothing was typed. Press again once it waits.",
      }),
    );
    render(<BlockWaysOut blocks={[GUARD]} who="steward 3" tell={tell} go={() => undefined} />);
    await userEvent.click(screen.getByRole("button", { name: TELL_THE_CHAT }));
    await userEvent.click(screen.getByRole("button", { name: "Type it in the chat" }));
    await waitFor(() =>
      expect(screen.getByText(/is working, so nothing was typed/)).toBeInTheDocument(),
    );
    expect(tell).toHaveBeenCalledTimes(1);
  });
});

describe("where the ways out are", () => {
  const AT = Math.floor(Date.parse("2026-11-13T11:00:00Z") / 1000);
  const TURN: FinishedTurn = {
    session: 3,
    key: "01TURNUNRESOLVED",
    at: AT,
    failed: false,
    blocked: 1,
    blocks: [
      {
        source: "guard",
        qualifier: null,
        target_kind: "guard_arm",
        target: "A3b",
        harness: "claude",
      },
    ],
  };

  it("a block's ways out are on its finish row and its Chat log row and not on the marker", async () => {
    // The Chat log row.
    expect(waysOf(rowOf(GUARD))).toEqual([TELL_THE_CHAT, SHOW_THIS_GUARD]);
    cleanup();

    // The turn's finish row in Updates › Mine, with its turn's unresolved blocks' ways.
    const rows = finishUpdates([TURN], () => "steward 3").map((one) => ({
      update: { ...one, read: false },
      dismiss: () => undefined,
      ways: (
        <BlockWaysOut
          blocks={finishWaysOf([TURN], [one.key])}
          who="steward 3"
          tell={WAYS.tell}
          go={WAYS.go}
        />
      ),
    }));
    render(
      <Inbox
        part="updates"
        plane="/home/dev/plane"
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        updates={{ rows, onMarkAllRead: () => undefined, onDismissAll: () => undefined }}
      />,
    );
    const finish = await screen.findByRole("row", { name: /Finished its turn · 1 blocked/ });
    expect(waysOf(finish)).toEqual([TELL_THE_CHAT, SHOW_THIS_GUARD]);
    await userEvent.click(within(finish).getByRole("button", { name: SHOW_THIS_GUARD }));
    expect(within(finish).getByRole("region", { name: "This guard" }).textContent).toContain(GROUP);

    // A finished turn with no block left unresolved carries none.
    expect(
      finishWaysOf([{ ...TURN, blocked: 0, blocks: [] }], [finishUpdates([TURN], () => "")[0].key]),
    ).toEqual([]);

    // The marker carries none: it opens the Chat log, and draws no way out of its own.
    const marker = readFileSync(join(process.cwd(), "src/BlockedMarker.tsx"), "utf8");
    expect(marker).not.toMatch(/BlockWaysOut|blockWays/);
    for (const label of [TELL_THE_CHAT, SHOW_THIS_GUARD, SHOW_THIS_CHECK, GO_TO_CHAT])
      expect(marker).not.toContain(label);
  });
});

describe("a finish row's ways out (Q272)", () => {
  const PLANE = "/home/dev/plane";
  const AT = Math.floor(Date.parse("2026-11-13T11:00:00Z") / 1000);
  const NAMES: Record<number, string> = {
    3: "steward 3",
    4: "planner 4",
    5: "coder 5",
    6: "reviewer 6",
  };
  const nameOf = (session: number) => NAMES[session] ?? String(session);
  const block = (over: Partial<FinishedBlock>): FinishedBlock => ({
    source: "guard",
    qualifier: null,
    target_kind: null,
    target: null,
    harness: "claude",
    ...over,
  });
  const turn = (session: number, one: FinishedBlock): FinishedTurn => ({
    session,
    key: `01TURN${session}`,
    at: AT - session,
    failed: false,
    blocked: 1,
    blocks: [one],
  });

  /** What `tell_chat_blocked` was asked to type into, in order. */
  let told: number[] = [];
  /** The chats Go to chat went to. */
  let went: number[] = [];

  /** Updates › Mine with the finish rows of `turns`, grouped and wired as PlaneView wires them. */
  function finishRowsOf(turns: readonly FinishedTurn[]) {
    told = [];
    went = [];
    mockIPC((cmd, args) => {
      if (cmd !== "tell_chat_blocked") return null;
      told.push((args as { session: number }).session);
      return null;
    });
    const updates = finishUpdates(turns, nameOf).map((one) => ({ ...one, read: false }));
    const rows = groupedFinishes(updates).map(({ update, keys }) => ({
      update,
      dismiss: () => undefined,
      ways:
        finishWaysOf(turns, keys).length > 0 ? (
          <FinishRowWays
            plane={PLANE}
            turns={turns}
            keys={keys}
            who={update.chain.at(-1) ?? "this chat"}
            nameOf={nameOf}
            go={(session) => went.push(session)}
          />
        ) : undefined,
    }));
    render(
      <Inbox
        part="updates"
        plane={PLANE}
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        updates={{ rows, onMarkAllRead: () => undefined, onDismissAll: () => undefined }}
      />,
    );
  }

  it("a grouped finish row tells only the chats whose block offers Tell, and names them", async () => {
    finishRowsOf([
      turn(3, block({ source: "guard", target_kind: "guard_arm", target: "A3b" })),
      turn(4, block({ source: "sandbox", qualifier: "cant_tell" })),
      turn(5, block({ source: "sandbox" })),
      turn(6, block({ source: "harness_check", harness: "codex" })),
    ]);
    const row = await screen.findByRole("row", { name: /Finished its turn · 1 blocked/ });
    await userEvent.click(within(row).getByRole("button", { name: TELL_THE_CHAT }));
    const asking = within(row).getByRole("region", { name: "Tell the chat" });
    // The confirming text names the chats it types into, and no other.
    expect(asking.textContent).toContain("steward 3");
    expect(asking.textContent).toContain("reviewer 6");
    expect(asking.textContent).not.toContain("planner 4");
    expect(asking.textContent).not.toContain("coder 5");
    await userEvent.click(within(asking).getByRole("button", { name: TYPE_IT }));
    const typed = await within(row).findByText(/^Typed in /);
    await waitFor(() => expect([...told].sort()).toEqual([3, 6]));
    expect(typed.textContent).toContain("steward 3");
    expect(typed.textContent).toContain("reviewer 6");
    expect(typed.textContent).not.toContain("planner 4");
  });

  it("a grouped finish row's Go to chat goes to the chat that holds the Can't-tell block", async () => {
    finishRowsOf([
      turn(3, block({ source: "guard", target_kind: "guard_arm", target: "A3b" })),
      turn(4, block({ source: "sandbox", qualifier: "cant_tell" })),
    ]);
    const row = await screen.findByRole("row", { name: /Finished its turn · 1 blocked/ });
    await userEvent.click(within(row).getByRole("button", { name: GO_TO_CHAT }));
    expect(went).toEqual([4]);
  });

  it("a Can't-tell block on a finish row offers Go to chat, and no Tell or Show", async () => {
    finishRowsOf([turn(4, block({ source: "guard", qualifier: "cant_tell" }))]);
    const row = await screen.findByRole("row", { name: /Finished its turn · 1 blocked/ });
    expect(waysOf(row)).toEqual([GO_TO_CHAT]);
    await userEvent.click(within(row).getByRole("button", { name: GO_TO_CHAT }));
    expect(went).toEqual([4]);
  });

  it("a harness-check block on a finish row names its harness", async () => {
    finishRowsOf([turn(6, block({ source: "harness_check", harness: "codex" }))]);
    const row = await screen.findByRole("row", { name: /Finished its turn · 1 blocked/ });
    expect(waysOf(row)).toEqual([TELL_THE_CHAT, SHOW_THIS_CHECK]);
    expect(row.textContent).toContain("only Codex's own check decides");
    await userEvent.click(within(row).getByRole("button", { name: SHOW_THIS_CHECK }));
    expect(within(row).getByRole("region", { name: "This check" }).textContent).toContain("Codex");
  });
});
