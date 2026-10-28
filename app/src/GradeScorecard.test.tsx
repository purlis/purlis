// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import {
  GradeChatTab,
  GradeScorecardTab,
  gradeChatOfView,
  gradeChatView,
  gradeOfView,
  gradeView,
  runsSaid,
} from "./GradeScorecard";
import type { GradeScorecard } from "./bindings";
import type { ViewRef } from "./tabs";

afterEach(() => {
  cleanup();
  clearMocks();
});

// jsdom has no `matchMedia`, and xterm asks it for the display's pixel ratio as it opens.
vi.stubGlobal("matchMedia", (query: string) => ({
  matches: false,
  media: query,
  addEventListener: () => {},
  removeEventListener: () => {},
  addListener: () => {},
  removeListener: () => {},
}));

const CARD: GradeScorecard = {
  grade: "01GRADE",
  skill: "deploy",
  graded: "2026-10-28 10:00 UTC",
  version: "0123456789ab",
  ran_on: "Claude Code · claude-sonnet-4-5",
  ended: "Finished",
  delta: 33,
  asked: 1,
  tests: [
    {
      query: "ship it to production",
      should_trigger: true,
      arms: [
        {
          arm: "with",
          runs: 3,
          triggered: 2,
          pass: true,
          asked: 0,
          chats: [
            { id: "01A", kept: true },
            { id: "01B", kept: false },
            { id: "01C", kept: true },
          ],
        },
        {
          arm: "without",
          runs: 2,
          triggered: 0,
          pass: false,
          asked: 1,
          chats: [
            { id: "01D", kept: true },
            { id: "01E", kept: true },
            { id: "01F", kept: true },
          ],
        },
      ],
    },
  ],
};

describe("A grade's scorecard (SK-59)", () => {
  it("keys its tabs by the skill, the grade and the chat, and reads them back", () => {
    expect(gradeOfView(gradeView("deploy", "01G"))).toEqual({ skill: "deploy", grade: "01G" });
    expect(gradeChatOfView(gradeChatView("deploy", "01G", "01C"))).toEqual({
      skill: "deploy",
      grade: "01G",
      chat: "01C",
    });
    expect(gradeOfView({ from: null, view: "skill-grade", key: "deploy" })).toBeUndefined();
    expect(gradeOfView(gradeChatView("deploy", "01G", "01C"))).toBeUndefined();
    expect(
      gradeChatOfView({ from: "ext", view: "skill-grade-chat", key: "a@b@c" }),
    ).toBeUndefined();
  });

  it("says each arm's runs, its verdict and the chats that asked you something", () => {
    expect(runsSaid(CARD.tests[0].arms[0], true)).toBe(
      "2 of 3 chats started it (should start it), passed",
    );
    expect(runsSaid(CARD.tests[0].arms[1], true)).toBe(
      "0 of 2 chats started it (should start it), failed; 1 chat asked you something and was ended there, not counted",
    );
  });

  it("opens each kept chat in one click, and says which are not kept here", async () => {
    mockIPC((cmd) => (cmd === "skill_grade_scorecard" ? CARD : null));
    const opened: Array<{ view: ViewRef; title: string }> = [];
    render(
      <GradeScorecardTab
        plane="p"
        skill="deploy"
        grade="01GRADE"
        onOpenView={(view, title) => opened.push({ view, title })}
      />,
    );
    const about = await screen.findByTestId("grade-scorecard-about");
    expect(about.textContent).toContain("on Claude Code · claude-sonnet-4-5");
    expect(about.textContent).toContain("a difference of +33 points");
    expect(screen.getByTestId("grade-scorecard-asked").textContent).toContain(
      "1 chat asked you something",
    );
    const with_ = screen.getByTestId("grade-scorecard-with");
    const buttons = with_.querySelectorAll("button");
    expect(buttons).toHaveLength(3);
    // A disabled button with a hint keeps its focus: it is marked, not removed.
    expect(buttons[1].getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(buttons[2]);
    expect(opened).toEqual([
      { view: gradeChatView("deploy", "01GRADE", "01C"), title: "Grade chat 3 · deploy" },
    ]);
  });

  it("says why a grade it cannot read is not shown", async () => {
    mockIPC(() => {
      throw "That grade of deploy is not in the record any more.";
    });
    render(<GradeScorecardTab plane="p" skill="deploy" grade="01GONE" />);
    expect((await screen.findByTestId("grade-scorecard-refused")).textContent).toContain(
      "not in the record any more",
    );
  });
});

describe("A grade's chat, read-only (SK-59)", () => {
  it("draws what the chat showed, and takes no input", async () => {
    const asked: Array<{ cmd: string; args: unknown }> = [];
    mockIPC((cmd, args) => {
      asked.push({ cmd, args });
      return cmd === "skill_grade_chat"
        ? "> ship it to production\r\nUsing the deploy skill"
        : null;
    });
    render(<GradeChatTab plane="p" skill="deploy" grade="01GRADE" chat="01C" />);
    await screen.findByTestId("grade-chat");
    expect(asked[0]).toMatchObject({
      cmd: "skill_grade_chat",
      args: { plane: "p", skill: "deploy", grade: "01GRADE", chat: "01C" },
    });
    expect(screen.getByText(/It takes no input/)).toBeTruthy();
  });

  it("says a chat this machine did not keep is not kept here", async () => {
    mockIPC(() => {
      throw "purlis keeps a grade's chats on the machine that ran it.";
    });
    render(<GradeChatTab plane="p" skill="deploy" grade="01GRADE" chat="01B" />);
    expect((await screen.findByTestId("grade-chat-refused")).textContent).toContain(
      "on the machine that ran it",
    );
  });
});
