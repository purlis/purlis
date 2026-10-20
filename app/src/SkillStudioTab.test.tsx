// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
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
      skills: [
        {
          name: "deploy",
          level: "project",
          on: true,
          left_out: null,
          description: "Deploys the site.",
          folder: "skills/deploy",
        },
        {
          name: "secrets",
          level: "built-in",
          on: true,
          left_out: null,
          description: "Keeps secrets.",
          folder: "/app/plugin/skills/secrets",
        },
        {
          name: "review",
          level: "project",
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

  it("says it is not on while its flag is off", async () => {
    answering({ on: false, skills: [] });
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
