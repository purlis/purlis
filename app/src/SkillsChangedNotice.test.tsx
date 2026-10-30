// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PlaneUpdated } from "./bindings";
import { CHAT_SKILLS_ASKED, type ChatSkillsAsk } from "./chatSkillsAsk";
import { SKILLS_CHANGED, SkillsChangedNotice, WHAT_CHANGED } from "./SkillsChangedNotice";

afterEach(() => {
  cleanup();
  clearMocks();
});

const UPDATED: PlaneUpdated = {
  session: 7,
  files: [],
  skills: false,
  skills_added: [],
  skills_gone: [],
  skills_edited: [],
};

function core(updated: PlaneUpdated[]) {
  mockIPC((cmd) => {
    if (cmd === "skill_studio_on") return true;
    if (cmd === "chats_plane_updated") return updated;
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("a chat whose skills changed since it started", () => {
  it("is told so and offered a restart on the same conversation", async () => {
    core([{ ...UPDATED, skills: true }]);
    const onRestart = vi.fn();
    render(<SkillsChangedNotice plane="p" session={7} onRestart={onRestart} />);

    const notice = await screen.findByRole("status", { name: /Skills changed/ });
    expect(notice).toHaveTextContent(SKILLS_CHANGED);
    await userEvent.setup().click(within(notice).getByRole("button", { name: "Restart" }));
    expect(onRestart).toHaveBeenCalledOnce();
  });

  it("offers What changed, which shows this chat's skills in the Skills view (S8, #3694)", async () => {
    core([{ ...UPDATED, skills: true, skills_added: ["triage"] }]);
    const ran: ChatSkillsAsk[] = [];
    const hear = (event: Event) => ran.push((event as CustomEvent<ChatSkillsAsk>).detail);
    window.addEventListener(CHAT_SKILLS_ASKED, hear);
    try {
      render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);
      const notice = await screen.findByRole("status", { name: /Skills changed/ });
      await userEvent.setup().click(within(notice).getByRole("button", { name: WHAT_CHANGED }));
      expect(ran).toEqual([{ plane: "p", session: 7 }]);
    } finally {
      window.removeEventListener(CHAT_SKILLS_ASKED, hear);
    }
  });

  it("says nothing to a chat whose instructions alone changed", async () => {
    core([{ ...UPDATED, files: ["CLAUDE.md"] }]);
    render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
