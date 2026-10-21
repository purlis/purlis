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
});
