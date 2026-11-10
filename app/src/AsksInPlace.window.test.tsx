// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import { IN_ANOTHER_WINDOW, type Elsewhere } from "./InboxElsewhere";
import { MOVED_JUST_NOW, SETTLED_ELSEWHERE } from "./InboxAsks";
import { ALLOW_FOR_ALL } from "./SameAskRow";
import { SETTLE_MS } from "./TaskBlocksNotice";
import { forgetInbox } from "./inboxRules";
import { keepInboxScope } from "./inboxScope";
import { forgetProjectViews } from "./projectViews";
import type { Needing } from "./NeedsYou";
import type { Shown } from "./bindings";
import { chatGroup } from "./test-inbox";

/**
 * **Another open project's ask, answered in place from Asks** (#1921, ADR 0121 decisions 4 and
 * 6): Asks lists every open project's asks, this window's other projects' and the other
 * windows', as rows of its own: the same options on the row, the same settle guard, New until
 * seen, one order by facts and one row for identical requests across projects. An answer goes to
 * the project that holds the ask, and one that arrives after the ask was answered elsewhere
 * sends nothing and says so on the row.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
  keepInboxScope(PLANE, "all");
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";
const WEB = "/home/dev/web";
const OPS = "/home/dev/ops";

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const host = (session: number, target = "api.example.com", more: Partial<Shown> = {}): Shown => ({
  session,
  ask: `block:${session}:connect:host:${target}`,
  says: `The sandbox refused ${target}`,
  options: [
    { id: "chat", label: "Allow only for this chat", allows: true },
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: [`steward ${session}`],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target },
  },
  ...more,
});

const permission = (session: number, says: string, more: Partial<Shown> = {}): Shown => ({
  session,
  ask: `permission-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: [`dev ${session}`],
  answer: { via: "hook" },
  ...more,
});

const question = (session: number): Shown => ({
  session,
  ask: `question:${session}`,
  says: "Which port?",
  options: [],
  source: "question",
  chain: [`dev ${session}`],
  answer: { via: "in-its-pane" },
  last_message: "Which port should the dev server use?",
});

type Calls = { cmd: string; args: Record<string, unknown> }[];

/** The core: each project's asks as `asks_waiting` reads them now, and every command heard. */
function core(waiting: Record<string, readonly Shown[]>) {
  const calls: Calls = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    calls.push({ cmd, args: a });
    if (cmd === "asks_waiting")
      return { plane: a.plane, asks: waiting[a.plane as string] ?? [], with_asking_chat: [] };
    if (cmd === "allow_sandbox_block") return { said: "Allowed.", live: true };
    return null;
  });
  return calls;
}

const sent = (calls: Calls, cmd: string) => calls.filter((one) => one.cmd === cmd);

function draw(asks: readonly Shown[], elsewhere: Partial<Elsewhere>, more = {}) {
  const there: Elsewhere = { asking: [], quiet: [], onPress: vi.fn(), ...elsewhere };
  const onAnswered = vi.fn();
  render(
    <Inbox
      part="asks"
      plane={PLANE}
      asks={asks}
      onGo={vi.fn()}
      onLeave={vi.fn()}
      onAnswered={onAnswered}
      elsewhere={there}
      {...more}
    />,
  );
  return { there, onAnswered };
}

/** WEB, a project of this window, holding `asks`. */
const webHere = (asks: readonly Shown[]): Partial<Elsewhere> => ({
  waitingHere: [
    { plane: PLANE, project: "plane", count: 0 },
    { plane: WEB, project: "web", count: asks.length, asks },
  ],
});

describe("another project's ask, in place in Asks (#1921)", () => {
  it("draws a decision of another project with its options on its row", () => {
    core({});
    draw([], webHere([host(4)]));
    const group = chatGroup("steward 4");
    expect(group).toHaveAttribute("data-elsewhere", `${WEB}#4`);
    expect(within(group).getByText("Allow reaching this host?")).toBeTruthy();
    expect(
      within(group)
        .getAllByRole("button")
        .map((one) => one.textContent),
    ).toEqual(expect.arrayContaining(["Allow only for this chat", "Keep blocked", "Go to chat"]));
  });

  it("answers it in the project that holds it, once it still waits there", async () => {
    const ask = host(4);
    const calls = core({ [WEB]: [ask] });
    const { onAnswered } = draw([], webHere([ask]));
    read();

    await userEvent.click(
      within(chatGroup("steward 4")).getByRole("button", { name: "Allow only for this chat" }),
    );

    await waitFor(() => expect(sent(calls, "allow_sandbox_block")).toHaveLength(1));
    expect(sent(calls, "allow_sandbox_block")[0].args).toMatchObject({
      plane: WEB,
      session: 4,
      level: "chat",
    });
    // Its project's asks were read again first, and this project's own were not asked.
    expect(sent(calls, "asks_waiting").map(({ args }) => args.plane)).toContain(WEB);
    // What this project does after an answer of its own is not done for another's.
    expect(onAnswered).not.toHaveBeenCalled();
  });

  it("sends nothing, and says so on the row, where the ask was answered elsewhere meanwhile", async () => {
    // Its window still lists it; its project's registry no longer holds it.
    const calls = core({ [WEB]: [] });
    draw([], webHere([host(4)]));
    read();

    await userEvent.click(
      within(chatGroup("steward 4")).getByRole("button", { name: "Allow only for this chat" }),
    );

    expect(await within(chatGroup("steward 4")).findByRole("status")).toHaveTextContent(
      SETTLED_ELSEWHERE,
    );
    expect(sent(calls, "allow_sandbox_block")).toHaveLength(0);
  });

  it("holds an Allow pressed just after the row was drawn, as this project's are held", async () => {
    const ask = host(4);
    const calls = core({ [WEB]: [ask] });
    draw([], webHere([ask]));

    await userEvent.click(
      within(chatGroup("steward 4")).getByRole("button", { name: "Allow only for this chat" }),
    );

    expect(within(chatGroup("steward 4")).getByRole("status")).toHaveTextContent(MOVED_JUST_NOW);
    expect(sent(calls, "allow_sandbox_block")).toHaveLength(0);
  });

  it("draws two projects' asks of the same chat number and id as two rows", () => {
    core({});
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    draw([question(6)], webHere([question(6)]));
    expect(document.querySelectorAll('[role="row"].inbox-ask')).toHaveLength(2);
    expect(document.querySelector(`[data-elsewhere="${WEB}#6"]`)).not.toBeNull();
    expect(errors).not.toHaveBeenCalledWith(expect.stringContaining("Duplicate key"));
    errors.mockRestore();
  });

  it("keeps a visible Answer on a typed ask of another project", () => {
    core({});
    draw([], webHere([question(6)]));
    expect(within(chatGroup("dev 6")).getByRole("button", { name: /^Answer/ })).toBeTruthy();
  });

  it("orders every project's asks by their facts, another's Expiring ask first", () => {
    core({});
    const now = Math.floor(Date.now() / 1000);
    draw(
      [permission(3, "Run cargo test", { chain: ["steward 3"], since: now - 600 })],
      webHere([permission(5, "Run npm test", { since: now - 10, expires: now + 30 })]),
    );
    const order = [...document.querySelectorAll('.inbox-chat [role="rowheader"]')].map(
      (one) => one.textContent,
    );
    expect(order).toEqual(["dev 5", "steward 3"]);
    expect(screen.getByText("Expires in 30 s")).toBeTruthy();
  });

  it("makes identical requests from chats in two projects one row, Allow for all answering each in its own", async () => {
    // The same session number in each project: two chats, not one.
    const mine = host(3);
    const theirs = host(3);
    const calls = core({ [WEB]: [theirs] });
    draw([mine], webHere([theirs]));
    expect(screen.getByText("2 chats want to reach api.example.com")).toBeTruthy();
    read();

    await userEvent.click(screen.getByRole("button", { name: ALLOW_FOR_ALL }));

    await waitFor(() => expect(sent(calls, "allow_sandbox_block")).toHaveLength(2));
    expect(sent(calls, "allow_sandbox_block").map(({ args }) => [args.plane, args.level])).toEqual([
      [PLANE, "chat"],
      [WEB, "chat"],
    ]);
  });

  it("never groups another project's risky ask with this project's same request (#6011)", () => {
    core({});
    const risky: Shown = {
      ...host(3),
      risk: {
        class: "production-host",
        why: "Reaches api.example.com, which looks like production",
        confirm: "Allow reaching api.example.com?",
      },
    };
    draw([host(3)], webHere([risky]));
    expect(screen.queryByText("2 chats want to reach api.example.com")).toBeNull();
    expect(document.querySelector(`[data-elsewhere="${WEB}#3"]`)).not.toBeNull();
  });

  it("offers no answer that opens something in its project's window, where that project's Asks does it", () => {
    core({});
    const proposal: Shown = {
      session: 0,
      ask: "proposed:1",
      says: "purlis proposes a change to purlis.toml",
      options: [
        { id: "open", label: "Open", allows: false },
        { id: "dismiss", label: "Dismiss", allows: false },
      ],
      source: "proposed-change",
      chain: ["purlis"],
      answer: { via: "proposed-change", id: 1, path: "purlis.toml" },
    };
    draw([], webHere([proposal]));
    const group = chatGroup("purlis");
    // Its Answer opens what it offers here: Dismiss, and no Open.
    expect(within(group).queryByRole("button", { name: "Open" })).toBeNull();
  });

  it("draws another window's project ask by ask, and its chats not again a chat at a time", () => {
    core({});
    const away: Needing = {
      plane: OPS,
      project: "ops",
      session: 7,
      name: "steward 7",
      workspace: "infra",
    };
    draw([], {
      asking: [away],
      there: [{ plane: OPS, project: "ops", count: 1, asks: [host(7)] }],
    });
    const group = chatGroup("steward 7");
    expect(group).toHaveAttribute("data-elsewhere", `${OPS}#7`);
    expect(within(group).queryByText(IN_ANOTHER_WINDOW)).toBeNull();
    expect(within(group).getByRole("button", { name: "Allow only for this chat" })).toBeTruthy();
    expect(document.querySelectorAll(`[data-elsewhere="${OPS}#7"]`)).toHaveLength(1);
  });

  it("still draws a chat of another window a chat at a time where that window told no asks", () => {
    core({});
    const away: Needing = {
      plane: OPS,
      project: "ops",
      session: 7,
      name: "steward 7",
      workspace: "infra",
    };
    draw([], { asking: [away] });
    expect(within(chatGroup("steward 7")).getByText(IN_ANOTHER_WINDOW)).toBeTruthy();
  });

  it("draws another project's ask New by that project's own seen record", () => {
    core({});
    const fresh = host(4);
    const old = host(5, "old.example.com");
    draw([], webHere([fresh, old]), {
      // WEB's record, which has seen the old one: this project's own is not read for them.
      seenOf: (plane: string) => ({
        isNew: (key: string) => plane === WEB && key !== `ask:5:${old.ask}`,
        watch: () => () => undefined,
      }),
    });
    const isNew = (session: number) =>
      chatGroup(`steward ${session}`).querySelector(".inbox-ask")?.getAttribute("data-new");
    expect(isNew(4)).not.toBeNull();
    expect(isNew(5)).toBeNull();
  });
});
