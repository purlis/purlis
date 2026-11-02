// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render as renderBare,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import type { ChatSkills } from "./bindings";
import { onAMac } from "./tabKeys";
import { stripNamed } from "./test-strips";

/**
 * **Use skill…'s key** (#3954): ⌘⇧S (Ctrl+Shift+S off a Mac) opens Use skill… on the chat in
 * front, against the whole window, as its palette row does; never under a modal.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

const render = (ui: React.ReactElement) => renderBare(<StrictMode>{ui}</StrictMode>);

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const ALPHA = `${PLANE}/workspaces/alpha`;

const CHAT = {
  session: 3,
  name: "steward 3",
  cwd: ALPHA,
  harness: "claude",
  in_front: true,
  resumed: null,
  fresh: null,
  profile: null,
  persona: "steward",
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

const SKILLS: ChatSkills = {
  on: true,
  known: true,
  skills: [
    {
      name: "deploy",
      source_label: null,
      level: "project",
      label: "project",
      description: "Deploys.",
      manual: false,
      takes_arguments: false,
      arguments: [],
      hint: null,
      hidden: null,
    },
  ],
  budget: null,
  repos: [],
  not_supported: null,
  routing: null,
  missing: null,
};

/** The core: one chat in alpha, the Skill studio on. What the window asked, by command. */
function core() {
  const asked: string[] = [];
  mockIPC(
    (cmd) => {
      asked.push(cmd);
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "skill_studio_on") return true;
      if (cmd === "chat_skills") return SKILLS;
      if (cmd === "plane_sidebar")
        return {
          root: PLANE,
          personas: [],
          persona: null,
          unfiled: [],
          workspaces: [{ name: "alpha", path: ALPHA, vision: "", todos: [], chats: [{ ...CHAT }] }],
        };
      if (cmd === "chat_states") return [];
      if (cmd === "chats_that_would_not_start") return [];
      if (cmd === "running_sessions") return [];
      if (cmd === "about_charter") return new Promise(() => {});
      return null;
    },
    { shouldMockEvents: true },
  );
  return asked;
}

/** ⌘⇧S on a Mac, Ctrl+Shift+S elsewhere, on the window. */
function useSkillKey() {
  const mac = onAMac();
  fireEvent.keyDown(window, { key: "S", shiftKey: true, metaKey: mac, ctrlKey: !mac });
}

describe("Use skill…'s key (#3954)", () => {
  it("opens Use skill… on the chat in front, wherever the keyboard is", async () => {
    core();
    render(<App />);
    await waitFor(() =>
      expect(within(stripNamed("Tabs")).queryAllByRole("tab").length).toBeGreaterThan(0),
    );
    // The row is there once the core said the studio is on.
    await act(async () => {
      await new Promise((settle) => setTimeout(settle, 0));
    });
    useSkillKey();
    const picker = await screen.findByRole("dialog", { name: "Use skill" });
    // Its skills are read as it opens.
    expect(await within(picker).findByText("deploy")).toBeTruthy();
  });

  it("is not taken under a modal", async () => {
    core();
    render(<App />);
    await waitFor(() =>
      expect(within(stripNamed("Tabs")).queryAllByRole("tab").length).toBeGreaterThan(0),
    );
    fireEvent.keyDown(window, { key: "F2" });
    await screen.findByRole("dialog", { name: "Command palette" });
    useSkillKey();
    await act(async () => {
      await new Promise((settle) => setTimeout(settle, 50));
    });
    expect(screen.queryByRole("dialog", { name: "Use skill" })).toBeNull();
  });
});
