// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { Moved, OpenChat } from "./bindings";
import { forgetDismissals } from "./dismissals";
import { askToEnd } from "./menus.testkit";
import { stripNamed } from "./test-strips";
import { forgetInboxOpen, inboxOpenAtLaunch } from "./test-inbox";

/**
 * **A chat whose program ended keeps its pane** (#4756, spec #4750 stories 21 and 93), against
 * the whole window: its last screen stays, read-only, under an ended band with Resume and Clear
 * pane; a reader hears it once; the keyboard stays where it was; and nothing ends a chat a second
 * time.
 */

/** Each pane says which chat it draws, and whether its screen takes input. */
vi.mock("./SessionPane", () => ({
  SessionPane: ({ session, readOnly }: { session: number; readOnly?: boolean }) => (
    <div data-testid="pane" data-read-only={readOnly ? "" : undefined}>
      session {session}
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT: OpenChat = {
  card: null,
  session: 4,
  name: "claude 4",
  cwd: `${ALPHA}/repo`,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: "claude",
  persona: null,
  unreported: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

const WAITING: Moved = {
  plane: PLANE,
  session: 4,
  state: "waiting",
  needs_you: false,
  queue: [],
  moved_at: 1,
  reports: [],
  refusals: [],
  children: [],
  sequence: 1,
};
/** Its program ended, content: what the core says when the session itself ends. */
const DONE: Moved = { ...WAITING, state: "done", moved_at: 2, sequence: 2 };

/** Another chat in alpha, with a tab of its own. */
const OTHER = { ...CHAT, session: 5, name: "claude 5", in_front: false };

function core(open: OpenChat[] = [CHAT]) {
  const asked: { cmd: string; args: Record<string, unknown> }[] = [];
  mockIPC(
    (cmd, args) => {
      asked.push({ cmd, args: (args ?? {}) as Record<string, unknown> });
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return open;
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: open }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "smart_close_offer") return { available: true, why: null, close_first: false };
      if (cmd === "smart_close") return "sent";
      if (cmd === "smart_closing") return [];
      if (cmd === "resume_session") return { ...CHAT, session: 9, resumed: "c1" };
      if (cmd === "resume_ended_chat") return { ...CHAT, session: 11, resumed: "c1" };
      if (cmd === "restart_chat")
        return { chat: { ...CHAT, session: 9, resumed: "c1" }, notices: [], not_yet: null };
      return null;
    },
    { shouldMockEvents: true },
  );
  return {
    asked: (cmd: string) => asked.filter((one) => one.cmd === cmd).map(({ args }) => args),
  };
}

async function settled() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

/** Chat 4 on screen, then its program ends. */
async function anEndedChat(open: OpenChat[] = [CHAT]) {
  const said = core(open);
  render(<App />);
  await screen.findByTestId("pane");
  await settled();
  await act(() => emit("chat-moved", WAITING));
  return said;
}

const band = () => screen.findByRole("status", { name: /ended/i });

beforeEach(() => inboxOpenAtLaunch());
afterEach(() => {
  cleanup();
  clearMocks();
  forgetDismissals();
  forgetInboxOpen();
});

describe("a chat whose program ended (#4756)", () => {
  it("keeps its pane and its last screen, read-only, under an ended band", async () => {
    const { asked } = await anEndedChat();
    expect(screen.getByTestId("pane")).not.toHaveAttribute("data-read-only");

    await act(() => emit("chat-moved", DONE));

    const ended = await band();
    expect(ended).toHaveTextContent("claude 4 ended. Resume or Clear pane.");
    expect(within(ended).getByRole("button", { name: "Resume" })).toBeInTheDocument();
    expect(within(ended).getByRole("button", { name: "Clear pane" })).toBeInTheDocument();
    expect(screen.getByText("session 4")).toHaveAttribute("data-read-only");
    expect(asked("close_session")).toEqual([]);
  });

  it("leaves the keyboard where it was as the band comes", async () => {
    await anEndedChat();
    const before = document.activeElement;

    await act(() => emit("chat-moved", DONE));
    await band();

    expect(document.activeElement).toBe(before);
  });

  it("clears the pane on Clear pane, and ends nothing", async () => {
    const { asked } = await anEndedChat();
    await act(() => emit("chat-moved", DONE));

    await userEvent.click(within(await band()).getByRole("button", { name: "Clear pane" }));

    expect(await screen.findByText("Nothing in this pane")).toBeInTheDocument();
    expect(screen.queryByTestId("pane")).toBeNull();
    expect(asked("close_session")).toEqual([]);
  });

  it("starts it again on its conversation on Resume, in its pane", async () => {
    const { asked } = await anEndedChat();
    await act(() => emit("chat-moved", DONE));

    await userEvent.click(within(await band()).getByRole("button", { name: "Resume" }));

    await waitFor(() => expect(asked("restart_chat")).toHaveLength(1));
    expect(await screen.findByText("session 9")).toBeInTheDocument();
  });

  it("draws no band while the chat runs or waits", async () => {
    await anEndedChat();

    expect(screen.queryByRole("status", { name: /ended/i })).toBeNull();
  });
});

/** Chat 4 as its pane and its row name it: its harness, then its name. */
const NAMED = "claude claude 4";

/** The names on the strip's tabs, left to right. */
const tabNames = () =>
  within(stripNamed("Tabs"))
    .queryAllByRole("tab")
    .map((tab) => tab.querySelector(".tab-name")?.textContent);

/** Chat 4 on screen, ended on purpose: End…, answered with its Close. */
async function endedWithEnd(open: OpenChat[] = [CHAT]) {
  const said = await anEndedChat(open);
  const asking = await askToEnd(NAMED);
  await userEvent.click(within(asking).getByRole("button", { name: "Close" }));
  await waitFor(() => expect(said.asked("close_session")).toEqual([{ plane: PLANE, session: 4 }]));
  return said;
}

describe("a chat ended on purpose (#5019, spec #4750 story 62)", () => {
  it("leaves its pane on the ended band after End…, read-only, with Resume and Clear pane", async () => {
    await endedWithEnd();

    const ended = await band();
    expect(ended).toHaveTextContent("claude 4 ended. Resume or Clear pane.");
    expect(within(ended).getByRole("button", { name: "Resume" })).toBeInTheDocument();
    expect(within(ended).getByRole("button", { name: "Clear pane" })).toBeInTheDocument();
    expect(screen.getByText("session 4")).toHaveAttribute("data-read-only");
    expect(tabNames()).toEqual([NAMED]);
  });

  it("keeps the keyboard in the chat's pane after End…", async () => {
    const said = core();
    render(<App />);
    await screen.findByTestId("pane");
    await settled();
    const asking = await askToEnd(NAMED);
    await userEvent.click(within(asking).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(said.asked("close_session")).toHaveLength(1));
    await band();

    // The dialog gives the keyboard back to where it was, and the pane is still there to have it.
    expect(screen.getByTestId("pane").closest(".pane-frame")).toContainElement(
      document.activeElement as HTMLElement,
    );
  });

  it("resumes it on its conversation from the band after End…, in its pane", async () => {
    const { asked } = await endedWithEnd();

    await userEvent.click(within(await band()).getByRole("button", { name: "Resume" }));

    await waitFor(() => expect(asked("resume_ended_chat").map((one) => one.session)).toEqual([4]));
    expect(await screen.findByText("session 11")).toBeInTheDocument();
    expect(screen.queryByText("session 4")).toBeNull();
    expect(screen.queryByRole("status", { name: /ended/i })).toBeNull();
    expect(tabNames()).toHaveLength(1);
  });

  it("clears a shell's pane on its End…, with no band (story 64)", async () => {
    const shell = { ...CHAT, harness: null, profile: null, name: "shell 4" };
    const { asked } = await anEndedChat([shell]);
    const asking = await askToEnd("shell 4");
    await userEvent.click(within(asking).getByRole("button", { name: "End" }));
    await waitFor(() => expect(asked("close_session")).toEqual([{ plane: PLANE, session: 4 }]));

    await waitFor(() => expect(screen.queryByTestId("pane")).toBeNull());
    expect(screen.queryByRole("status", { name: /ended/i })).toBeNull();
  });

  it("closes the pane on Clear pane, ending nothing more, and the strip's last tab keeps one empty pane", async () => {
    const { asked } = await endedWithEnd();

    await userEvent.click(within(await band()).getByRole("button", { name: "Clear pane" }));

    expect(await screen.findByText("Nothing in this pane")).toBeInTheDocument();
    expect(screen.queryByTestId("pane")).toBeNull();
    expect(asked("close_session")).toHaveLength(1);
  });

  it("closes the tab on Clear pane where it is not the strip's last", async () => {
    await endedWithEnd([CHAT, OTHER]);
    expect(tabNames()).toEqual([NAMED, "claude claude 5"]);

    await userEvent.click(within(await band()).getByRole("button", { name: "Clear pane" }));

    await waitFor(() => expect(tabNames()).toEqual(["claude claude 5"]));
    expect(screen.queryByText("Nothing in this pane")).toBeNull();
  });

  it("leaves its pane on the ended band once a Smart close's record is written, and Resume starts it from the record in that pane", async () => {
    const { asked } = await anEndedChat();
    const asking = await askToEnd(NAMED);
    await userEvent.click(within(asking).getByRole("button", { name: "Smart close" }));
    await waitFor(() => expect(asked("smart_close")).toHaveLength(1));

    const record = { path: "workspaces/alpha/sessions/2026-11-03-claude-4.md", title: "Claude 4" };
    await act(() => emit("smart-close", { plane: PLANE, session: 4, phase: "closed", record }));

    const ended = await band();
    expect(ended).toHaveTextContent("claude 4 ended. Resume or Clear pane.");
    // And a link to the record it wrote (story 62).
    expect(within(ended).getByRole("button", { name: "Open record" })).toBeInTheDocument();
    expect(screen.getByText("session 4")).toHaveAttribute("data-read-only");
    // The core closed it with its record: the window ends nothing a second time.
    expect(asked("close_session")).toEqual([]);

    await userEvent.click(within(ended).getByRole("button", { name: "Resume" }));

    await waitFor(() =>
      expect(asked("resume_session").map((one) => one.path)).toEqual([record.path]),
    );
    // The resumed chat takes the pane the ended one was left in, not a tab of its own.
    expect(await screen.findByText("session 9")).toBeInTheDocument();
    expect(screen.queryByText("session 4")).toBeNull();
    expect(tabNames()).toHaveLength(1);
  });
});
