// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatLog, ChatLogRow, GrantLevel, SandboxGrant } from "./bindings";
import { ChatLogTab } from "./ChatLogTab";
import { AllowedRows } from "./settings/GrantedList";
import { ALLOW_NEXT_TIME } from "./blockWays";
import { SANDBOX_ALLOW_SAID } from "./allowLabels";
import { SETTLE_MS } from "./noticeSettle";

/**
 * **Allow next time on a sandbox block** (T1-34; R-291, Q188), against a grant store: the core's
 * seam is stood in for by one that keeps what the window allowed, lists it as the Allowed list
 * reads it, and judges the chat's next attempt at a target by it. Allow next time · once lets
 * that attempt through with no new ask and is then gone from the list; a wider label stays
 * until it is removed. The real store is `purlis_core::sandbox::asks`, tested at its seam.
 */

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date", "performance"] });
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;
const TARGET = "registry.example.com:443";

/** The person reads what is drawn for longer than an Allow waits. */
const read = () => act(() => vi.advanceTimersByTime(SETTLE_MS + 1));

const BLOCK: ChatLogRow = {
  kind: "blocked",
  at: "2026-11-14T09:10:00Z",
  tool: "Bash",
  source: "sandbox",
  qualifier: null,
  target_kind: "host",
  target: TARGET,
  harness: "claude",
  reason: null,
  resolved: false,
  // What the core says this row may offer (Q278): every label, under no policy.
  allow_levels: ["once", "you", "chat", "project"],
};

const LOG: ChatLog = {
  known: true,
  rows: [BLOCK],
  summary: {
    allowed: 0,
    by_your_rules: 0,
    unverified: false,
    ran_as_you: false,
    harness: "claude",
    cant_tell_its_checks_blocks: false,
    cant_tell_its_mode: false,
  },
};

/** One grant the store keeps: for which chat, of what, at which level. */
type Kept = { session: number; target: string; level: GrantLevel };

/** **A grant's id, as the core's Allowed list names it** (`sandboxing::grants_of`): a waiting
 *  once is `next␟<chat>␟<host>`, one chat's `chat␟<its id>␟host␟<host>`, a wider one
 *  `<level>␟host␟<host>`. */
const idOf = (one: Kept) =>
  one.level === "once"
    ? `next\u001f${one.session}\u001f${one.target}`
    : one.level === "chat"
      ? `chat\u001fchat-${one.session}\u001fhost\u001f${one.target}`
      : `${one.level}\u001fhost\u001f${one.target}`;

/** **The grant store** the window allows into: what `allow_next_time` keeps, and what the
 *  chat's attempts are judged by. */
function grantStore() {
  const kept: Kept[] = [];
  const asked: string[] = [];
  const revoked: string[] = [];
  const listed = () =>
    kept.map((one): SandboxGrant => ({
      id: idOf(one),
      what: "host",
      target: one.target,
      persona: null,
      level: one.level,
      by: null,
      at: null,
      chat: "steward 3",
      locked: null,
      waiting: null,
      for_no_persona: false,
      expected: false,
    }));
  mockIPC((cmd, args) => {
    const a = args as Record<string, unknown>;
    if (cmd === "opened_chats") return [];
    if (cmd === "chat_log") return LOG;
    if (cmd === "chat_blocks") return [];
    if (cmd === "allow_next_time") {
      const one = { session: a.session as number, target: a.target as string, level: a.level };
      kept.push(one as Kept);
      return { said: `Allowed ${one.target}.`, live: true };
    }
    if (cmd === "sandbox_grants") return listed();
    if (cmd === "revoke_sandbox_grant") {
      const id = a.id as string;
      const at = kept.findIndex((one) => idOf(one) === id);
      if (at === -1) throw new Error("purlis did not remove it: it is no longer there.");
      revoked.push(id);
      kept.splice(at, 1);
      return listed();
    }
    return null;
  });
  return {
    kept,
    asked,
    revoked,
    /** **The chat's next attempt at `target`**: through on a grant, which a once uses up;
     *  else held and asked about. */
    attempt(session: number, target: string): "through" | "asked" {
      const at = kept.findIndex((one) => one.session === session && one.target === target);
      if (at === -1) {
        asked.push(target);
        return "asked";
      }
      if (kept[at].level === "once") kept.splice(at, 1);
      return "through";
    },
  };
}

/** Presses Allow next time on the block's row, then `label` once the row has settled. */
async function allowNextTime(label: string) {
  const row = await screen.findByRole("group", { name: "Ways out for steward 3" });
  await userEvent.click(within(row).getByRole("button", { name: ALLOW_NEXT_TIME }));
  read();
  await userEvent.click(screen.getByRole("button", { name: label }));
  await screen.findByText(`Allowed ${TARGET}.`);
}

/** The Allowed list, as Settings › Sandbox › Network reads it now. */
async function allowedList(): Promise<string[]> {
  const drawn = render(
    <AllowedRows plane={PLANE} file="purlis.toml" ids={{ id: "allowed", labelledBy: "a" }} />,
  );
  await waitFor(() => expect(drawn.queryByText("Reading what is allowed…")).toBeNull());
  const list = drawn.queryByRole("list", { name: "Allowed" });
  const rows =
    list === null
      ? []
      : within(list)
          .queryAllByRole("listitem")
          .map((one) => one.textContent);
  drawn.unmount();
  return rows.map((one) => one ?? "");
}

describe("Allow next time on a sandbox block (T1-34, R-291)", () => {
  it("Allow next time once lets the next attempt through with no new ask and is then gone", async () => {
    const store = grantStore();
    render(<ChatLogTab plane={PLANE} session={SESSION} name="steward 3" />);
    await allowNextTime(SANDBOX_ALLOW_SAID.once);
    expect(store.kept).toEqual([{ session: SESSION, target: TARGET, level: "once" }]);
    // Listed until it is used.
    expect(await allowedList()).toEqual([expect.stringContaining("registry.example.com")]);

    // The chat's next attempt goes through with no new ask, and the entry is gone.
    expect(store.attempt(SESSION, TARGET)).toBe("through");
    expect(store.asked).toEqual([]);
    expect(await allowedList()).toEqual([]);
    // The attempt after it is asked about again.
    expect(store.attempt(SESSION, TARGET)).toBe("asked");
  });

  it("Remove takes a waiting once away by its id before it is used, so the next attempt is asked about", async () => {
    const store = grantStore();
    render(<ChatLogTab plane={PLANE} session={SESSION} name="steward 3" />);
    await allowNextTime(SANDBOX_ALLOW_SAID.once);
    const drawn = render(
      <AllowedRows plane={PLANE} file="purlis.toml" ids={{ id: "allowed", labelledBy: "a" }} />,
    );
    await waitFor(() => expect(drawn.queryByText("Reading what is allowed…")).toBeNull());
    const list = drawn.getByRole("list", { name: "Allowed" });
    await userEvent.click(within(list).getByRole("button", { name: /^Remove/ }));
    await waitFor(() => expect(store.revoked).toEqual([`next\u001f${SESSION}\u001f${TARGET}`]));
    drawn.unmount();
    expect(await allowedList()).toEqual([]);
    expect(store.attempt(SESSION, TARGET)).toBe("asked");
  });

  it("with a wider label the entry stays until it is removed", async () => {
    const store = grantStore();
    render(<ChatLogTab plane={PLANE} session={SESSION} name="steward 3" />);
    await allowNextTime(SANDBOX_ALLOW_SAID.you);
    expect(store.kept).toEqual([{ session: SESSION, target: TARGET, level: "you" }]);
    for (let n = 0; n < 3; n += 1) expect(store.attempt(SESSION, TARGET)).toBe("through");
    expect(store.asked).toEqual([]);
    expect(await allowedList()).toEqual([expect.stringContaining("registry.example.com")]);
  });
});
