// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { SkillHidden } from "./bindings";
import { inWorkspace } from "./skillStudio";
import { skill, studio } from "./skillsListing.testkit";
import { forgetStudio, openSkill, renderStudio } from "./SkillStudioTab.testkit";

const OVER: SkillHidden = {
  router: "built-in",
  why: "over-budget",
  said: "hidden by the built-in router: over this chat's skill budget, so it is in the index skill",
};

/** smart-ide, whose clone purlis holds 12 repo skills: `changelog` is listed at the root and
 *  hidden on Claude Code in purlis. */
const READ = studio(
  [
    skill("changelog", {
      hidden_in: [{ clone: "purlis", harness: "claude", title: "Claude Code", hidden: OVER }],
    }),
    skill("deploy"),
  ],
  {
    workspace: "smart-ide",
    workspaces: ["smart-ide"],
    places: [
      {
        clone: "purlis",
        repo_skills: 12,
        preview: "A chat started in purlis.",
      },
    ],
  },
);

/** Every `skill_studio` read the studio asks for. */
function answering(): unknown[] {
  const asked: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "skills_deleted") return [];
    if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
    asked.push(args);
    return READ;
  });
  return asked;
}

afterEach(() => {
  cleanup();
  clearMocks();
  forgetStudio();
});

describe("the Skill studio previews a chat started in a clone (#4227)", () => {
  it("marks hidden what a chat started in the chosen clone hides, without reading again", async () => {
    const asked = answering();
    renderStudio(inWorkspace("smart-ide"));
    const user = userEvent.setup();

    const row = await screen.findByTestId("skill-changelog");
    expect(within(row).queryByText("Hidden by the router")).toBeNull();

    await user.click(screen.getByRole("button", { name: "A chat in smart-ide · at its root" }));
    const menu = await screen.findByRole("menu", { name: "Whose chats and where they start" });
    expect(within(menu).getByText("Its 12 repo skills count too.")).toBeInTheDocument();
    await user.click(within(menu).getByRole("menuitemradio", { name: /In purlis/ }));

    await waitFor(() =>
      expect(
        within(screen.getByTestId("skill-changelog")).getByText("Hidden by the router"),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByRole("button", { name: "A chat in smart-ide · in purlis" }),
    ).toBeInTheDocument();
    // The one read covers every place.
    expect(asked).toHaveLength(1);

    // Its page says where, and lists each place.
    const page = await openSkill("skill-changelog", user);
    expect(within(page).getByTestId("skill-banner-hidden")).toHaveTextContent(
      "Hidden by the router on Claude Code for a chat started in purlis",
    );
    expect(within(page).getByTestId("skill-banner-hidden")).toHaveTextContent(
      "A chat at the workspace's root lists it: purlis's 12 repo skills push it out.",
    );
    expect(within(page).getByTestId("skill-in-root")).toHaveTextContent("Listed");
    expect(within(page).getByTestId("skill-in-purlis")).toHaveTextContent(
      "Hidden by the router on Claude Code",
    );
  });

  it("offers no place where no clone holds repo skills: the picker is the select it was", async () => {
    mockIPC((cmd) => {
      if (cmd === "skills_deleted") return [];
      if (cmd === "skill_studio") return { ...READ, places: [] };
      throw new Error(`unexpected ${cmd}`);
    });
    renderStudio(inWorkspace("smart-ide"));
    await screen.findByTestId("skill-changelog");
    expect(screen.getByRole("button", { name: /Whose chats/ })).toBeInTheDocument();
    expect(screen.queryByText("Where it starts")).toBeNull();
  });
});
