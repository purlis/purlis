// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillsMissingNotice, missingSays } from "./SkillsMissingNotice";
import { SETTINGS_ACTION, type SettingsActionAsk } from "./settings/links";

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
  it("names each, and opens the Skill studio on the persona", async () => {
    const asked: string[] = [];
    mockIPC((cmd) => {
      if (cmd === "chat_skills_missing") return MISSING;
      if (cmd === "put_away_chat_skills_missing") {
        asked.push(cmd);
        return null;
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const ran: SettingsActionAsk[] = [];
    const hear = (event: Event) => ran.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, hear);
    try {
      render(<SkillsMissingNotice plane="p" session={7} />);

      expect(await screen.findByText(/superpowers:tdd, langfuse/)).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Open in Skill studio" }));
      expect(ran).toEqual([{ plane: "p", action: "skills.studio:persona/ops" }]);
    } finally {
      window.removeEventListener(SETTINGS_ACTION, hear);
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
            why: "hidden by the repo skill of this name",
          },
          {
            name: "ship",
            level: "workspace",
            local: false,
            label: "workspace",
            why: "purlis could not read the skills folder of web whole, so a copy of this skill may be there, and a chat in that repo would get that one",
          },
        ],
        beside_a_built_in: [],
      }),
    ).toBe(
      "This chat started without 2 skills because of the repo it works in: " +
        "review (workspace), hidden by the repo skill of this name; " +
        "ship (workspace), purlis could not read the skills folder of web whole, so a copy of " +
        "this skill may be there, and a chat in that repo would get that one.",
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

  it("opens the Skill studio on a chat at the project's root for a chat on no persona", async () => {
    mockIPC((cmd) => {
      if (cmd === "chat_skills_missing")
        return {
          persona: null,
          missing: [],
          no_built_in: [],
          broken: [
            { name: "half", level: "project", local: false, label: "project", why: "not a skill" },
          ],
          by_a_repo: [],
          beside_a_built_in: [],
        };
      return null;
    });
    const ran: SettingsActionAsk[] = [];
    const hear = (event: Event) => ran.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, hear);
    try {
      render(<SkillsMissingNotice plane="p" session={7} />);
      expect(await screen.findByText(/could not be loaded: half/)).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Open in Skill studio" }));
      expect(ran).toEqual([{ plane: "p", action: "skills.studio" }]);
    } finally {
      window.removeEventListener(SETTINGS_ACTION, hear);
    }
  });
});
