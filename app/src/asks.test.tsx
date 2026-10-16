// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { IN_ITS_CHAT, answerTaken, answerThrough, asksMoved, useAsks } from "./asks";
import type { Shown } from "./bindings";
import { REMOVE_WORKSPACE_ASK } from "./removeWorkspaceAsk";
import { PROFILE_SIGN_IN } from "./settings/links";

/**
 * The asks registry's window side (#1690): each ask is answered through the command its own
 * source's Notice answers with, and nothing else; the list is read again whenever a source may
 * have moved, so an ask leaves it when its source stops waiting.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";

/** Every command the window sent, with what it sent, answering each with `answers`. */
function sent(answers: Record<string, unknown> = {}) {
  const calls: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args: args as Record<string, unknown> });
    if (cmd in answers) {
      const answer = answers[cmd];
      if (answer instanceof Error) throw answer.message;
      return answer;
    }
    return null;
  });
  return calls;
}

const PERMISSION: Shown = {
  session: 3,
  ask: "01J0000000000000000000000A",
  says: "Run cargo test",
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain: ["steward 3"],
  answer: { via: "hook" },
};

const DISPATCH: Shown = {
  session: 4,
  ask: "dispatch:7",
  says: "Wants to hand a task to devops",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "you", label: "Allow for me on this machine", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
    { id: "never", label: "Never for this pair", allows: false },
  ],
  source: "dispatch",
  chain: ["steward 4"],
  answer: { via: "dispatch", id: 7, shown: "digest-1" },
};

const REMOVAL: Shown = {
  session: 6,
  ask: "removal:6:alpha",
  says: "Removing workspace alpha deletes its memory, its todos and its clones",
  options: [
    { id: "remove", label: "Remove…", allows: true },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: ["Safe remove · alpha"],
  answer: { via: "workspace-removal", workspace: "alpha", memories: 2, todos: 0 },
};

const HOST: Shown = {
  session: 5,
  ask: "block:5:connect:host:api.example.com",
  says: "The sandbox refused api.example.com",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "sandbox-host",
  chain: ["steward 5"],
  answer: {
    via: "sandbox-block",
    shown: { operation: "connect", kind: "host", what: "host", target: "api.example.com" },
  },
};

const SIGN_IN: Shown = {
  session: 0,
  ask: "sign-in:oc",
  says: "oc is not signed in on this Mac: a chat on it does not start until it is.",
  options: [
    { id: "sign-in:openai", label: "Sign in to ChatGPT", allows: true },
    { id: "dismiss", label: "Dismiss", allows: false },
  ],
  source: "sign-in",
  chain: ["Profile oc"],
  answer: { via: "sign-in", profile: "oc" },
};

const SANDBOX_ONLY: Shown = {
  session: 0,
  ask: "sandbox-only:codex",
  says: "Codex needs a sandbox-only profile here: a sandboxed Codex chat runs only on one.",
  options: [
    { id: "make", label: "Make one", allows: true },
    { id: "dismiss", label: "Dismiss", allows: false },
  ],
  source: "sandbox-only",
  chain: ["Codex"],
  answer: { via: "sandbox-only" },
};

const BROWSER: Shown = {
  session: 8,
  ask: "browser:8",
  says: "Wants a browser: what may it reach?",
  options: [
    { id: "project", label: "This project's sites", allows: true },
    { id: "open-web", label: "Also the open web", allows: true },
    { id: "project-always", label: "This project's sites, always for steward", allows: true },
    { id: "open-web-always", label: "Also the open web, always for steward", allows: true },
    { id: "keep", label: "Keep off", allows: false },
  ],
  source: "browser",
  chain: ["steward 8"],
  answer: { via: "browser" },
};

const QUESTION: Shown = {
  session: 6,
  ask: "question:6",
  says: "Waiting on your reply",
  options: [],
  source: "question",
  chain: ["steward 6"],
  answer: { via: "in-its-pane" },
};

describe("answering through the path an ask names", () => {
  it("answers a permission prompt on its chat's own hook", async () => {
    const calls = sent();
    expect(await answerThrough(PLANE, PERMISSION, "allow")).toBeUndefined();
    expect(calls).toEqual([
      {
        cmd: "answer_ask",
        args: { plane: PLANE, session: 3, ask: PERMISSION.ask, option: "allow" },
      },
    ]);
  });

  it("allows a dispatch at the level chosen with what its question showed, and nothing ticked", async () => {
    const calls = sent({ allow_dispatch: { said: "Allowed." } });
    expect(await answerThrough(PLANE, DISPATCH, "you")).toBeUndefined();
    expect(calls).toEqual([
      {
        cmd: "allow_dispatch",
        args: { plane: PLANE, id: 7, level: "you", also: [], shown: "digest-1" },
      },
    ]);
  });

  it("keeps a dispatch blocked, or says never for its pair, by the dispatch's own commands", async () => {
    const calls = sent({ keep_dispatch_blocked: true, never_dispatch: { said: "Never." } });
    await answerThrough(PLANE, DISPATCH, "keep");
    await answerThrough(PLANE, DISPATCH, "never");
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["keep_dispatch_blocked", { plane: PLANE, id: 7 }],
      ["never_dispatch", { plane: PLANE, id: 7 }],
    ]);
  });

  it("allows a refused host bound to the block shown, and keeps it blocked by the Notice's own command", async () => {
    const shown = { operation: "connect", kind: "host", what: "host", target: "api.example.com" };
    const calls = sent({ allow_sandbox_block: { said: "Allowed.", live: false } });
    await answerThrough(PLANE, HOST, "chat");
    await answerThrough(PLANE, HOST, "keep");
    await answerTaken(PLANE, HOST, "keep", "  use the mirror ");
    // Keep blocked is `keep_sandbox_block`, as on the block's Notice (#1666): it answers the
    // block and refuses what the chat's proxy holds on it, so the command waits no longer. The
    // reason the person typed goes with it, for the chat's one line (#1987).
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["allow_sandbox_block", { plane: PLANE, session: 5, shown, level: "chat" }],
      ["keep_sandbox_block", { plane: PLANE, session: 5, shown, reason: null }],
      ["keep_sandbox_block", { plane: PLANE, session: 5, shown, reason: "use the mirror" }],
    ]);
  });

  it("says whether the chat's proxy took a host's Allow live, so no restart is owed for it", async () => {
    sent({ allow_sandbox_block: { said: "Allowed.", live: true } });
    expect(await answerTaken(PLANE, HOST, "chat")).toEqual({ refused: undefined, live: true });
    sent({ allow_sandbox_block: { said: "Allowed.", live: false } });
    expect(await answerTaken(PLANE, HOST, "chat")).toEqual({ refused: undefined, live: false });
    sent({ answer_ask: null });
    expect(await answerTaken(PLANE, PERMISSION, "allow")).toEqual({
      refused: undefined,
      live: false,
    });
  });

  it("opens Delete workspace for a workspace a chat asked about, sends no removal, and keeps it by Keep (qw356)", async () => {
    // A removal is for good and a chat raised the question, so Remove… deletes nothing: the
    // window's own dialog is opened, and the person's press there is what removes.
    const calls = sent({ keep_asked_workspace: null });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(REMOVE_WORKSPACE_ASK, heard);
    try {
      expect(await answerTaken(PLANE, REMOVAL, "remove")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
      expect(opened).toEqual([{ plane: PLANE, workspace: "alpha" }]);
      expect(calls).toEqual([]);
      // Keep answers it, and opens nothing.
      expect(await answerTaken(PLANE, REMOVAL, "keep")).toEqual({
        refused: undefined,
        live: false,
      });
      expect(opened).toHaveLength(1);
    } finally {
      window.removeEventListener(REMOVE_WORKSPACE_ASK, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["keep_asked_workspace", { plane: PLANE, session: 6, workspace: "alpha" }],
    ]);
  });

  it("signs a profile in by opening its shell tab, sending nothing, and puts it away by Dismiss (#2035)", async () => {
    const calls = sent({ dismiss_sign_in: null });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(PROFILE_SIGN_IN, heard);
    try {
      expect(await answerTaken(PLANE, SIGN_IN, "sign-in:openai")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
      expect(opened).toEqual([{ plane: PLANE, profile: "oc", provider: "openai" }]);
      expect(calls).toEqual([]);
      expect(await answerTaken(PLANE, SIGN_IN, "dismiss")).toEqual({
        refused: undefined,
        live: false,
      });
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["dismiss_sign_in", { plane: PLANE, profile: "oc" }],
    ]);
  });

  it("makes a sandbox-only Codex profile, then opens its sign-in, and puts it away by Dismiss (#2037)", async () => {
    const calls = sent({ make_sandbox_only_profile: "codex-sandbox", dismiss_sandbox_only: null });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(PROFILE_SIGN_IN, heard);
    try {
      expect(await answerTaken(PLANE, SANDBOX_ONLY, "make")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
      expect(opened).toEqual([{ plane: PLANE, profile: "codex-sandbox", provider: undefined }]);
      expect(await answerTaken(PLANE, SANDBOX_ONLY, "dismiss")).toEqual({
        refused: undefined,
        live: false,
      });
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["make_sandbox_only_profile", { plane: PLANE }],
      ["dismiss_sandbox_only", { plane: PLANE }],
    ]);
  });

  it("sets what a chat's browser reaches by its own commands, for the chat or always for its persona, or keeps it off with the reason (#2013, #2241)", async () => {
    const calls = sent({ allow_chat_browser: null, keep_chat_browser_off: null });
    await answerThrough(PLANE, BROWSER, "project");
    await answerThrough(PLANE, BROWSER, "open-web");
    await answerThrough(PLANE, BROWSER, "project-always");
    await answerThrough(PLANE, BROWSER, "open-web-always");
    await answerTaken(PLANE, BROWSER, "keep", "  not for this task ");
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["allow_chat_browser", { plane: PLANE, session: 8, openWeb: false, always: false }],
      ["allow_chat_browser", { plane: PLANE, session: 8, openWeb: true, always: false }],
      ["allow_chat_browser", { plane: PLANE, session: 8, openWeb: false, always: true }],
      ["allow_chat_browser", { plane: PLANE, session: 8, openWeb: true, always: true }],
      ["keep_chat_browser_off", { plane: PLANE, session: 8, reason: "not for this task" }],
    ]);
  });

  it("says the source's own sentence when the source refuses", async () => {
    sent({ allow_dispatch: new Error("Nothing was allowed: the question changed.") });
    expect(await answerThrough(PLANE, DISPATCH, "chat")).toBe(
      "Nothing was allowed: the question changed.",
    );
  });

  it("sends nothing for an option the ask does not offer, or for an ask answered in its chat", async () => {
    const calls = sent();
    expect(await answerThrough(PLANE, HOST, "project")).toMatch(/not one of the answers/);
    expect(await answerThrough(PLANE, PERMISSION, "allow_always")).toMatch(/not one of/);
    expect(await answerThrough(PLANE, QUESTION, "yes")).toBe(IN_ITS_CHAT);
    expect(calls).toEqual([]);
  });
});

describe("the list", () => {
  it("is read again when a source may have moved, so an answered ask leaves it", async () => {
    let waiting: Shown[] = [PERMISSION, HOST];
    mockIPC(
      (cmd) => {
        if (cmd === "asks_waiting") return { plane: PLANE, asks: waiting };
        return null;
      },
      { shouldMockEvents: true },
    );
    const { result } = renderHook(() => useAsks([PLANE]));
    await waitFor(() => expect(result.current.held[PLANE]).toHaveLength(2));

    // Kept blocked in its Notice: the core no longer holds it, and a chat moving says so.
    waiting = [PERMISSION];
    await act(() => emit("chat-moved", { plane: PLANE, session: 5 }));
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([PERMISSION]));

    // Answered elsewhere: an answer reads it again whatever it answered.
    waiting = [];
    act(() => result.current.reread(PLANE));
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([]));
  });

  it("is read again when the window itself answered one where no event says so (#1692)", async () => {
    // A Notice's Keep blocked, or an answer in the Inbox, moves the core with no event of its
    // own: the window says so, and the list follows, so an answer in one place clears both.
    let waiting: Shown[] = [HOST];
    mockIPC((cmd) => (cmd === "asks_waiting" ? { plane: PLANE, asks: waiting } : null));
    const { result } = renderHook(() => useAsks([PLANE]));
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([HOST]));
    waiting = [];
    act(() => asksMoved(PLANE));
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([]));
  });

  it("is read again once a held connection's hold has run out, which no event says (#1709)", async () => {
    // The proxy gives up on its own clock and tells nobody: the ask names when its hold ends,
    // and the list is read again then, so the ask says it no longer waits.
    const until = Math.floor(Date.now() / 1000) + 1;
    const held: Shown = {
      ...HOST,
      says: "A connection to api.example.com waits on your answer",
      held_until: until,
    };
    let waiting: Shown[] = [held];
    let reads = 0;
    mockIPC((cmd) => {
      if (cmd !== "asks_waiting") return null;
      reads += 1;
      return { plane: PLANE, asks: waiting };
    });
    const { result } = renderHook(() => useAsks([PLANE]));
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([held]));
    const before = reads;
    waiting = [HOST];
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([HOST]), { timeout: 4000 });
    expect(reads).toBe(before + 1);
  });

  it("reads its list under React's StrictMode, which runs its effects twice", async () => {
    mockIPC((cmd) => (cmd === "asks_waiting" ? { plane: PLANE, asks: [HOST] } : null));
    const { result } = renderHook(() => useAsks([PLANE]), { wrapper: StrictMode });
    await waitFor(() => expect(result.current.held[PLANE]).toEqual([HOST]));
  });

  it("holds nothing for a project the core says nothing for", async () => {
    sent();
    const { result } = renderHook(() => useAsks([PLANE]));
    await new Promise((settle) => setTimeout(settle, 20));
    expect(PLANE in result.current.held).toBe(false);
  });
});
