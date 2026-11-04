// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A studio chat's link, as the window draws it** (#5185): the chip in its header, the line
 * above its skill's tabs while it runs, and the Runs tab's "Chats the studio started".
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReactNode } from "react";
import type { SkillDraft, StudioLink } from "./bindings";
import {
  StudioChatChip,
  StudioChatLines,
  StudioChatsHere,
  StudioChatsStarted,
  type StudioChatsLent,
} from "./studioChats";

afterEach(() => {
  cleanup();
  clearMocks();
});

const DEMO = { at: "workspace", name: "purlis-demo", local: false } as const;
const EVALS: StudioLink = { skill: "test-design", level: DEMO, job: "outcome-evals", done: false };

const draftOf = (chat: number): SkillDraft => ({
  id: "d1",
  name: "test-design",
  level: DEMO,
  chat,
  chat_open: true,
  new: false,
  stale: false,
  files: [],
});

/** What the window lends, with chat 7 writing test-design's outcome evals. */
function lent(): StudioChatsLent {
  return {
    chats: [{ session: 7, name: "chat 7", link: EVALS }],
    show: vi.fn(),
    openSkill: vi.fn(),
  };
}

function drawn(value: StudioChatsLent, children: ReactNode) {
  return render(<StudioChatsHere.Provider value={value}>{children}</StudioChatsHere.Provider>);
}

describe("a studio chat's chip (#5185)", () => {
  it("names the skill and the job, and opens the skill's page", async () => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? [] : null));
    const value = lent();
    drawn(value, <StudioChatChip plane="p" session={7} />);

    const chip = await screen.findByRole("button", { name: "test-design · writing outcome evals" });
    fireEvent.click(chip);

    expect(value.openSkill).toHaveBeenCalledWith(EVALS);
  });

  it("says what the job did once its draft waits", async () => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? [draftOf(7)] : null));
    drawn(lent(), <StudioChatChip plane="p" session={7} />);

    expect(
      await screen.findByRole("button", { name: "test-design · wrote outcome evals" }),
    ).toBeInTheDocument();
  });

  it("is not drawn on a chat the studio did not start", () => {
    mockIPC(() => []);
    drawn(lent(), <StudioChatChip plane="p" session={8} />);

    expect(screen.queryByTestId("pane-studio")).toBeNull();
  });
});

describe("a skill's page while a studio chat works on it (#5185)", () => {
  it("says the chat above the tabs while it runs, with Open chat", () => {
    const value = lent();
    drawn(value, <StudioChatLines name="test-design" level={DEMO} drafts={[]} />);

    expect(screen.getByTestId("studio-chat-running-7")).toHaveTextContent(
      "A chat at purlis-demo is writing outcome evals · running",
    );
    fireEvent.click(screen.getByRole("button", { name: "Open chat" }));
    expect(value.show).toHaveBeenCalledWith(7);
  });

  it("says nothing above the tabs once its draft waits, or for another skill's chat", () => {
    drawn(lent(), <StudioChatLines name="test-design" level={DEMO} drafts={[draftOf(7)]} />);
    expect(screen.queryByTestId("studio-chat-running-7")).toBeNull();
    cleanup();

    drawn(lent(), <StudioChatLines name="deploy" level={DEMO} drafts={[]} />);
    expect(screen.queryByTestId("skill-studio-chats-running")).toBeNull();
  });

  it("keeps the chat on Runs once it is done, with where its draft waits and Open chat", () => {
    const value = lent();
    drawn(value, <StudioChatsStarted name="test-design" level={DEMO} drafts={[draftOf(7)]} />);

    const started = screen.getByRole("region", { name: "Chats the studio started" });
    const row = within(started).getByTestId("studio-chat-7");
    expect(row).toHaveTextContent("Wrote outcome evals");
    expect(row).toHaveTextContent("Finished");
    expect(row).toHaveTextContent("chat 7, at purlis-demo · its draft waits in Drafts");
    fireEvent.click(within(row).getByRole("button", { name: "Open chat" }));
    expect(value.show).toHaveBeenCalledWith(7);
  });
});
