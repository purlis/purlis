// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatMove, PoolSignInStep } from "./bindings";
import { chatMovesOf, forgetMove, heardMove } from "./chatMoves";
import { MoveNotice } from "./MoveNotice";

/**
 * **Move to runner waits for the chat's profile to sign in there** (#3477, spec #4023 story
 * 21-24, prototype Variant A): the move's notice says it waits while the chat keeps working here,
 * the same sign-in strip opens under it, and once the strip says signed in the core is told so
 * and the move goes ahead by itself. Cancel cancels the move and keeps the chat here.
 */

const PLANE = "/home/op/project";

const waiting = (kind = "claude"): ChatMove => ({
  session: 3,
  from: null,
  to: "builds",
  stage: "signing-in",
  why: null,
  with: [],
  sign_in: { profile: "work", kind, harness: kind === "claude" ? "Claude Code" : "Codex" },
});

let asked: { cmd: string; args: Record<string, unknown> }[];
let answer: ((step: PoolSignInStep) => void) | undefined;

beforeEach(() => {
  asked = [];
  answer = undefined;
  mockIPC((cmd, args) => {
    asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
    if (cmd === "pool_sign_in_start") return 7;
    if (cmd === "pool_sign_in_next")
      return new Promise<PoolSignInStep>((said) => {
        answer = said;
      });
    return null;
  });
});
afterEach(() => {
  cleanup();
  forgetMove(PLANE, 3);
  clearMocks();
});

const calls = (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);

async function notice(move: ChatMove) {
  act(() => heardMove(PLANE, move));
  render(<MoveNotice plane={PLANE} session={3} />);
  return screen.findByText(/once work is signed in there/);
}

describe("a move waiting on its profile's sign-in", () => {
  it("says it waits while the chat keeps working here, with the strip under it", async () => {
    const said = await notice(waiting());
    expect(said).toHaveTextContent(
      "Moves to pool builds once work is signed in there. It keeps working here meanwhile.",
    );
    expect(
      await screen.findByRole("group", { name: "Signing work in on builds" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "work", provider: null },
      ]),
    );
  });

  it("tells the core once the strip says signed in, so the move goes ahead by itself", async () => {
    await notice(waiting());
    await waitFor(() => expect(answer).toBeDefined());
    act(() => answer?.({ at: "signed-in" }));
    await waitFor(() =>
      expect(calls("chat_move_signed_in")).toEqual([{ plane: PLANE, session: 3 }]),
    );
    // The core prepares it again; nothing more is pressed.
    act(() => heardMove(PLANE, { ...waiting(), stage: "preparing", sign_in: null }));
    expect(await screen.findByText(/Moving to pool builds/)).toBeInTheDocument();
    expect(calls("put_away_chat_move")).toEqual([]);
  });

  it("Cancel in the strip cancels the move, and the chat stays here", async () => {
    await notice(waiting());
    const strip = await screen.findByRole("group", { name: "Signing work in on builds" });
    await userEvent.click(within(strip).getByRole("button", { name: "Cancel" }));
    await waitFor(() =>
      expect(calls("put_away_chat_move")).toEqual([{ plane: PLANE, session: 3 }]),
    );
    expect(chatMovesOf(PLANE)[3]).toBeUndefined();
    expect(calls("chat_move_signed_in")).toEqual([]);
  });

  it("Cancel move on the notice cancels it too", async () => {
    await notice(waiting());
    await userEvent.click(screen.getByRole("button", { name: "Cancel move" }));
    await waitFor(() =>
      expect(calls("put_away_chat_move")).toEqual([{ plane: PLANE, session: 3 }]),
    );
  });

  it("a harness whose sign-in the window does not drive offers Sign in and Try the move", async () => {
    await notice(waiting("codex"));
    expect(screen.queryByRole("group", { name: "Signing work in on builds" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    await waitFor(() =>
      expect(calls("chat_move_signed_in")).toEqual([{ plane: PLANE, session: 3 }]),
    );
    expect(screen.getByRole("button", { name: "Sign in" })).toBeInTheDocument();
  });
});
