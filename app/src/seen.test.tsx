// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useRef } from "react";
import type { InboxUpdate, Shown } from "./bindings";
import {
  SEEN_AFTER_MS,
  SeenRowsOf,
  askSeenKey,
  newAsks,
  newNotices,
  newUpdates,
  noticeSeenKey,
  updateSeenKey,
  useChatOnScreenSeen,
  useSeen,
  useSeenRow,
} from "./seen";

/**
 * **New** (#6002, ADR 0121 decisions 10 and 11): a row is New until it has been in sight in an
 * open view for about a second, or its chat is on screen in a focused window; the record is the
 * core's, mocked here as one that keeps what it is told.
 */

afterEach(() => {
  cleanup();
  clearMocks();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/plane";

const ask = (session: number, key: string) => ({ session, ask: key }) as Shown;
const update = (key: string, session: number | null, read = false) =>
  ({ key, session, read }) as InboxUpdate;

/** A core that keeps what it is told, starting from `kept`; answers what it was asked. */
function core(kept: string[] = []) {
  const record = new Set(kept);
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    const told = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: told });
    if (cmd === "inbox_seen") return [...record];
    if (cmd === "see_inbox") {
      const standing = told.asks as string[] | null;
      if (standing !== null)
        for (const key of [...record])
          if (key.startsWith("ask:") && !standing.includes(key)) record.delete(key);
      for (const key of told.keys as string[]) record.add(key);
      return [...record];
    }
    throw new Error(`not mocked: ${cmd}`);
  });
  return { record, asked: (cmd: string) => asked.filter((one) => one.cmd === cmd) };
}

describe("what is New", () => {
  it("nothing is New until the record is read, then what it has not seen is", async () => {
    core([askSeenKey(ask(3, "p1"))]);
    const asks = [ask(3, "p1"), ask(4, "p2")];
    const { result } = renderHook(() => useSeen(PLANE, asks));
    expect(result.current.rows.isNew(askSeenKey(ask(4, "p2")))).toBe(false);

    await waitFor(() => expect(result.current.seen).toBeDefined());

    expect(result.current.rows.isNew(askSeenKey(ask(3, "p1")))).toBe(false);
    expect(result.current.rows.isNew(askSeenKey(ask(4, "p2")))).toBe(true);
    expect(newAsks(asks, result.current.seen)).toBe(1);
  });

  it("an update marked read is not New, though no row of it was seen", () => {
    const seen = new Set<string>();
    expect(newUpdates([update("a", 3), update("b", 3, true)], seen)).toBe(1);
    expect(newUpdates([update("a", 3)], undefined)).toBe(0);
  });

  it("a Notice is New by its cause", () => {
    expect(
      newNotices(["pin-gone:ide", "doctor-finding:x"], new Set([noticeSeenKey("pin-gone:ide")])),
    ).toBe(1);
  });
});

/** A row of a view, under the project's seen record. */
function Row({ seenKey }: { seenKey: string }) {
  const drawn = useRef<HTMLLIElement>(null);
  const [isNew] = useSeenRow(seenKey, drawn);
  return (
    <li ref={drawn} data-new={isNew || undefined}>
      {seenKey}
    </li>
  );
}

function View({ keys }: { keys: string[] }) {
  const { rows } = useSeen(PLANE, undefined);
  return (
    <SeenRowsOf.Provider value={rows}>
      <ul>
        {keys.map((key) => (
          <Row key={key} seenKey={key} />
        ))}
      </ul>
    </SeenRowsOf.Provider>
  );
}

describe("a row in sight in an open view", () => {
  it("is seen after about a second in sight, and kept", async () => {
    const { asked, record } = core();
    const key = updateSeenKey("task:a");
    render(<View keys={[key]} />);
    // The window's test setup tells every watched element it is in sight at once.
    await waitFor(() => expect(screen.getByText(key)).toHaveAttribute("data-new"));
    expect(asked("see_inbox")).toEqual([]);

    await waitFor(() => expect(screen.getByText(key)).not.toHaveAttribute("data-new"), {
      timeout: SEEN_AFTER_MS * 3,
    });

    expect(asked("see_inbox").map((one) => one.args)).toEqual([
      { plane: PLANE, keys: [key], asks: null },
    ]);
    expect(record.has(key)).toBe(true);
  });

  it("is not seen while it has been in sight for less than a second", async () => {
    const { asked } = core();
    const key = updateSeenKey("task:a");
    render(<View keys={[key]} />);
    await waitFor(() => expect(screen.getByText(key)).toHaveAttribute("data-new"));

    vi.useFakeTimers();
    act(() => vi.advanceTimersByTime(SEEN_AFTER_MS - 100));

    expect(asked("see_inbox")).toEqual([]);
    expect(screen.getByText(key)).toHaveAttribute("data-new");
  });

  it("is not seen while the window is hidden", async () => {
    const { asked } = core();
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    const key = updateSeenKey("task:a");
    render(<View keys={[key]} />);
    await waitFor(() => expect(screen.getByText(key)).toHaveAttribute("data-new"));

    vi.useFakeTimers();
    act(() => vi.advanceTimersByTime(SEEN_AFTER_MS * 2));

    expect(asked("see_inbox")).toEqual([]);
  });
});

describe("the chat on screen in a focused window", () => {
  function OnScreen({
    session,
    asks,
    updates,
  }: {
    session: number;
    asks: Shown[];
    updates?: InboxUpdate[];
  }) {
    const { see, rows } = useSeen(PLANE, asks);
    useChatOnScreenSeen(see, session, true, asks, updates);
    return (
      <ul>
        {asks.map((one) => (
          <li key={one.ask} data-new={rows.isNew(askSeenKey(one)) || undefined}>
            {one.ask}
          </li>
        ))}
      </ul>
    );
  }

  it("its asks are never New, and are kept seen; another chat's are New", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const { asked } = core();
    render(<OnScreen session={3} asks={[ask(3, "p1"), ask(4, "p2")]} updates={[update("t", 3)]} />);

    await waitFor(() => expect(asked("see_inbox")).toHaveLength(1));
    expect(asked("see_inbox")[0].args).toEqual({
      plane: PLANE,
      keys: [askSeenKey(ask(3, "p1")), updateSeenKey("t")],
      asks: null,
    });
    expect(screen.getByText("p1")).not.toHaveAttribute("data-new");
    expect(screen.getByText("p2")).toHaveAttribute("data-new");
  });

  it("in a window without the keyboard, its asks are New", async () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { asked } = core();
    render(<OnScreen session={3} asks={[ask(3, "p1")]} />);

    await waitFor(() => expect(screen.getByText("p1")).toHaveAttribute("data-new"));
    expect(asked("see_inbox")).toEqual([]);
  });
});

describe("an ask that stops standing", () => {
  it("is forgotten, so the same ask asked again is New", async () => {
    const one = ask(3, "wait");
    const { asked, record } = core([askSeenKey(one), noticeSeenKey("pin-gone:ide")]);
    const { result, rerender } = renderHook(({ asks }) => useSeen(PLANE, asks), {
      initialProps: { asks: [one] },
    });
    await waitFor(() => expect(result.current.seen).toBeDefined());
    expect(asked("see_inbox")).toEqual([]);

    rerender({ asks: [] });
    await waitFor(() => expect(asked("see_inbox")).toHaveLength(1));
    expect(asked("see_inbox")[0].args).toEqual({ plane: PLANE, keys: [], asks: [] });
    expect([...record]).toEqual([noticeSeenKey("pin-gone:ide")]);

    rerender({ asks: [one] });
    await waitFor(() => expect(result.current.rows.isNew(askSeenKey(one))).toBe(true));
  });
});
