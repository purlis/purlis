// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatMove, PoolSignInStep } from "./bindings";
import { chatMovesOf, forgetMove, heardMove } from "./chatMoves";
import { MoveNotice } from "./MoveNotice";
import { PROFILE_SIGN_IN, type ProfileSignInAsk } from "./settings/links";

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
  sign_in: {
    profile: "work",
    kind,
    harness: kind === "claude" ? "Claude Code" : "Codex",
    plans: [],
  },
});

/** A move waiting on an opencode profile, whose harness serves several plans. */
const opencode = (): ChatMove => ({
  ...waiting(),
  sign_in: {
    profile: "work",
    kind: "opencode",
    harness: "opencode",
    plans: [
      { id: "openai", title: "ChatGPT" },
      { id: "github-copilot", title: "GitHub Copilot" },
    ],
  },
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

/** The notice, with its Sign in pressed: the strip under it. */
async function strip(move: ChatMove = waiting()) {
  await notice(move);
  await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
  return screen.findByRole("group", { name: "Signing work in on builds" });
}

describe("a move waiting on its profile's sign-in", () => {
  it("says it waits while the chat keeps working here, and Sign in opens the strip under it", async () => {
    const said = await notice(waiting());
    expect(said).toHaveTextContent(
      "Moves to pool builds once work is signed in there. It keeps working here meanwhile.",
    );
    // Nothing starts on the runner until Sign in is pressed (prototype Variant A).
    expect(screen.queryByRole("group", { name: "Signing work in on builds" })).toBeNull();
    expect(calls("pool_sign_in_start")).toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(
      await screen.findByRole("group", { name: "Signing work in on builds" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "work", provider: null, terminal: null },
      ]),
    );
  });

  it("tells the core once the strip says signed in, so the move goes ahead by itself", async () => {
    await strip();
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
    const opened = await strip();
    await userEvent.click(within(opened).getByRole("button", { name: "Cancel" }));
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

  it("a harness whose sign-in the window does not drive signs in in a terminal, then Try the move", async () => {
    await notice(waiting("gemini"));
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(screen.queryByRole("group", { name: "Signing work in on builds" })).toBeNull();
    expect(calls("pool_sign_in_start")).toEqual([]);
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    await waitFor(() =>
      expect(calls("chat_move_signed_in")).toEqual([{ plane: PLANE, session: 3 }]),
    );
  });

  it("asks a harness that serves several plans which one, and signs that one in (#4181)", async () => {
    await notice(opencode());
    expect(screen.queryByRole("button", { name: "Sign in" })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Sign in to ChatGPT" }));

    await screen.findByRole("group", { name: "Signing work in on builds" });
    await waitFor(() =>
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "work", provider: "openai", terminal: null },
      ]),
    );
  });

  it("a plan's Open in terminal runs its device code, which has a way back (#4181)", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      await notice(opencode());
      await userEvent.click(screen.getByRole("button", { name: "Sign in to ChatGPT" }));
      const opened = await screen.findByRole("group", { name: "Signing work in on builds" });

      await userEvent.click(within(opened).getByRole("button", { name: "Open in terminal" }));

      expect(heard).toEqual([
        { plane: PLANE, profile: "work", provider: "openai", pool: "builds", device: true },
      ]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("a refused Try the move reads the core's moves back, never inventing a failure", async () => {
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "chat_move_signed_in") throw "Its move waits on no sign-in.";
      if (cmd === "chat_moves") return [{ ...waiting(), stage: "ready", sign_in: null }];
      return null;
    });
    await notice(waiting());
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    expect(await screen.findByText("Moves to pool builds at its next pause.")).toBeInTheDocument();
    expect(chatMovesOf(PLANE)[3]?.stage).toBe("ready");
  });
});
