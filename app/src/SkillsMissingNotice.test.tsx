// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillsMissingNotice, missingSays } from "./SkillsMissingNotice";
import { CHAT_SKILLS_ASKED, type ChatSkillsAsk } from "./chatSkillsAsk";

afterEach(() => {
  cleanup();
  clearMocks();
});

const MISSING = {
  persona: "ops",
  missing: ["superpowers:tdd", "langfuse"],
  no_built_in: ["purlis:gone"],
  broken: [],
  by_a_repo: [],
  beside_a_built_in: [],
};

describe("a chat's missing skills", () => {
  it("names each, and shows this chat's skills in the Skills view (S8, #3694)", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      if (cmd === "chat_skills_missing") return MISSING;
      if (cmd === "put_away_chat_skills_missing") {
        asked.push(cmd);
        return null;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const ran: ChatSkillsAsk[] = [];
    const hear = (event: Event) => ran.push((event as CustomEvent<ChatSkillsAsk>).detail);
    window.addEventListener(CHAT_SKILLS_ASKED, hear);
    try {
      render(<SkillsMissingNotice plane="p" session={7} />);

      expect(await screen.findByText(/superpowers:tdd, langfuse/)).toBeInTheDocument();
      // One action: where the missing skills sit in Needs you with their fixes. The old
      // Open Skill studio is gone.
      expect(screen.queryByRole("button", { name: "Open Skill studio" })).toBeNull();
      fireEvent.click(screen.getByRole("button", { name: "Show in Skills" }));
      expect(ran).toEqual([{ plane: "p", session: 7 }]);
    } finally {
      window.removeEventListener(CHAT_SKILLS_ASKED, hear);
    }
  });

  it("says nothing for a chat that has every skill", async () => {
    let read = false;
    mockIPC((cmd) => {
      read = cmd === "chat_skills_missing";
      return null;
    });
    const { container } = render(<SkillsMissingNotice plane="p" session={7} />);
    await waitFor(() => expect(read).toBe(true));
    expect(container).toBeEmptyDOMElement();
  });

  it("tells a typo'd built-in skill apart from one not installed", () => {
    expect(missingSays("This chat", MISSING)).toBe(
      "This chat started without 2 skills ops names, not installed on this machine: " +
        "superpowers:tdd, langfuse. ops names 1 built-in skill purlis does not have: purlis:gone.",
    );
    expect(missingSays("This chat", { ...MISSING, missing: [] })).toBe(
      "ops names 1 built-in skill purlis does not have: purlis:gone.",
    );
  });

  it("says a skill that could not be loaded, with its level and why (SK-8)", () => {
    expect(
      missingSays("This chat", {
        persona: null,
        missing: [],
        no_built_in: [],
        broken: [
          {
            name: "half",
            level: "project",
            local: false,
            label: "project",
            why: "not a skill: its SKILL.md names no `description`",
          },
        ],
        by_a_repo: [],
        beside_a_built_in: [],
      }),
    ).toBe(
      "This chat started without 1 skill that could not be loaded: half (project), not a skill: its SKILL.md names no `description`.",
    );
  });

  it("says each skill the repo the chat works in leaves out, with why (SK-53)", () => {
    expect(
      missingSays("This chat", {
        persona: null,
        missing: [],
        no_built_in: [],
        broken: [],
        by_a_repo: [
          {
            name: "review",
            level: "workspace",
            local: false,
            label: "workspace",
            why: "the repo api has its own copy, which loads in its place",
          },
          {
            name: "ship",
            level: "workspace",
            local: false,
            label: "workspace",
            why: "purlis could not read the skills folder of web whole, so it is left out in case the repo has its own",
          },
        ],
        beside_a_built_in: [],
      }),
    ).toBe(
      "This chat started without 2 skills because of the repo it works in: " +
        "review (workspace), the repo api has its own copy, which loads in its place; " +
        "ship (workspace), purlis could not read the skills folder of web whole, so it is left " +
        "out in case the repo has its own.",
    );
  });

  it("names each skill of the repo the chat works in that is named like a built-in one (SK-37)", () => {
    const said = (names: string[]) =>
      missingSays("This chat", {
        persona: null,
        missing: [],
        no_built_in: [],
        broken: [],
        by_a_repo: [],
        beside_a_built_in: names,
      });
    expect(said(["secrets"])).toBe(
      "This chat works in a repo with its own copy of 1 built-in skill, which it gets beside " +
        "purlis's and may use instead: secrets.",
    );
    expect(said(["secrets", "browser"])).toBe(
      "This chat works in a repo with its own copy of 2 built-in skills, which it gets beside " +
        "purlis's and may use instead: secrets, browser.",
    );
  });
});
