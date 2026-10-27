// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { catalogue, type Now } from "../actions";
import { noTabs } from "../tabs";
import { skillsSummary, workspaceSkillsGroup } from "./skillsSummary";

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("the Workspace level's Skills summary (#2767)", () => {
  it("counts what the workspace's chats get and what is left out", () => {
    expect(skillsSummary(0, 0)).toBe("No skills reach this workspace's chats yet.");
    expect(skillsSummary(1, 0)).toBe("Its chats get 1 skill.");
    expect(skillsSummary(3, 1)).toBe(
      "Its chats get 3 skills, and 1 more is left out, each with why in the studio.",
    );
  });

  it("links to the catalogue row that opens the studio filtered to the workspace", () => {
    const group = workspaceSkillsGroup("p", "alpha");
    expect(group.id).toBe("workspace.skills");
    expect(group.links).toEqual([{ label: "Open in Skill studio", action: "skills.studio:alpha" }]);

    const now = (skillStudio: boolean): Now => ({
      tabs: noTabs(),
      workspaces: ["alpha"],
      focused: "alpha",
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      skillStudio,
    });
    const row = catalogue(now(true)).find((offer) => offer.id === "skills.studio:alpha");
    expect(row?.does).toEqual({
      verb: "openView",
      view: { from: null, view: "skill-studio", key: "alpha" },
      title: "Skill studio · alpha",
    });
    const ids = catalogue(now(false)).map((offer) => offer.id);
    expect(ids).not.toContain("skills.studio:alpha");
    expect(ids).not.toContain("settings.group:workspace.skills:alpha");
    expect(catalogue(now(true)).map((offer) => offer.id)).toContain(
      "settings.group:workspace.skills:alpha",
    );
  });
});
