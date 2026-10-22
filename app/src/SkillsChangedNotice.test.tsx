// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SKILLS_CHANGED, SkillsChangedNotice } from "./SkillsChangedNotice";

afterEach(() => {
  cleanup();
  clearMocks();
});

function core(updated: unknown[]) {
  mockIPC((cmd) => {
    if (cmd === "skill_studio_on") return true;
    if (cmd === "chats_plane_updated") return updated;
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("a chat whose skills changed since it started", () => {
  it("is told so and offered a restart on the same conversation", async () => {
    core([{ session: 7, files: [], skills: true }]);
    const onRestart = vi.fn();
    render(<SkillsChangedNotice plane="p" session={7} onRestart={onRestart} />);

    const notice = await screen.findByRole("status", { name: /Skills changed/ });
    expect(notice).toHaveTextContent(SKILLS_CHANGED);
    await userEvent.setup().click(within(notice).getByRole("button", { name: "Restart" }));
    expect(onRestart).toHaveBeenCalledOnce();
  });

  it("says nothing to a chat whose instructions alone changed", async () => {
    core([{ session: 7, files: ["CLAUDE.md"], skills: false }]);
    render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
