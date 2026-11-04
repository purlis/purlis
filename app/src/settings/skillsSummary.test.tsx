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
import { projectSkillsGroup, skillsSummary, workspaceSkillsGroup } from "./skillsSummary";
import { asPersona, inWorkspace, skillStudioAt, skillStudioView } from "../skillStudio";

/** The group's one line, drawn as its Settings row draws its control. */
function Line({ setting }: { setting: LiveSetting }) {
  return <>{setting.useControl().control({ id: "summary", describedBy: "help" })}</>;
}

/**
 * A loaded runner (#5531): it stalls 3 ms just after `testId` first commits, once Testing
 * Library's `waitFor` has set the 1 ms timer it hands control back on. React draws a React Aria
 * table's rows a commit after the rest, from a `setImmediate`, so the timer then fires between
 * the two commits, as it did in the merge queue.
 */
function stallAfter(testId: string) {
  let stalled = false;
  const watch = new MutationObserver(() => {
    if (stalled || document.querySelector(`[data-testid="${testId}"]`) === null) return;
    stalled = true;
    watch.disconnect();
    let hops = 50;
    const hop = () => {
      if (--hops > 0) return queueMicrotask(hop);
      const until = Date.now() + 3;
      while (Date.now() < until);
    };
    queueMicrotask(hop);
  });
  watch.observe(document.body, { childList: true, subtree: true });
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

    const now: Now = {
      tabs: noTabs(),
      workspaces: ["alpha"],
      focused: "alpha",
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
    };
    const ids = catalogue(now).map((offer) => offer.id);
    const row = catalogue(now).find((offer) => offer.id === "skills.studio:alpha");
    expect(row?.does).toEqual({
      verb: "openView",
      view: skillStudioView(inWorkspace("alpha")),
      title: "Skill studio",
    });
    expect(ids).toContain("settings.group:workspace.skills:alpha");
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
  });
});

describe("the skill budget's row (#4610, #5353)", () => {
  it("is a row of the project's group, which Settings drops if it has no rows", () => {
    expect(projectSkillsGroup("p").settings.map((one) => one.id)).toEqual([
      "project.skills.budget.in-force",
    ]);
  });

  it("says each program's budget, and what a program would drop, in the core's words", async () => {
    const line = "Codex: at 15,000 characters this is past the 8,000 it holds.";
    mockIPC(async (command) => {
      if (command === "skill_studio") {
        await new Promise((resolve) => setTimeout(resolve, 200));
        return {
          ...studio([skill("deploy")]),
          router: {
            budget: [
              {
                program: "Claude Code",
                key: "claude",
                skills: 60,
                chars: 15000,
                skills_said: "60, Claude Code's default",
                chars_said: "15,000, Claude Code's default",
                skills_here: null,
                chars_here: null,
              },
            ],
            past_limits: [line],
          },
        };
      }
      return null;
    });
    const [inForce] = projectSkillsGroup("p").settings as LiveSetting[];
    stallAfter("skill-budget-past-limit");
    render(<Line setting={inForce} />);
    // The table's rows land a commit after the past-limit line: wait for both.
    await waitFor(() => {
      expect(screen.getByTestId("skill-budget-past-limit")).toHaveTextContent(line);
      expect(screen.getByTestId("skill-budget-in-force")).toHaveTextContent(
        "60, Claude Code's default",
      );
    });
  });
});

describe("the palette's Skills rows (#4610)", () => {
  it("offers the project's Skills group", () => {
    const offered = catalogue({
      tabs: noTabs(),
      workspaces: [],
      needsYou: [],
      nameOf: (session) => String(session),
      plane: "p",
    }).some((offer) => offer.id === "settings.group:project.skills");
    expect(offered).toBe(true);
  });
});
