// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { IN_ITS_CHAT, answerTaken, answerThrough, asksMoved, useAsks } from "./asks";
import type { Shown } from "./bindings";
import { OPEN_BROWSER_ASK, type OpenBrowserAsk } from "./browserNeedsYou";
import { OPEN_PROPOSAL_ASK } from "./proposedChangeOpen";
import { REMOVE_WORKSPACE_ASK } from "./removeWorkspaceAsk";
import { OPEN_RUNNER_WRITE_ASK } from "./runnerWriteOpen";
import { OPEN_RUNNER_DISPATCH_ASK } from "./runnerDispatchOpen";
import { PROFILE_SIGN_IN } from "./settings/links";
import { useSandboxCommands } from "./sandboxAsked";

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

/** A change a chat on a pool's runner asked for (#2046). */
const RUNNER_WRITE: Shown = {
  session: 6,
  ask: "runner-write:3",
  says: "Asks, from pool builds, to change this project: Remember in this workspace's memory: x",
  options: [
    { id: "apply", label: "Apply", allows: true },
    { id: "dismiss", label: "Dismiss", allows: false },
  ],
  source: "runner-write",
  chain: [],
  answer: { via: "runner-write", id: 3 },
};

/** A task a chat on a pool's runner asked to start on this machine. */
const RUNNER_DISPATCH: Shown = {
  session: 6,
  ask: "runner-dispatch:2",
  says: "Asks, from pool builds, to start the task 'check prod' as devops on this machine: look",
  options: [
    { id: "open", label: "Open", allows: false },
    { id: "refuse", label: "Refuse", allows: false },
  ],
  source: "runner-dispatch",
  chain: [],
  answer: { via: "runner-dispatch", id: 2 },
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
  answer: { via: "sign-in", profile: "oc", pool: null },
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

const OLD_PLUGINS: Shown = {
  session: 0,
  ask: "old-harness-plugins",
  says: "purlis no longer turns other harnesses' plugins on or off per project.",
  options: [
    { id: "remove", label: "Remove", allows: false },
    { id: "dismiss", label: "Dismiss", allows: false },
  ],
  source: "old-harness-plugins",
  chain: ["purlis"],
  answer: { via: "old-harness-plugins" },
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

const NEEDS_YOU: Shown = {
  session: 8,
  ask: "browser-needs-you:P1",
  says: "Needs you in its browser b1. The chat says: The page asks for the letters",
  options: [{ id: "take-over", label: "Take over", allows: true }],
  source: "browser-needs-you",
  chain: ["qa 8"],
  answer: { via: "take-over", program: "P1" },
};

const BROWSER_SITES: Shown = {
  session: 8,
  ask: "browser-sites:8",
  says: "Its browser wants to reach news.example.org:443. An Allow is its browser's alone, from its next act; nothing restarts.",
  options: [
    { id: "chat", label: "Allow for this chat", allows: true },
    { id: "you", label: "Always allow in this project, on this machine", allows: true },
    { id: "project", label: "Allow for everyone in this project", allows: true },
    { id: "keep", label: "Keep blocked", allows: false },
  ],
  source: "browser-site",
  chain: ["steward 8"],
  answer: { via: "browser-sites", sites: ["news.example.org:443"] },
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
      ["dismiss_sign_in", { plane: PLANE, profile: "oc", pool: null }],
    ]);
  });

  it("signs a profile in on a pool's runner by opening its shell tab, and puts it away there (#2049)", async () => {
    const onPool: Shown = {
      ...SIGN_IN,
      ask: "sign-in:oc:on:builds",
      answer: { via: "sign-in", profile: "oc", pool: "builds" },
    };
    const calls = sent({ dismiss_sign_in: null });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(PROFILE_SIGN_IN, heard);
    try {
      expect(await answerTaken(PLANE, onPool, "sign-in:openai")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
      expect(opened).toEqual([{ plane: PLANE, profile: "oc", provider: "openai", pool: "builds" }]);
      expect(calls).toEqual([]);
      await answerTaken(PLANE, onPool, "dismiss");
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["dismiss_sign_in", { plane: PLANE, profile: "oc", pool: "builds" }],
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

  it("removes the old harness plugin keys by opening purlis's proposed change, and puts it away by Dismiss (#3467)", async () => {
    const calls = sent({
      remove_old_harness_plugins: [{ id: 4, file: "charter.toml" }],
      dismiss_old_harness_plugins: null,
    });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent).detail);
    window.addEventListener(OPEN_PROPOSAL_ASK, heard);
    try {
      expect(await answerTaken(PLANE, OLD_PLUGINS, "remove")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
      expect(opened).toEqual([{ plane: PLANE, id: 4, path: "charter.toml" }]);
      expect(await answerTaken(PLANE, OLD_PLUGINS, "dismiss")).toEqual({
        refused: undefined,
        live: false,
      });
    } finally {
      window.removeEventListener(OPEN_PROPOSAL_ASK, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["remove_old_harness_plugins", { plane: PLANE }],
      ["dismiss_old_harness_plugins", { plane: PLANE }],
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

  it("takes a browser that needs the person over, then opens its view, and the Ask waits on for their Done (#2015)", async () => {
    const calls = sent({ take_over_browser: null });
    const opened: OpenBrowserAsk[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent<OpenBrowserAsk>).detail);
    window.addEventListener(OPEN_BROWSER_ASK, heard);
    try {
      expect(await answerTaken(PLANE, NEEDS_YOU, "take-over")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
    } finally {
      window.removeEventListener(OPEN_BROWSER_ASK, heard);
    }
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["take_over_browser", { program: "P1" }],
    ]);
    expect(opened).toEqual([{ plane: PLANE, program: "P1" }]);
  });

  it("only opens the view of a browser the person already drives, sending nothing (#2015)", async () => {
    const calls = sent();
    const driving: Shown = {
      ...NEEDS_YOU,
      says: "You are driving its browser b1: Done in its view hands it back. The chat said: q",
      options: [{ id: "open-view", label: "Open its view", allows: false }],
    };
    const opened: OpenBrowserAsk[] = [];
    const heard = (event: Event) => opened.push((event as CustomEvent<OpenBrowserAsk>).detail);
    window.addEventListener(OPEN_BROWSER_ASK, heard);
    try {
      expect(await answerTaken(PLANE, driving, "open-view")).toEqual({
        refused: undefined,
        live: false,
        opened: true,
      });
    } finally {
      window.removeEventListener(OPEN_BROWSER_ASK, heard);
    }
    expect(calls).toEqual([]);
    expect(opened).toEqual([{ plane: PLANE, program: "P1" }]);
  });

  it("opens no view of a browser the host would not hand over (#2015)", async () => {
    sent({ take_over_browser: new Error("this browser has ended") });
    const opened: unknown[] = [];
    const heard = (event: Event) => opened.push(event);
    window.addEventListener(OPEN_BROWSER_ASK, heard);
    try {
      expect(await answerThrough(PLANE, NEEDS_YOU, "take-over")).toBe("this browser has ended");
    } finally {
      window.removeEventListener(OPEN_BROWSER_ASK, heard);
    }
    expect(opened).toEqual([]);
  });

  it("allows a browser's sites, the ones the ask showed, for the chat or always here, or keeps them blocked with the reason (#2018)", async () => {
    const calls = sent({ allow_browser_sites: null, keep_browser_sites_blocked: null });
    await answerThrough(PLANE, BROWSER_SITES, "chat");
    await answerThrough(PLANE, BROWSER_SITES, "you");
    await answerThrough(PLANE, BROWSER_SITES, "project");
    await answerTaken(PLANE, BROWSER_SITES, "keep", "  not this one ");
    const sites = ["news.example.org:443"];
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["allow_browser_sites", { plane: PLANE, session: 8, sites, level: "chat" }],
      ["allow_browser_sites", { plane: PLANE, session: 8, sites, level: "you" }],
      ["allow_browser_sites", { plane: PLANE, session: 8, sites, level: "project" }],
      ["keep_browser_sites_blocked", { plane: PLANE, session: 8, sites, reason: "not this one" }],
    ]);
  });

  it("sends no Allow for a browser sites ask that offers Keep blocked alone, and keeps it blocked (#2364)", async () => {
    const calls = sent({ allow_browser_sites: null, keep_browser_sites_blocked: null });
    const keepOnly: Shown = {
      ...BROWSER_SITES,
      options: BROWSER_SITES.options.filter((one) => !one.allows),
    };
    const refused = await answerThrough(PLANE, keepOnly, "chat");
    expect(refused).toMatch(/not one of the answers this ask offers/);
    await answerTaken(PLANE, keepOnly, "keep");
    expect(calls.map((one) => one.cmd)).toEqual(["keep_browser_sites_blocked"]);
  });

  it("tells what reads a chat's network to read it again once a browser site is allowed (#2364)", async () => {
    sent({ allow_browser_sites: null });
    const { result } = renderHook(() => useSandboxCommands());
    const before = result.current;
    await act(() => answerThrough(PLANE, BROWSER_SITES, "chat"));
    expect(result.current).toBeGreaterThan(before);
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

describe("a runner chat's open or dispatch", () => {
  it("is refused through its own command, and opened in its tab to approve", async () => {
    const calls = sent({ refuse_runner_dispatch: null });
    const asked: unknown[] = [];
    const heard = (event: Event) => asked.push((event as CustomEvent).detail);
    window.addEventListener(OPEN_RUNNER_DISPATCH_ASK, heard);
    expect(await answerTaken(PLANE, RUNNER_DISPATCH, "open")).toEqual({
      refused: undefined,
      live: false,
    });
    window.removeEventListener(OPEN_RUNNER_DISPATCH_ASK, heard);
    expect(asked).toEqual([{ plane: PLANE, id: 2 }]);
    expect(await answerTaken(PLANE, RUNNER_DISPATCH, "refuse")).toEqual({
      refused: undefined,
      live: false,
    });
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["refuse_runner_dispatch", { plane: PLANE, id: 2 }],
    ]);
  });
});

describe("a runner chat's change to the project (#2046)", () => {
  it("is applied or dismissed through its own commands, and nothing else", async () => {
    const calls = sent({ apply_runner_write: "Applied.", dismiss_runner_write: null });
    expect(await answerTaken(PLANE, RUNNER_WRITE, "apply")).toEqual({
      refused: undefined,
      live: false,
    });
    expect(await answerTaken(PLANE, RUNNER_WRITE, "dismiss")).toEqual({
      refused: undefined,
      live: false,
    });
    expect(calls.map((one) => [one.cmd, one.args])).toEqual([
      ["apply_runner_write", { plane: PLANE, id: 3, from: "row" }],
      ["dismiss_runner_write", { plane: PLANE, id: 3 }],
    ]);
  });

  it("opens one its row cannot show whole in its own tab, and sends nothing", async () => {
    const calls = sent({});
    const asked: unknown[] = [];
    const heard = (event: Event) => asked.push((event as CustomEvent).detail);
    window.addEventListener(OPEN_RUNNER_WRITE_ASK, heard);
    const opens: Shown = {
      ...RUNNER_WRITE,
      options: [
        { id: "open", label: "Open", allows: false },
        { id: "dismiss", label: "Dismiss", allows: false },
      ],
    };
    expect(await answerTaken(PLANE, opens, "open")).toEqual({
      refused: undefined,
      live: false,
      opened: true,
    });
    expect(await answerTaken(PLANE, opens, "apply")).toMatchObject({
      refused: expect.stringContaining("is not one of the answers"),
    });
    window.removeEventListener(OPEN_RUNNER_WRITE_ASK, heard);
    expect(asked).toEqual([{ plane: PLANE, id: 3 }]);
    expect(calls).toEqual([]);
  });
});
