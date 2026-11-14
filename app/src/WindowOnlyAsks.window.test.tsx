// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox } from "./Inbox";
import type { FinishedTurn, Shown } from "./bindings";
import { useAsks } from "./asks";
import { fixedChats, nothingKnown, type ChatStates } from "./chatState";
import { finishRowsOf, useFinishedTurns } from "./finishRows";
import { forgetInbox } from "./inboxRules";
import { forgetProjectViews } from "./projectViews";
import { SETTLE_MS } from "./TaskBlocksNotice";

/**
 * **A window-only ask is Blocking while a chat waits on it, and otherwise sits on its chat's
 * finish row** (T1-42, R-329, Q181), against the asks registry (`useAsks`) with the core faked
 * at its command boundary. The core decides which: a chat in its turn waits on its Trust this
 * clone, so it is in `asks`, Blocking; a chat whose turn has ended does not, so it is listed
 * apart in `on_finish_rows`, drawn on that chat's finish row with its control, and not in Asks.
 */

const PLANE = "/home/dev/plane";
const AT = Math.floor(Date.parse("2026-11-13T11:00:00Z") / 1000);

/** Chat 2's refused mint, for an unrecorded clone (R-330). */
const TRUST_CLONE: Shown = {
  session: 2,
  ask: "trust-clone:2:workspaces/alpha/api|github.com/acme/api",
  says: "Trust this clone so chats can use GitHub: workspaces/alpha/api was not cloned by purlis, and its remote is github.com/acme/api. Trust it only if you made this clone yourself.",
  options: [{ id: "press", label: "Trust this clone", allows: true }],
  source: "trust-clone",
  chain: ["steward 2"],
  answer: { via: "trust-clone", folder: "workspaces/alpha/api", place: "github.com/acme/api" },
  since: AT - 60,
};

/** Chat 2's turn that ended after the refusal, as the core counts it. */
const ENDED: FinishedTurn = { session: 2, key: "01TURNENDED", at: AT, failed: false, blocked: 0 };

const WAITING: ChatStates = { ...nothingKnown, bySession: { 2: "waiting" }, movedAt: { 2: 7 } };
const CHATS = fixedChats(WAITING);

let asked: { cmd: string; args: unknown }[] = [];

/** The core: its asks, those it lists apart for a finish row, and chat 2's finishes. */
function core({
  asks,
  onFinishRows,
  turns,
  refuses,
}: {
  asks: readonly Shown[];
  onFinishRows: readonly Shown[];
  turns: readonly FinishedTurn[];
  refuses?: string;
}) {
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "asks_waiting")
      return { plane: PLANE, asks, with_asking_chat: [], on_finish_rows: onFinishRows };
    if (cmd === "finished_turns") return turns;
    if (cmd === "vouch_clone") {
      if (refuses !== undefined) throw refuses;
      return "github.com/acme/api";
    }
    return null;
  });
}

beforeEach(() => {
  asked = [];
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
});

/** Asks and Updates › Mine, as the project's view draws them from the registry. */
function Window() {
  const registry = useAsks([PLANE]);
  const turns = useFinishedTurns(PLANE, CHATS);
  const rows = finishRowsOf({
    plane: PLANE,
    turns,
    asks: registry.questions[PLANE]?.on_finish_rows ?? [],
    nameOf: () => "steward 2",
    reread: () => registry.reread(PLANE),
  }).map((row) => ({ ...row, update: { ...row.update, read: false }, dismiss: () => undefined }));
  return (
    <>
      <Inbox
        part="asks"
        plane={PLANE}
        asks={registry.held[PLANE] ?? []}
        onGo={() => undefined}
        onLeave={() => undefined}
      />
      <Inbox
        part="updates"
        plane={PLANE}
        asks={[]}
        onGo={() => undefined}
        onLeave={() => undefined}
        updates={{ rows, onMarkAllRead: () => undefined, onDismissAll: () => undefined }}
      />
    </>
  );
}

/** The person reads what was drawn for longer than an answer waits for its row to settle. */
const read = () => new Promise((done) => setTimeout(done, SETTLE_MS + 50));

const askRow = () => document.querySelector<HTMLElement>('[role="row"].inbox-ask');
const finishRow = () =>
  document.querySelector<HTMLElement>('[role="row"].inbox-update[data-kind="turn-finished"]');

describe("a window-only ask, Blocking or on its chat's finish row (T1-42, R-329)", () => {
  it("a window-only ask is Blocking when a chat waits and otherwise sits on the finish row", async () => {
    // A chat waits on it: Blocking, in Asks with its control, and on no finish row.
    core({ asks: [TRUST_CLONE], onFinishRows: [], turns: [ENDED] });
    const waits = render(<Window />);
    await waitFor(() => expect(askRow()).not.toBeNull());
    expect(askRow()).toHaveTextContent("Trust this clone so chats can use GitHub");
    expect(
      within(askRow() as HTMLElement).getByRole("button", { name: /^Trust this clone/ }),
    ).toBeInTheDocument();
    await waitFor(() => expect(finishRow()).not.toBeNull());
    expect(finishRow()).not.toHaveTextContent("Trust this clone");
    waits.unmount();
    forgetInbox();

    // None waits: on its chat's finish row with its control, and not in Asks.
    core({ asks: [], onFinishRows: [TRUST_CLONE], turns: [ENDED] });
    render(<Window />);
    await waitFor(() => expect(finishRow()).not.toBeNull());
    await waitFor(() =>
      expect(finishRow()).toHaveTextContent("Trust this clone so chats can use GitHub"),
    );
    expect(askRow()).toBeNull();
    const trust = within(finishRow() as HTMLElement).getByRole("button", {
      name: /^Trust this clone/,
    });
    await read();

    await userEvent.click(trust);

    await waitFor(() =>
      expect(asked.filter((one) => one.cmd === "vouch_clone").map((one) => one.args)).toEqual([
        { plane: PLANE, folder: "workspaces/alpha/api", place: "github.com/acme/api" },
      ]),
    );
  });

  it("a window-only ask whose chat has no finish row counted yet still has one", async () => {
    core({ asks: [], onFinishRows: [TRUST_CLONE], turns: [] });
    render(<Window />);

    await waitFor(() =>
      expect(finishRow()).toHaveTextContent("Trust this clone so chats can use GitHub"),
    );
    expect(
      within(finishRow() as HTMLElement).getByRole("button", { name: /^Trust this clone/ }),
    ).toBeInTheDocument();
    expect(askRow()).toBeNull();
  });

  it("holds Trust this clone on the finish row to the settle guard", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ["setTimeout", "clearTimeout"] });
    try {
      core({ asks: [], onFinishRows: [TRUST_CLONE], turns: [ENDED] });
      render(<Window />);
      await waitFor(() =>
        expect(finishRow()).toHaveTextContent("Trust this clone so chats can use GitHub"),
      );
      const press = userEvent.setup({ advanceTimers: (ms) => vi.advanceTimersByTime(ms) });

      await press.click(
        within(finishRow() as HTMLElement).getByRole("button", { name: /^Trust this clone/ }),
      );

      expect(asked.filter((one) => one.cmd === "vouch_clone")).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("says on the finish row why a trust pressed there was refused (Q274)", async () => {
    const why =
      "workspaces/alpha/api's remote is now github.com/acme/other, so purlis didn't trust it.";
    core({ asks: [], onFinishRows: [TRUST_CLONE], turns: [ENDED], refuses: why });
    render(<Window />);
    await waitFor(() =>
      expect(finishRow()).toHaveTextContent("Trust this clone so chats can use GitHub"),
    );
    const trust = within(finishRow() as HTMLElement).getByRole("button", {
      name: /^Trust this clone/,
    });
    await read();

    await userEvent.click(trust);

    await waitFor(() =>
      expect(within(finishRow() as HTMLElement).getByRole("status")).toHaveTextContent(why),
    );
  });

  it("names each of two asks on one finish row where it can be seen (Q274)", async () => {
    const other: Shown = {
      ...TRUST_CLONE,
      ask: "trust-clone:2:workspaces/alpha/web|github.com/acme/web",
      answer: { via: "trust-clone", folder: "workspaces/alpha/web", place: "github.com/acme/web" },
    };
    core({ asks: [], onFinishRows: [TRUST_CLONE, other], turns: [ENDED] });
    render(<Window />);
    await waitFor(() =>
      expect(
        within(finishRow() as HTMLElement).getAllByRole("button", { name: /^Trust this clone/ }),
      ).toHaveLength(2),
    );

    const seen = within(finishRow() as HTMLElement)
      .getAllByRole("button", { name: /^Trust this clone/ })
      .map((button) => button.textContent);

    expect(seen).toEqual([
      expect.stringContaining("workspaces/alpha/api"),
      expect.stringContaining("workspaces/alpha/web"),
    ]);
  });
});
