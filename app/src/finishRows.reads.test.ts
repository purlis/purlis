// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { commands, type FinishedTurn, type PlaneId } from "./bindings";
import { type ChatStates, type Chats, nothingKnown } from "./chatState";
import { useFinishedTurns } from "./finishRows";

const PLANE = "/plane" as PlaneId;

type Answer = Awaited<ReturnType<typeof commands.finishedTurns>>;

const turn = (blocked: number): FinishedTurn => ({
  session: 3,
  key: `turn-${blocked}`,
  at: 1_700_000_000,
  failed: false,
  blocked,
  blocks: [],
});

/** A store of one chat, whose moves the test makes. */
function chatsOf(first: ChatStates) {
  let states = first;
  const listeners = new Set<() => void>();
  const chats: Chats = {
    plane: PLANE,
    store: {
      subscribe: (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
      statesFor: () => states,
    },
  };
  const move = (next: ChatStates) => {
    states = next;
    act(() => listeners.forEach((listener) => listener()));
  };
  return { chats, move };
}

const running = (movedAt: number): ChatStates => ({
  ...nothingKnown,
  bySession: { 3: "running" },
  movedAt: { 3: movedAt },
});

const ended = (movedAt: number): ChatStates => ({
  ...nothingKnown,
  bySession: { 3: "done" },
  movedAt: { 3: movedAt },
});

/** A read that answers when the test says. */
function deferred() {
  let answer: (value: Answer) => void = () => undefined;
  const promise = new Promise<Answer>((resolve) => {
    answer = resolve;
  });
  return { promise, answer };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the finish rows' reads (Q263)", () => {
  it("keeps the newest answer when an older read answers after it", async () => {
    const older = deferred();
    const newer = deferred();
    const read = vi
      .spyOn(commands, "finishedTurns")
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);
    const { chats, move } = chatsOf(ended(1));
    const { result } = renderHook(() => useFinishedTurns(PLANE, chats));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    move(ended(2));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
    await act(async () => {
      newer.answer({ status: "ok", data: [turn(1)] });
      await newer.promise;
    });
    await waitFor(() => expect(result.current).toEqual([turn(1)]));
    await act(async () => {
      older.answer({ status: "ok", data: [turn(0)] });
      await older.promise;
    });
    expect(result.current).toEqual([turn(1)]);
  });

  it("asks again when a turn next ends after a read failed", async () => {
    const read = vi
      .spyOn(commands, "finishedTurns")
      .mockResolvedValueOnce({ status: "error", error: "the log is busy" })
      .mockResolvedValueOnce({ status: "ok", data: [turn(1)] });
    const { chats, move } = chatsOf(ended(1));
    const { result } = renderHook(() => useFinishedTurns(PLANE, chats));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    await act(async () => {
      await Promise.resolve();
    });
    move(ended(2));
    await waitFor(() => expect(result.current).toEqual([turn(1)]));
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("asks again when a turn next ends after a read threw", async () => {
    const read = vi
      .spyOn(commands, "finishedTurns")
      .mockRejectedValueOnce(new Error("gone"))
      .mockResolvedValueOnce({ status: "ok", data: [turn(1)] });
    const { chats, move } = chatsOf(ended(1));
    const { result } = renderHook(() => useFinishedTurns(PLANE, chats));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    await act(async () => {
      await Promise.resolve();
    });
    move(ended(2));
    await waitFor(() => expect(result.current).toEqual([turn(1)]));
  });

  it("asks nothing when a chat moves within its turn, nor again at the end it is already at", async () => {
    const read = vi
      .spyOn(commands, "finishedTurns")
      .mockResolvedValue({ status: "error", error: "the log is busy" });
    const { chats, move } = chatsOf(running(1));
    renderHook(() => useFinishedTurns(PLANE, chats));
    for (let at = 2; at < 10; at++) move(running(at));
    move(ended(10));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
    move(ended(10));
    await act(async () => {
      await new Promise((done) => setTimeout(done, 20));
    });
    expect(read).toHaveBeenCalledTimes(1);
  });
});
