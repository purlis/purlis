// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A studio chat's link, as the window draws it** (#5185, #5248): the chip on its pane foot, the
 * line above its skill's tabs while it runs, the Runs tab's "Chats the studio started", open and
 * closed, and the Grades card's running job.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { ReactNode } from "react";
import type { ClosedStudioChat, SkillDraft, StudioLink } from "./bindings";
import {
  RunningJob,
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
const EVALS: StudioLink = {
  skill: "test-design",
  level: DEMO,
  job: "outcome-evals",
  done: false,
  started: null,
  finished: null,
};

/** Chat 5, closed: it wrote test-design's checks, finished on 4 November at 17:42, local time. */
const CLOSED: ClosedStudioChat = {
  session: 5,
  name: "checks chat",
  closed: new Date(2026, 10, 4, 18, 0).toISOString(),
  link: {
    ...EVALS,
    job: "checks",
    done: true,
    finished: new Date(2026, 10, 4, 17, 42).toISOString(),
  },
};

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
    openDraft: vi.fn(),
    stop: vi.fn(),
  };
}

function drawn(value: StudioChatsLent, children: ReactNode) {
  return render(<StudioChatsHere.Provider value={value}>{children}</StudioChatsHere.Provider>);
}

describe("a studio chat's chip (#5185)", () => {
  it("names the skill and the job, and runs its row on a press", async () => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? [] : null));
    const value = lent();
    const open = vi.fn();
    drawn(value, <StudioChatChip plane="p" session={7} onOpen={open} />);

    const chip = await screen.findByRole("button", { name: "test-design · writing outcome evals" });
    fireEvent.click(chip);

    expect(open).toHaveBeenCalled();
  });

  it("says what the job did once its draft waits", async () => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? [draftOf(7)] : null));
    drawn(lent(), <StudioChatChip plane="p" session={7} onOpen={vi.fn()} />);

    expect(
      await screen.findByRole("button", { name: "test-design · wrote outcome evals" }),
    ).toBeInTheDocument();
  });

  it("is its words alone with no row to run: nothing to press that does nothing", () => {
    mockIPC((cmd) => (cmd === "skill_drafts" ? [] : null));
    drawn(lent(), <StudioChatChip plane="p" session={7} />);

    expect(screen.getByTestId("pane-studio").tagName).toBe("SPAN");
    expect(screen.queryByRole("button")).toBeNull();
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
    drawn(value, <StudioChatLines plane="/p/purlis" name="test-design" level={DEMO} drafts={[]} />);

    expect(screen.getByTestId("studio-chat-running-7")).toHaveTextContent(
      "A chat at purlis-demo is writing outcome evals · running",
    );
    fireEvent.click(screen.getByRole("button", { name: "Open chat" }));
    expect(value.show).toHaveBeenCalledWith(7);
  });

  it("says nothing above the tabs once its draft waits, or for another skill's chat", () => {
    drawn(
      lent(),
      <StudioChatLines plane="/p/purlis" name="test-design" level={DEMO} drafts={[draftOf(7)]} />,
    );
    expect(screen.queryByTestId("studio-chat-running-7")).toBeNull();
    cleanup();

    drawn(lent(), <StudioChatLines plane="/p/purlis" name="deploy" level={DEMO} drafts={[]} />);
    expect(screen.queryByTestId("skill-studio-chats-running")).toBeNull();
  });

  it("keeps the chat on Runs once it is done, with where its draft waits and Open chat", () => {
    mockIPC((cmd) => (cmd === "studio_chats_closed" ? [] : null));
    const value = lent();
    drawn(
      value,
      <StudioChatsStarted
        plane="/p/purlis"
        name="test-design"
        level={DEMO}
        drafts={[draftOf(7)]}
      />,
    );

    const started = screen.getByRole("region", { name: "Chats the studio started" });
    const row = within(started).getByTestId("studio-chat-7");
    expect(row).toHaveTextContent("Wrote outcome evals");
    expect(row).toHaveTextContent("Finished");
    expect(row).toHaveTextContent("chat 7, at purlis-demo · its draft waits in Drafts");
    fireEvent.click(within(row).getByRole("button", { name: "Open chat" }));
    expect(value.show).toHaveBeenCalledWith(7);
  });

  it("opens the draft a finished chat wrote, from its row on Runs (#5248)", () => {
    mockIPC((cmd) => (cmd === "studio_chats_closed" ? [] : null));
    const value = lent();
    drawn(
      value,
      <StudioChatsStarted
        plane="/p/purlis"
        name="test-design"
        level={DEMO}
        drafts={[draftOf(7)]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Review draft" }));

    expect(value.openDraft).toHaveBeenCalledWith(EVALS, "d1");
  });

  it("keeps a chat on Runs once it has closed, with when it finished and no Open chat (#5248)", async () => {
    mockIPC((cmd) => (cmd === "studio_chats_closed" ? [CLOSED] : null));
    const value = lent();
    drawn(
      value,
      <StudioChatsStarted
        plane="/p/purlis"
        name="test-design"
        level={DEMO}
        drafts={[draftOf(5)]}
      />,
    );

    const row = await screen.findByTestId("studio-chat-5");
    expect(row).toHaveTextContent("Wrote checks");
    expect(row).toHaveTextContent("Finished");
    expect(row).toHaveTextContent(
      "checks chat, at purlis-demo · finished 11-04 17:42 · its draft waits in Drafts",
    );
    expect(within(row).queryByRole("button", { name: "Open chat" })).toBeNull();
    fireEvent.click(within(row).getByRole("button", { name: "Review draft" }));
    expect(value.openDraft).toHaveBeenCalledWith(CLOSED.link, "d1");
    // The open chat is listed first.
    const rows = within(screen.getByRole("grid")).getAllByRole("row");
    expect(rows.map((one) => one.dataset.testid)).toEqual(["studio-chat-7", "studio-chat-5"]);
  });

  it("lists on Runs a closed chat of this skill alone, and none still open (#5248)", async () => {
    const other = { ...CLOSED, session: 6, link: { ...CLOSED.link, skill: "deploy" } };
    const reopened = { ...CLOSED, session: 7 };
    mockIPC((cmd) => (cmd === "studio_chats_closed" ? [other, reopened, CLOSED] : null));
    drawn(
      lent(),
      <StudioChatsStarted plane="/p/purlis" name="test-design" level={DEMO} drafts={[]} />,
    );

    await screen.findByTestId("studio-chat-5");
    expect(screen.queryByTestId("studio-chat-6")).toBeNull();
    expect(screen.getAllByTestId("studio-chat-7")).toHaveLength(1);
  });
});

describe("the Grades card's running job (#5185, #5248)", () => {
  it("says how long the chat has run, with Open chat and Stop it", () => {
    const value = lent();
    const started = new Date(Date.now() - 2 * 60_000 - 5_000).toISOString();
    const chat = { session: 7, name: "chat 7", link: { ...EVALS, started } };
    drawn(value, <RunningJob plane="/p/purlis" chat={chat} />);

    expect(screen.getByTestId("skill-checks-running-7")).toHaveTextContent(
      "A chat at purlis-demo is writing outcome evals · running 2 min",
    );
    fireEvent.click(screen.getByRole("button", { name: "Stop it" }));
    expect(value.stop).toHaveBeenCalledWith(7);
    fireEvent.click(screen.getByRole("button", { name: "Open chat" }));
    expect(value.show).toHaveBeenCalledWith(7);
  });
});
