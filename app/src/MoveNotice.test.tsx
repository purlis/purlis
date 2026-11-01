// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, renderHook, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ChatMove, PoolSignInStep } from "./bindings";
import {
  chatMovesOf,
  dismissMovedAfterSignIn,
  forgetMove,
  heardMove,
  movedAfterSignIn,
  putAwayMove,
  useMovedAfterSignIn,
} from "./chatMoves";
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
  dismissMovedAfterSignIn(PLANE, 8);
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

  it("keeps a wait on a sign-in only while the move stays on that pool (#4325)", () => {
    const moved = renderHook(() => useMovedAfterSignIn(PLANE, 9));
    const after = (to: string) => {
      act(() => heardMove(PLANE, waiting()));
      act(() => heardMove(PLANE, { ...waiting(), stage: "preparing", sign_in: null, to }));
      act(() => movedAfterSignIn(PLANE, 3, 9));
      return moved.result.current;
    };
    expect(after("other")).toBeUndefined();
    expect(after("builds")).toBe("builds");
    act(() => dismissMovedAfterSignIn(PLANE, 9));
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
      // The strip's login ended: only the shell tab's runs.
      await waitFor(() => expect(calls("pool_sign_in_cancel")).toEqual([{ flow: 7 }]));
      expect(calls("put_away_chat_move")).toEqual([]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("a plan pressed by mistake can be left, and the move stays (#4181)", async () => {
    await notice(opencode());
    await userEvent.click(screen.getByRole("button", { name: "Sign in to ChatGPT" }));
    await screen.findByRole("group", { name: "Signing work in on builds" });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(1));

    await userEvent.click(screen.getByRole("button", { name: "Choose another plan" }));

    expect(await screen.findByRole("button", { name: "Sign in to GitHub Copilot" })).toBeVisible();
    await waitFor(() => expect(calls("pool_sign_in_cancel")).toEqual([{ flow: 7 }]));
    expect(calls("put_away_chat_move")).toEqual([]);
  });

  it("keeps the keyboard in the notice after Choose another plan, on the first plan (#4278)", async () => {
    await notice(opencode());
    const first = screen.getByRole("button", { name: "Sign in to ChatGPT" });
    expect(first, "a notice that appears takes no keyboard").not.toHaveFocus();
    await userEvent.click(first);
    await screen.findByRole("group", { name: "Signing work in on builds" });

    await userEvent.click(screen.getByRole("button", { name: "Choose another plan" }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Sign in to ChatGPT" })).toHaveFocus(),
    );
  });

  it("does not take the keyboard for a strip closed another way after a plan was left (#4278)", async () => {
    await notice(opencode());
    await userEvent.click(screen.getByRole("button", { name: "Sign in to ChatGPT" }));
    await userEvent.click(await screen.findByRole("button", { name: "Choose another plan" }));
    await userEvent.click(await screen.findByRole("button", { name: "Sign in to ChatGPT" }));
    const strip = await screen.findByRole("group", { name: "Signing work in on builds" });

    await userEvent.click(within(strip).getByRole("button", { name: "Open in terminal" }));

    expect(await screen.findByRole("button", { name: "Sign in to ChatGPT" })).not.toHaveFocus();
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

  it("forgets a move the core no longer holds once it is read back, so its notice goes (#4138)", async () => {
    mockIPC((cmd) => {
      if (cmd === "chat_move_signed_in") throw "Chat 3 has no move.";
      if (cmd === "chat_moves") return [];
      return null;
    });
    await notice(waiting());
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    await waitFor(() => expect(chatMovesOf(PLANE)[3]).toBeUndefined());
    expect(screen.queryByText(/once work is signed in there/)).toBeNull();
  });

  it("keeps a move when the read back fails, and a failed move through a read back (#4138)", async () => {
    let reads = 0;
    mockIPC((cmd) => {
      if (cmd === "chat_move_signed_in") throw "Chat 3 has no move.";
      if (cmd === "chat_moves") {
        reads += 1;
        if (reads === 1) throw "no answer";
        return [];
      }
      return null;
    });
    const failed: ChatMove = { ...waiting(), session: 4, stage: "failed", why: "refused" };
    act(() => heardMove(PLANE, failed));
    await notice(waiting());
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    await waitFor(() => expect(reads).toBe(1));
    expect(chatMovesOf(PLANE)[3]?.stage).toBe("signing-in");
    await userEvent.click(screen.getByRole("button", { name: "Try the move" }));
    await waitFor(() => expect(chatMovesOf(PLANE)[3]).toBeUndefined());
    expect(chatMovesOf(PLANE)[4]?.stage).toBe("failed");
    forgetMove(PLANE, 4);
  });

  it("says the chat moved once a move that waited on a sign-in was made, until dismissed (#4135)", async () => {
    act(() => {
      heardMove(PLANE, waiting());
      heardMove(PLANE, { ...waiting(), stage: "switching", sign_in: null });
      // The core's `over` can come before the answer to the switch.
      heardMove(PLANE, { ...waiting(), stage: "over", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(await screen.findByText("Moved to pool builds.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(screen.queryByText("Moved to pool builds.")).toBeNull();
  });

  it("still says it moved after its moving line was dismissed (#4135)", async () => {
    act(() => {
      heardMove(PLANE, waiting());
      heardMove(PLANE, { ...waiting(), stage: "switching", sign_in: null });
      forgetMove(PLANE, 3);
      heardMove(PLANE, { ...waiting(), stage: "over", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(await screen.findByText("Moved to pool builds.")).toBeInTheDocument();
  });

  it("still says it moved where Cancel came too late (#4135)", async () => {
    mockIPC((cmd) => {
      if (cmd === "put_away_chat_move") throw "Its move is being made.";
      if (cmd === "chat_moves") return [{ ...waiting(), stage: "switching", sign_in: null }];
      return null;
    });
    act(() => heardMove(PLANE, waiting()));
    act(() => putAwayMove(PLANE, 3));
    await waitFor(() => expect(chatMovesOf(PLANE)[3]?.stage).toBe("switching"));
    act(() => {
      heardMove(PLANE, { ...waiting(), stage: "over", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(await screen.findByText("Moved to pool builds.")).toBeInTheDocument();
  });

  it("says nothing of an earlier wait when a later move never waited (#4135)", () => {
    act(() => {
      heardMove(PLANE, waiting());
      forgetMove(PLANE, 3);
      heardMove(PLANE, { ...waiting(), to: null, stage: "ready", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(screen.queryByText(/^Moved to/)).toBeNull();
  });

  it("says nothing of a move that was put away, here or from another tab (#4135)", async () => {
    await act(async () => {
      heardMove(PLANE, waiting());
      putAwayMove(PLANE, 3);
      await Promise.resolve();
    });
    await waitFor(() => expect(calls("put_away_chat_move")).toHaveLength(1));
    await act(async () => {
      await new Promise((done) => setTimeout(done, 0));
    });
    act(() => {
      movedAfterSignIn(PLANE, 3, 8);
      heardMove(PLANE, waiting());
      heardMove(PLANE, { ...waiting(), stage: "over", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(screen.queryByText(/^Moved to/)).toBeNull();
  });

  it("says nothing of a move that never waited on a sign-in (#4135)", () => {
    act(() => {
      heardMove(PLANE, { ...waiting(), stage: "ready", sign_in: null });
      movedAfterSignIn(PLANE, 3, 8);
    });
    render(<MoveNotice plane={PLANE} session={8} />);
    expect(screen.queryByText(/^Moved to/)).toBeNull();
  });
});

describe("a move refused by a pool's limit (#4313)", () => {
  const WHY = "it needs 150 MiB, over this pool's limit of 128 MiB";
  const refusedMove: ChatMove = {
    session: 3,
    from: null,
    to: "builds",
    stage: "failed",
    why: WHY,
    with: [],
    sign_in: null,
  };
  const OFFER = {
    pool: "builds",
    hits: [
      {
        key: "profileSize",
        plain: "Most a profile may send: size",
        unit: "MiB",
        in_force: 128,
        needed: 150,
        suggested: 192,
      },
    ],
  };
  beforeEach(() => {
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "pool_limit_offer_for_move") return OFFER;
      if (cmd === "pool_limits") return { groups: [], advanced: [] };
      return null;
    });
    act(() => heardMove(PLANE, refusedMove));
  });

  it("offers the suggestion, saves it to the pool and asks for the move again", async () => {
    const user = userEvent.setup();
    render(<MoveNotice plane={PLANE} session={3} />);
    const box = await screen.findByRole("textbox", { name: /Most a profile may send: size/ });
    expect((box as HTMLInputElement).value).toBe("192");
    await user.click(screen.getByRole("button", { name: "Save to the pool and move" }));
    await waitFor(() => expect(asked.some((one) => one.cmd === "move_chat")).toBe(true));
    const names = asked.map((one) => one.cmd);
    expect(names.indexOf("pool_limit_set")).toBeLessThan(names.indexOf("move_chat"));
    expect(asked.find((one) => one.cmd === "pool_limit_set")?.args).toEqual({
      pool: "builds",
      key: "profileSize",
      value: 192,
    });
    expect(asked.find((one) => one.cmd === "move_chat")?.args).toMatchObject({
      session: 3,
      to: "builds",
    });
  });

  it("keeps the form and doesn't move where the save was refused", async () => {
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "pool_limit_offer_for_move") return OFFER;
      if (cmd === "pool_limit_set") throw "there is no pool named builds on this machine";
      if (cmd === "pool_limits") return { groups: [], advanced: [] };
      return null;
    });
    const user = userEvent.setup();
    render(<MoveNotice plane={PLANE} session={3} />);
    await user.click(await screen.findByRole("button", { name: "Save to the pool and move" }));
    expect(await screen.findByText(/there is no pool named builds/)).toBeTruthy();
    expect(asked.some((one) => one.cmd === "move_chat")).toBe(false);
    expect(screen.getByRole("button", { name: "Save to the pool and move" })).toBeTruthy();
  });

  it("doesn't move when dismissed while the limit saves", async () => {
    let saved: (() => void) | undefined;
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "pool_limit_offer_for_move") return OFFER;
      if (cmd === "pool_limit_set")
        return new Promise<null>((done) => {
          saved = () => done(null);
        });
      if (cmd === "pool_limits") return { groups: [], advanced: [] };
      return null;
    });
    const user = userEvent.setup();
    render(<MoveNotice plane={PLANE} session={3} />);
    await user.click(await screen.findByRole("button", { name: "Save to the pool and move" }));
    await waitFor(() => expect(saved).toBeDefined());
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    saved?.();
    await new Promise((done) => setTimeout(done, 20));
    expect(asked.some((one) => one.cmd === "move_chat")).toBe(false);
  });

  it("offers nothing on any other refusal, and on a move here", async () => {
    mockIPC((cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      return null;
    });
    render(<MoveNotice plane={PLANE} session={3} />);
    await waitFor(() =>
      expect(asked.some((one) => one.cmd === "pool_limit_offer_for_move")).toBe(true),
    );
    expect(screen.queryByRole("textbox")).toBeNull();
    cleanup();
    asked.length = 0;
    act(() => heardMove(PLANE, { ...refusedMove, to: null, from: "builds" }));
    render(<MoveNotice plane={PLANE} session={3} />);
    expect(asked.some((one) => one.cmd === "pool_limit_offer_for_move")).toBe(false);
  });
});
