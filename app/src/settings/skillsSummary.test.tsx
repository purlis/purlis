// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { LiveSetting } from "./groups";
import { SETTINGS_ACTION, type SettingsActionAsk } from "./links";
import { need, skill, studio } from "../skillsListing.testkit";
import { catalogue, type Now } from "../actions";
import { noTabs } from "../tabs";
import { skillsSummary, workspaceSkillsGroup } from "./skillsSummary";
import { asPersona, inWorkspace, skillStudioAt, skillStudioView } from "../skillStudio";

/** The group's one line, drawn as its Settings row draws its control. */
function Line({ setting }: { setting: LiveSetting }) {
  return <>{setting.useControl().control({ id: "summary", describedBy: "help" })}</>;
}

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
    expect(group.links).toEqual([{ label: "Open Skill studio", action: "skills.studio:alpha" }]);

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
      view: skillStudioView(inWorkspace("alpha")),
      title: "Skill studio",
    });
    const ids = catalogue(now(false)).map((offer) => offer.id);
    expect(ids).not.toContain("skills.studio:alpha");
    expect(ids).not.toContain("settings.group:workspace.skills:alpha");
    expect(catalogue(now(true)).map((offer) => offer.id)).toContain(
      "settings.group:workspace.skills:alpha",
    );
  });
});

describe("the summary line's need you (#3695)", () => {
  it("adds K need you to the counts, which opens the workspace's Needs you", async () => {
    mockIPC((cmd) => {
      if (cmd === "skill_studio")
        return studio([skill("deploy"), skill("lint", { on: false })], {
          workspace: "alpha",
          needs: [need("a"), need("b")],
        });
      return null;
    });
    const asked: SettingsActionAsk[] = [];
    const heard = (event: Event) => asked.push((event as CustomEvent<SettingsActionAsk>).detail);
    window.addEventListener(SETTINGS_ACTION, heard);
    const user = userEvent.setup();
    render(<Line setting={workspaceSkillsGroup("p", "alpha").settings[0] as LiveSetting} />);

    await waitFor(() =>
      expect(screen.getByTestId("workspace-skills-summary")).toHaveTextContent(
        "Its chats get 1 skill, and 1 more is left out, each with why in the studio. · 2 need you",
      ),
    );
    await user.click(screen.getByRole("button", { name: "2 need you" }));
    window.removeEventListener(SETTINGS_ACTION, heard);
    expect(asked).toEqual([{ plane: "p", action: "skills.needs:alpha" }]);
  });

  it("says nothing of need you while nothing does", async () => {
    mockIPC((cmd) => (cmd === "skill_studio" ? studio([skill("deploy")]) : null));
    render(<Line setting={workspaceSkillsGroup("p", "alpha").settings[0] as LiveSetting} />);

    await waitFor(() =>
      expect(screen.getByTestId("workspace-skills-summary")).toHaveTextContent(
        "Its chats get 1 skill.",
      ),
    );
    expect(screen.queryByRole("button", { name: /need you/ })).toBeNull();
  });

  it("lands each need you on its scope's Needs you, a row no palette or menu lists", () => {
    const now: Now = {
      tabs: noTabs(),
      workspaces: ["alpha"],
      focused: "alpha",
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
      skillStudio: true,
      personas: ["ops"],
    };
    const rows = catalogue(now);
    const workspace = rows.find((offer) => offer.id === "skills.needs:alpha");
    expect(workspace?.does).toEqual({
      verb: "openView",
      view: skillStudioAt(inWorkspace("alpha"), "needs"),
      title: "Skill studio",
    });
    expect(workspace?.menuOnly).toBe(true);
    expect(workspace?.name).toBeUndefined();
    const persona = rows.find((offer) => offer.id === "skills.needs:persona/ops");
    expect(persona?.does).toEqual({
      verb: "openView",
      view: skillStudioAt(asPersona("ops"), "needs"),
      title: "Skill studio",
    });
    expect(catalogue({ ...now, skillStudio: false }).map((offer) => offer.id)).not.toContain(
      "skills.needs:alpha",
    );
  });
});
