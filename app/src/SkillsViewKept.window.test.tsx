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
import { askChatSkills } from "./chatSkillsAsk";
import { skill, studio } from "./skillsListing.testkit";
import { OPEN_SKILL_STUDIO, SKILL_STUDIO_TITLE } from "./skillStudio";
import { stripNamed } from "./test-strips";

/**
 * **The Skills view while a tab that is not a chat is in front** (#4351; ADR 0102, amended
 * 2026-11-02): it keeps the set of the chat last in front, and reads nothing again.
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
  program: [],
  plugins_off: [],
  own: null,
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

/** The core: one chat in alpha, the Skill studio on. Who each read of the studio was for. */
function core() {
  const studioFor: (string | null)[] = [];
  mockIPC(
    (cmd, args) => {
      if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
      if (cmd === "opened_chats") return [CHAT];
      if (cmd === "skill_studio_on") return true;
      if (cmd === "chat_skills") return SKILLS;
      if (cmd === "skill_studio") {
        studioFor.push((args as { persona: string | null }).persona);
        return studio([skill("deploy")]);
      }
      if (cmd === "skill_persona_skills" || cmd === "chat_branch_folder_skills") return [];
      if (cmd === "chats_plane_updated") return [];
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
  return studioFor;
}

const settle = () =>
  act(async () => {
    await new Promise((done) => setTimeout(done, 0));
  });

describe("the Skills view while a tab that is not a chat is in front (#4351)", () => {
  it("keeps the set of the chat last in front, and reads nothing again", async () => {
    const studioFor = core();
    render(<App />);
    await waitFor(() =>
      expect(within(stripNamed("Tabs")).queryAllByRole("tab").length).toBeGreaterThan(0),
    );
    await settle();
    act(() => askChatSkills(PLANE, 3));
    const view = await screen.findByTestId("skills-view");
    await waitFor(() =>
      expect(within(view).getByTestId("skills-view-whose")).toHaveTextContent("steward 3"),
    );
    const readBefore = studioFor.length;

    fireEvent.click(await within(view).findByRole("button", { name: OPEN_SKILL_STUDIO }));
    await waitFor(() =>
      expect(
        within(stripNamed("Tabs")).getByRole("tab", { name: new RegExp(SKILL_STUDIO_TITLE) }),
      ).toHaveAttribute("aria-selected", "true"),
    );
    await settle();

    expect(within(view).getByTestId("skills-view-whose")).toHaveTextContent("steward 3");
    // The studio reads for itself, as no persona; the view reads its chat's set no more.
    expect(studioFor.slice(readBefore)).not.toContain("steward");
  });
});
