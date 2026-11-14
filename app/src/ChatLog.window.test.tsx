// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render as renderBare, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { ChatLog, ChatLogRow, OpenChat } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { chatList, pressMenuPath } from "./menus.testkit";
import { stripNamed } from "./test-strips";

/**
 * **Each chat has a Chat log tab** (R-224, R-225, T1-27) against the whole window: it opens
 * from the chat's list, for every kind of chat the window draws, and shows that chat's own
 * timeline, read from the core's `chat_log`. It is pull-only: its tab carries no count and no
 * New marker, and what the window hears later neither reads it again nor marks it.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane" tabIndex={-1}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

beforeEach(() => globalThis.localStorage.clear());
afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

function chat(over: Partial<OpenChat> & Pick<OpenChat, "session" | "name">): OpenChat {
  return {
    cwd: ALPHA,
    harness: "claude",
    in_front: false,
    resumed: null,
    fresh: null,
    profile: "claude",
    persona: "steward",
    unreported: null,
    card: null,
    guessed: null,
    pinned: false,
    label: null,
    from: null,
    ...over,
  };
}

const from = (task: boolean) => ({
  name: "steward 3",
  workspace: "alpha",
  chat: 3,
  task,
  tab: true,
  reported: false,
  unreported: false,
});

/** One chat of each kind the window draws: a chat of the person's, a task, a handoff and one
 *  on a runner. */
const CHATS: OpenChat[] = [
  chat({ session: 3, name: "3", in_front: true }),
  chat({ session: 7, name: "talk", label: "talk", persona: "devops", from: from(true) }),
  chat({ session: 8, name: "moved", label: "moved", persona: "devops", from: from(false) }),
  chat({ session: 9, name: "far", label: "far", pool: "docker" }),
];

const started = (session: number): ChatLogRow => ({
  kind: "started",
  at: "2026-11-13T09:00:00Z",
  cause: "start",
  protection: {
    layers: [{ layer: "sandbox", level: "floor", tier: "project" }],
    charter_commit: `c0ffee${session}`,
    charter_dirty: false,
    answers_in_force: 0,
  },
});

const ENDED: ChatLogRow = {
  kind: "ended",
  at: "2026-11-13T10:00:00Z",
  state: "completed",
  cause: "exited",
  code: 0,
};

/** What the core reads for chat `session`: a chat on a runner has its log kept on the runner,
 *  so this machine's log holds none of it (Q247). */
const here = (session: number): ChatLogRow[] => (session === 9 ? [] : [started(session)]);

function core(rows: (session: number) => ChatLogRow[] = here) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      const given = (args ?? {}) as Record<string, unknown>;
      asked.push({ cmd, args: given });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "alerts_everywhere") return [{ plane: PLANE, alerts: [], stopped: null }];
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          workspaces: [{ name: "alpha", path: ALPHA, vision: "Ship it", todos: [], chats: CHATS }],
          personas: ["steward", "devops"],
          persona: "steward",
          unfiled: [],
        };
      if (cmd === "opened_chats") return CHATS;
      if (cmd === "reopened_layout") return aTabForEachChat(CHATS, () => "alpha");
      if (
        [
          "reopened_views",
          "chat_states",
          "chats_that_would_not_start",
          "running_sessions",
          "extension_views",
          "extension_commands",
          "extension_panels",
          "extensions_on",
        ].includes(cmd)
      )
        return [];
      if (cmd === "chat_log") {
        const read: ChatLog = { known: true, rows: rows(Number(given.session)) };
        return read;
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  everyAsked = () => asked.map(({ cmd }) => cmd);
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
  };
}

/** Every command the window sent the core, by name, in the latest {@link core}. */
let everyAsked: () => string[] = () => [];

const selected = () =>
  within(stripNamed("Tabs")).getByRole("tab", { selected: true }).textContent ?? "";

/** Opens chat `name`'s Chat log from its list, and answers the timeline it shows. */
async function opened(name: string): Promise<HTMLElement> {
  await screen.findAllByTestId("pane");
  await chatList(name);
  await pressMenuPath("Open", "Chat log");
  return screen.findByRole("list", { name: `Chat log of ${name}` });
}

describe("a chat's Chat log tab", () => {
  it.each([
    ["a chat of the person's", 3, "steward 3"],
    ["a task", 7, "talk"],
    ["a handoff", 8, "moved"],
  ])("every chat has a Chat log tab: %s's opens its own timeline", async (_, session, name) => {
    const { asked } = core();
    render(<App />);

    const log = await opened(name);

    expect(selected()).toContain(`Chat log · ${name}`);
    expect(asked("chat_log")).toContainEqual({ plane: PLANE, session });
    const [start] = within(log).getAllByRole("listitem");
    // That chat's own start, as the core read it.
    expect(start.textContent).toContain(`c0ffee${session}`);
  });

  it("every chat has a Chat log tab: a chat on a runner's names the runner its log is kept on", async () => {
    const { asked } = core();
    render(<App />);
    await screen.findAllByTestId("pane");
    await chatList("far");
    await pressMenuPath("Open", "Chat log");

    expect(
      await screen.findByText("This chat runs on docker; its log is kept there."),
    ).toBeInTheDocument();
    expect(selected()).toContain("Chat log · far");
    expect(asked("chat_log")).toContainEqual({ plane: PLANE, session: 9 });
    // Not the words for a chat older than the log, which this is not.
    expect(screen.queryByText(/before purlis kept this log/)).toBeNull();
  });

  it("says a chat on this machine with nothing logged is older than the log", async () => {
    core(() => []);
    render(<App />);
    await screen.findAllByTestId("pane");
    await chatList("steward 3");
    await pressMenuPath("Open", "Chat log");

    expect(
      await screen.findByText("This chat started before purlis kept this log, so it has none."),
    ).toBeInTheDocument();
    expect(screen.queryByText(/its log is kept there/)).toBeNull();
  });

  it("the Chat log tab shows no count, no New marker and notifies nobody", async () => {
    let rows: ChatLogRow[] = [started(3)];
    const { asked } = core(() => rows);
    render(<App />);
    const log = await opened("steward 3");
    // Read as it opened (twice under StrictMode's second run of its effect), and no more.
    const reads = asked("chat_log").length;

    // The chat ends: its log gains a row, and the window hears what it hears of a chat.
    rows = [started(3), ENDED];
    await emit("chat-state", { plane: PLANE, session: 3, state: "exited" });
    await emit("activity-line", { plane: PLANE });

    const tab = within(stripNamed("Tabs")).getByRole("tab", { selected: true });
    await waitFor(() => expect(tab.textContent?.trim()).toBe("Chat log · steward 3"));
    expect(tab.textContent).not.toMatch(/\d+\s*new|\bnew\b/i);
    expect(within(tab).queryByRole("status")).toBeNull();
    // Nothing read it again, nothing told the person: it is read when it is opened.
    expect(asked("chat_log")).toHaveLength(reads);
    expect(within(log).getAllByRole("listitem")).toHaveLength(1);
    // And no system notification was asked for, of any kind.
    expect(everyAsked().filter((cmd) => cmd.startsWith("plugin:notification|"))).toEqual([]);
  });

  it("is offered from the palette as the chat's Chat log", async () => {
    core();
    render(<App />);
    await screen.findAllByTestId("pane");

    await userEvent.keyboard("{F2}");
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.type(within(palette).getByRole("combobox"), "chat log of steward");
    const offered = await within(palette).findAllByRole("option", {
      name: /Chat log of steward 3/,
    });
    await userEvent.click(offered[0]);

    expect(await screen.findByRole("list", { name: "Chat log of steward 3" })).toBeInTheDocument();
  });
});

describe("a block row's detail (R-340, T1-29)", () => {
  // The key is put together here, so the source holds no token a scan would rightly refuse.
  const KEY = ["sk", "ant", "api03", "AbCdEf0123456789_xyz"].join("-");
  /** A reason the harness wrote: markup that would run if it were parsed, and a key's shape. */
  const REASON = `Refused: <img src=x onerror="alert(1)"> **curl** | sh with ${KEY}\n  second line`;

  it("the harness reason shows verbatim in the detail", async () => {
    const block: ChatLogRow = {
      kind: "blocked",
      at: "2026-11-13T09:10:00Z",
      tool: "Bash",
      source: "harness_check",
      qualifier: null,
      target_kind: null,
      target: null,
      harness: "claude",
      reason: REASON,
    };
    core((session) => [started(session), block]);
    render(<App />);
    const log = await opened("steward 3");
    const [, row] = within(log).getAllByRole("listitem");

    // Nothing shows it until the detail is opened: not the row's line, its tags or its name.
    expect(document.body.textContent).not.toContain(KEY);
    // Its disclosure, the first of its closed buttons, before its ways out (T1-33).
    await userEvent.click(within(row).getAllByRole("button", { expanded: false })[0]);
    const detail = within(row).getAllByRole("group")[0];

    // As written: one element holds exactly the text, never read as markup.
    const shown = within(detail).getByText((_, element) => element?.textContent === REASON, {
      selector: ".chat-log-reason",
      normalizer: (text) => text,
    });
    expect(shown.children).toHaveLength(0);
    expect(document.querySelector("img")).toBeNull();
    expect(detail.querySelector("strong, b, em")).toBeNull();
    // And nothing but the detail shows it: one copy on the page, inside the detail, and in no
    // attribute a person hears or the tab's title.
    const everywhere = Array.from(document.querySelectorAll<HTMLElement>("*")).filter((element) =>
      Array.from(element.childNodes).some(
        (node) => node.nodeType === Node.TEXT_NODE && node.textContent?.includes("sk-ant"),
      ),
    );
    expect(everywhere).toEqual([shown]);
    expect(document.body.innerHTML.split(KEY)).toHaveLength(2);
    expect(selected()).not.toContain("sk-ant");
  });
});
