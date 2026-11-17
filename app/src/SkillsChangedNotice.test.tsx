// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { emit } from "@tauri-apps/api/event";
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

function core(updated: PlaneUpdated[] | (() => PlaneUpdated[])) {
  let asked = 0;
  mockIPC(
    (cmd) => {
      if (cmd === "chats_plane_updated") {
        asked += 1;
        return typeof updated === "function" ? updated() : updated;
      }
      throw new Error(`unexpected ${cmd}`);
    },
    { shouldMockEvents: true },
  );
  return () => asked;
}

/** The plane changed on disk, in a way that concerns what chats started with. */
async function planeChanged() {
  await act(async () => {
    await emit("plane-changed", { plane: "p" });
    await new Promise((resolve) => setTimeout(resolve, 0));
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

  it("shows its offer once for a bulk change, however many changes on disk it was (#5331)", async () => {
    const one = { ...UPDATED, skills: true, skills_gone: ["grilling", "tdd"] };
    let now: PlaneUpdated[] = [one];
    const asked = core(() => now);
    render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);
    const notice = await screen.findByRole("status", { name: /Skills changed/ });
    await userEvent.setup().click(within(notice).getByRole("button", { name: /Dismiss/ }));
    expect(screen.queryByRole("status", { name: /Skills changed/ })).toBeNull();

    // More events from the same change: the same skills gone, and it stays away.
    const before = asked();
    await planeChanged();
    await waitFor(() => expect(asked()).toBeGreaterThan(before));
    await planeChanged();
    expect(screen.queryByRole("status", { name: /Skills changed/ })).toBeNull();

    // Another change: it is offered again.
    now = [{ ...one, skills_gone: ["grilling", "tdd", "review"] }];
    await planeChanged();
    expect(await screen.findByRole("status", { name: /Skills changed/ })).toBeInTheDocument();
  });

  it("offers again when a change is undone and then made again (#5331)", async () => {
    const one = { ...UPDATED, skills: true, skills_gone: ["grilling"] };
    let now: PlaneUpdated[] = [one];
    core(() => now);
    render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);
    const notice = await screen.findByRole("status", { name: /Skills changed/ });
    await userEvent.setup().click(within(notice).getByRole("button", { name: /Dismiss/ }));
    now = [];
    await planeChanged();
    now = [one];
    await planeChanged();
    expect(await screen.findByRole("status", { name: /Skills changed/ })).toBeInTheDocument();
  });

  it("says nothing to a chat whose instructions alone changed", async () => {
    core([{ ...UPDATED, files: ["CLAUDE.md"] }]);
    render(<SkillsChangedNotice plane="p" session={7} onRestart={() => undefined} />);

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
