// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { SkillStudioTab } from "./SkillStudioTab";
import { catalogue, SKILL_STUDIO_SHOW, type Now } from "./actions";
import { noTabs } from "./tabs";

afterEach(() => {
  cleanup();
  clearMocks();
});

function answering(studio: unknown) {
  mockIPC((cmd) => {
    if (cmd === "skill_studio") return studio;
    throw new Error(`unexpected ${cmd}`);
  });
}

describe("the Skill studio", () => {
  it("lists each skill with its level, and why one is left out", async () => {
    answering({
      on: true,
      workspace: null,
      workspaces: [],
      skills: [
        {
          name: "deploy",
          level: "project",
          local: false,
          grant: "root",
          on: true,
          left_out: null,
          description: "Deploys the site.",
          folder: "skills/deploy",
        },
        {
          name: "secrets",
          level: "built-in",
          local: false,
          grant: null,
          on: true,
          left_out: null,
          description: "Keeps secrets.",
          folder: "/app/plugin/skills/secrets",
        },
        {
          name: "review",
          level: "project",
          local: false,
          grant: "root",
          on: false,
          left_out: "a built-in skill has this name, and a built-in skill is never replaced",
          description: "",
          folder: "skills/review",
        },
      ],
    });

    render(<SkillStudioTab plane="p" changed={0} />);

    const deploy = await screen.findByTestId("skill-deploy");
    expect(within(deploy).getByText("project")).toBeInTheDocument();
    expect(within(deploy).getByText("On")).toBeInTheDocument();
    expect(within(screen.getByTestId("skill-secrets")).getByText("built-in")).toBeInTheDocument();
    expect(
      within(screen.getByTestId("skill-review")).getByText(
        "Left out: a built-in skill has this name, and a built-in skill is never replaced",
      ),
    ).toBeInTheDocument();
  });

  it("shows a workspace's chats, its own copy over the project's, and both copies", async () => {
    const asked: unknown[] = [];
    mockIPC((cmd, args) => {
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      asked.push(args);
      return {
        on: true,
        workspace: "alpha",
        workspaces: ["alpha", "beta"],
        skills: [
          {
            name: "lint",
            level: "workspace",
            local: false,
            grant: null,
            on: true,
            left_out: null,
            description: "Lints here.",
            folder: "workspaces/alpha/skills/lint",
          },
          {
            name: "lint",
            level: "project",
            local: false,
            grant: "all",
            on: false,
            left_out: "hidden by the workspace skill of this name",
            description: "",
            folder: "skills/lint",
          },
        ],
      };
    });

    render(<SkillStudioTab plane="p" workspace="alpha" changed={0} />);

    const mine = await screen.findByTestId("skill-lint");
    expect(within(mine).getByText("workspace")).toBeInTheDocument();
    const hidden = screen.getByTestId("skill-lint-hidden-1");
    expect(within(hidden).getByText("all")).toBeInTheDocument();
    expect(
      within(hidden).getByText("Left out: hidden by the workspace skill of this name"),
    ).toBeInTheDocument();
    expect(screen.getByText(/Every skill a chat in alpha gets/)).toBeInTheDocument();
    expect(asked[0]).toMatchObject({ plane: "p", workspace: "alpha" });
  });

  it("shares a local skill and reads the project again", async () => {
    let shared = false;
    const calls: string[] = [];
    mockIPC((cmd, args) => {
      calls.push(cmd);
      if (cmd === "skill_share") {
        expect(args).toMatchObject({ plane: "p", workspace: null, name: "scratch" });
        shared = true;
        return null;
      }
      if (cmd !== "skill_studio") throw new Error(`unexpected ${cmd}`);
      return {
        on: true,
        workspace: null,
        workspaces: [],
        skills: [
          {
            name: "scratch",
            level: "project",
            local: !shared,
            grant: "root",
            on: true,
            left_out: null,
            description: "Tries it.",
            folder: shared ? "skills/scratch" : "skills.local/scratch",
          },
        ],
      };
    });

    render(<SkillStudioTab plane="p" changed={0} />);
    const row = await screen.findByTestId("skill-scratch");
    expect(within(row).getByText(/project, local/)).toBeInTheDocument();
    within(row).getByRole("button", { name: "Share" }).click();

    await waitFor(() =>
      expect(within(screen.getByTestId("skill-scratch")).queryByRole("button")).toBeNull(),
    );
    expect(calls.filter((cmd) => cmd === "skill_studio")).toHaveLength(2);
  });

  it("says it is not on while its flag is off", async () => {
    answering({ on: false, workspace: null, workspaces: [], skills: [] });
    render(<SkillStudioTab plane="p" changed={0} />);
    expect(await screen.findByTestId("skill-studio-off")).toBeInTheDocument();
  });

  it("is offered in the palette only while its flag is on", () => {
    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      skillStudio,
    });
    const ids = (on: boolean) => catalogue(now(on)).map((offer) => offer.id);
    expect(ids(true)).toContain(SKILL_STUDIO_SHOW);
    expect(ids(false)).not.toContain(SKILL_STUDIO_SHOW);
  });
});
