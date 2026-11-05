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
import type { Shown } from "./bindings";
import { aTabForEachChat } from "./laidOut.testkit";
import { forgetInboxOpen, inboxAwayAtLaunch } from "./test-inbox";
import { forgetInbox } from "./inboxRules";

/**
 * **The pane's notices band as compact rows, against the whole window** (#5215, the approved
 * prototype's live pane). What a person sees in a chat's pane: one row a notice, each with its
 * own buttons and a ×; a head with how many wait and a fold; one line when folded; and a long
 * message opened whole with Esc to fold it.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">
      session {session}
      <textarea className="xterm-helper-textarea" aria-label="Terminal input" />
    </div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

beforeEach(() => inboxAwayAtLaunch("minimised"));
afterEach(forgetInboxOpen);
afterEach(forgetInbox);
afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
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

const removal = (workspace: string): Shown => ({
  session: 4,
  ask: `removal:4:${workspace}`,
  says: `Removing workspace ${workspace} deletes its memory, its todos and its clones`,
  options: [
    { id: "remove", label: "Remove…", allows: false },
    { id: "keep", label: "Keep", allows: false },
  ],
  source: "workspace-removal",
  chain: ["claude 4"],
  answer: { via: "workspace-removal", workspace, memories: 0, todos: 0 },
  since: 10,
});

/** The chat on screen stopped on a prompt in its own terminal. */
const PROMPT: Shown = {
  session: 4,
  ask: "terminal:4:1",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["claude 4"],
  answer: { via: "in-its-pane" },
  since: Math.floor(Date.now() / 1000),
};

/** Chat 4 is a task, and its question has reached the person. */
const QUESTION: Shown = {
  session: 4,
  ask: "task-question:4:1",
  says: "Which branch?\nSay why.\n\n- **main**\n- the fork",
  options: [],
  source: "task-question",
  chain: ["steward 1", "claude 4"],
  answer: { via: "task-question", why: "no-reply", from: "steward 1", note: null },
  since: 10,
};

/** A note: a block of chat 4's own work. */
const BLOCKED = {
  plane: PLANE,
  session: 4,
  operation: "write",
  kind: "toolchain-cache",
  ours: false,
  harness: "claude",
  said: "a write to a toolchain's package cache",
  offer: "none",
  target: null,
  route: null,
  levels: [],
  held: false,
  ruled: null,
};

function core(asks: Shown[]) {
  const layouts: string[] = [];
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "reopened_layout") return aTabForEachChat([CHAT], () => "alpha");
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [CHAT] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "owed_restarts") return [];
      if (cmd === "asks_waiting") return { plane: PLANE, asks };
      if (cmd === "mcp_failures") return [];
      if (cmd === "task_question")
        return { task: "claude 4", asked: "steward 1", number: 1, question: QUESTION.says };
      if (cmd === "write_layout") layouts.push((args as { text: string }).text);
      return null;
    },
    { shouldMockEvents: true },
  );
  return { layouts };
}

async function aPane(asks: Shown[]) {
  const said = core(asks);
  render(<App />);
  await screen.findByTestId("pane");
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return said;
}

const frame = () => screen.getByTestId("pane").closest(".pane-frame") as HTMLElement;
const bell = () => within(frame()).getByRole("button", { name: /^Pane notices:/ });
const band = () => within(frame()).getByRole("region", { name: /^Pane notices/ });
const unfold = async () =>
  userEvent.click(await within(frame()).findByRole("button", { name: /·\s*Show$/ }));

describe("the band folded (the default)", () => {
  it("is one line naming how many wait and on what, and shows no row", async () => {
    await aPane([removal("beta"), removal("gamma"), PROMPT]);

    const line = await within(frame()).findByRole("button", {
      name: /^3 waiting · Terminal prompt, Remove workspace\s*·\s*Show$/,
    });
    expect(band()).toContainElement(line);
    expect(band()).toHaveAccessibleName("Pane notices: 3 waiting on you");
    expect(within(frame()).queryByRole("status", { name: "Remove workspace" })).toBeNull();
    expect(within(frame()).queryByRole("button", { name: "Keep" })).toBeNull();
  });

  it("opens from its line, keeps the choice, and gives the keyboard to the fold", async () => {
    const { layouts } = await aPane([removal("beta")]);

    await unfold();

    expect(await within(frame()).findByRole("status", { name: "Remove workspace" })).toBeTruthy();
    const fold = within(frame()).getByRole("button", { name: "Fold the band" });
    await waitFor(() => expect(fold).toHaveFocus());
    await waitFor(() =>
      expect(layouts.map((text) => JSON.parse(text) as { notices?: string }).pop()?.notices).toBe(
        "expanded",
      ),
    );
  });

  it("folds from the head, and gives the keyboard to its line", async () => {
    await aPane([removal("beta")]);
    await unfold();

    await userEvent.click(await within(frame()).findByRole("button", { name: "Fold the band" }));

    const line = await within(frame()).findByRole("button", { name: /·\s*Show$/ });
    await waitFor(() => expect(line).toHaveFocus());
    expect(within(frame()).queryByRole("status", { name: "Remove workspace" })).toBeNull();
  });

  it("draws nothing but the bell where only notes are held", async () => {
    await aPane([]);
    await act(() => emit("chat-sandbox-blocked", BLOCKED));

    await waitFor(() =>
      expect(bell()).toHaveAccessibleName("Pane notices: 1 notice, 1 in trouble"),
    );
    expect(within(frame()).queryByRole("button", { name: /·\s*Show$/ })).toBeNull();
    expect(within(frame()).queryByRole("region", { name: /^Pane notices/ })).toBeNull();
  });
});

describe("a row", () => {
  it("draws its title, its sub-line, its own buttons in order and a ×", async () => {
    await aPane([removal("beta")]);
    await unfold();

    const row = await within(frame()).findByRole("status", { name: "Remove workspace" });
    expect(within(row).getByText(/asks you to remove workspace/)).toBeTruthy();
    expect(within(row).getByText("Remove workspace")).toBeTruthy();
    const buttons = within(row)
      .getAllByRole("button")
      .map((one) => one.getAttribute("aria-label") ?? one.textContent);
    expect(buttons.at(-1)).toBe("Close");
    expect(buttons.slice(0, -1)).toEqual(expect.arrayContaining(["Keep"]));
  });

  it("closes an unanswered ask from this pane alone: its count goes, and the ✋ keeps it", async () => {
    await aPane([removal("beta"), removal("gamma")]);
    await unfold();
    const rows = await within(frame()).findAllByRole("status", { name: "Remove workspace" });
    expect(rows).toHaveLength(2);

    await userEvent.click(within(rows[0]).getByRole("button", { name: "Close" }));

    await waitFor(() =>
      expect(within(frame()).getAllByRole("status", { name: "Remove workspace" })).toHaveLength(1),
    );
    await waitFor(() => expect(band()).toHaveAccessibleName("Pane notices: 1 waiting on you"));
    await waitFor(() => expect(bell()).toHaveAccessibleName("Pane notices: No notices, 1 asking"));
    // The registry still lists both: the window's own count of what waits says 2.
    expect(screen.getAllByRole("button", { name: /2 .*wait/i }).length).toBeGreaterThan(0);
  });

  it("dismisses a note with its × and leaves the band's head count right", async () => {
    await aPane([removal("beta")]);
    await act(() => emit("chat-sandbox-blocked", BLOCKED));
    await unfold();
    const note = await within(frame()).findByRole("status", { name: "Sandbox block" });

    await userEvent.click(within(note).getByRole("button", { name: "Dismiss" }));

    await waitFor(() =>
      expect(within(frame()).queryByRole("status", { name: "Sandbox block" })).toBeNull(),
    );
    expect(band()).toHaveAccessibleName("Pane notices: 1 waiting on you");
  });
});

describe("a terminal prompt in the band", () => {
  it("is a row whose Go to prompt puts the keyboard in the terminal", async () => {
    await aPane([PROMPT]);
    await unfold();

    const row = await within(frame()).findByRole("status", { name: "Terminal prompt" });
    expect(within(row).getByText("Waiting in its terminal: a permission prompt")).toBeTruthy();
    expect(within(row).getByText(/Answer it in the terminal below/)).toBeTruthy();

    await userEvent.click(within(row).getByRole("button", { name: "Go to prompt" }));

    expect(within(frame()).getByLabelText("Terminal input")).toHaveFocus();
  });
});

describe("a task's question in the band", () => {
  it("is one row with its first line, and opens whole with its answer", async () => {
    await aPane([QUESTION]);
    await unfold();

    const quiz = await within(frame()).findByRole("group", { name: "claude 4 asks" });
    expect(within(quiz).getByText(/Which branch\?/)).toBeTruthy();
    expect(within(quiz).queryByRole("listitem")).toBeNull();
    expect(within(quiz).queryByRole("textbox", { name: "Your answer" })).toBeNull();

    await userEvent.click(within(quiz).getByRole("button", { name: "Show the whole question" }));

    expect(within(quiz).getAllByRole("listitem")).toHaveLength(2);
    expect(within(quiz).getByRole("textbox", { name: "Your answer" })).toBeTruthy();
    expect(within(quiz).queryByRole("button", { name: /Show all/ })).toBeNull();
  });

  it("folds an opened question with Esc, keeps what was typed, and lands on its toggle", async () => {
    await aPane([QUESTION]);
    await unfold();
    const quiz = await within(frame()).findByRole("group", { name: "claude 4 asks" });
    await userEvent.click(within(quiz).getByRole("button", { name: "Show the whole question" }));
    within(quiz).getByRole("textbox", { name: "Your answer" }).focus();
    await userEvent.paste("main");

    // The tooltip the toggle's press left up takes the first Esc (`ui/tooltip`), the quiz the next.
    await userEvent.keyboard("{Escape}");
    await userEvent.keyboard("{Escape}");

    const toggle = within(quiz).getByRole("button", { name: "Show the whole question" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveFocus();
    await userEvent.click(toggle);
    expect(within(quiz).getByRole("textbox", { name: "Your answer" })).toHaveValue("main");
  });

  it("answers in the Inbox from its row", async () => {
    await aPane([QUESTION]);
    await unfold();
    const quiz = await within(frame()).findByRole("group", { name: "claude 4 asks" });

    await userEvent.click(within(quiz).getByRole("button", { name: "Answer" }));

    await waitFor(() => expect(screen.getByRole("region", { name: /^Inbox/ })).toBeInTheDocument());
  });

  it("leaves the pane's bell when closed", async () => {
    await aPane([QUESTION]);
    await unfold();
    const quiz = await within(frame()).findByRole("group", { name: "claude 4 asks" });
    await waitFor(() => expect(bell()).toHaveAccessibleName("Pane notices: No notices, 1 asking"));

    await userEvent.click(within(quiz).getByRole("button", { name: "Close question" }));

    await waitFor(() => expect(bell()).toHaveAccessibleName("Pane notices: No notices"));
    expect(within(frame()).queryByRole("group", { name: "claude 4 asks" })).toBeNull();
  });
});

describe("a long message", () => {
  /** A layout where every message is cut on its line: jsdom never lays one out. */
  function everyMessageCut() {
    const tall = vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(100);
    const short = vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(20);
    return () => {
      tall.mockRestore();
      short.mockRestore();
    };
  }

  it("has no toggle while it fits on its line", async () => {
    await aPane([removal("beta")]);
    await unfold();
    const row = await within(frame()).findByRole("status", { name: "Remove workspace" });
    expect(within(row).queryByRole("button", { name: "Show the whole message" })).toBeNull();
  });

  it("opens whole from its row, and Esc folds it and lands on the toggle", async () => {
    const restore = everyMessageCut();
    try {
      await aPane([removal("beta")]);
      await unfold();
      const row = await within(frame()).findByRole("status", { name: "Remove workspace" });
      const toggle = await within(row).findByRole("button", { name: "Show the whole message" });
      expect(toggle).toHaveAttribute("aria-expanded", "false");

      toggle.focus();
      await userEvent.keyboard("{Enter}");

      const opened = within(row).getByRole("button", { name: "Show less" });
      expect(opened).toHaveAttribute("aria-expanded", "true");
      expect(row.closest('[data-part="root"]')).toHaveAttribute("data-whole");

      // The tooltip the keyboard brought up takes the first Esc (`ui/tooltip`), the row the next.
      await userEvent.keyboard("{Escape}");
      await userEvent.keyboard("{Escape}");

      const back = within(row).getByRole("button", { name: "Show the whole message" });
      expect(back).toHaveFocus();
      // Esc folded it and put nothing away.
      expect(row).toBeInTheDocument();
    } finally {
      restore();
    }
  });
});
