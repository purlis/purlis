// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type HarnessPlugins, type PlaneId } from "../bindings";
import { counted } from "../dispatches";
import { usePlaneChanged } from "../planeChanged";
import { SKILL_STUDIO_IN, SKILL_STUDIO_SHOW } from "../skillStudio";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **Settings › Workspace › Skills** (#2767; spec #2762, decision 35): a summary of the skills
 * this workspace's chats get, and the way into the Skill studio filtered to them. Every skill
 * choice is the studio's, so this row only counts: how many skills its chats get, and how many
 * are left out, each with its reason in the studio. Its link runs the catalogue's
 * `skills.studio:<ws>`, the row the workspace's menu offers too. Behind the studio's flag.
 */
export function workspaceSkillsGroup(plane: PlaneId, workspace: string): SettingsGroup {
  return {
    id: "workspace.skills",
    label: "Skills",
    help: "The skills this workspace's chats get. They are chosen in the Skill studio.",
    settings: [summarySetting(plane, workspace)],
    links: [{ label: "Open in Skill studio", action: `${SKILL_STUDIO_IN}${workspace}` }],
  };
}

/** The one line the group says: how many skills the workspace's chats get, and how many not. */
export function skillsSummary(on: number, leftOut: number): string {
  if (on === 0 && leftOut === 0) return "No skills reach this workspace's chats yet.";
  const gets = `Its chats get ${counted(on, "skill", "skills")}`;
  if (leftOut === 0) return `${gets}.`;
  return `${gets}, and ${counted(leftOut, "more is", "more are")} left out, each with why in the studio.`;
}

function summarySetting(plane: PlaneId, workspace: string): LiveSetting {
  return {
    id: "workspace.skills.summary",
    label: "In its chats",
    help: "Counted from the project's, the workspace's and the local skills, as a chat starting here gets them.",
    writes: "nothing",
    useControl: () => {
      const [said, setSaid] = useState<string>();
      const changed = usePlaneChanged([plane], undefined);
      useEffect(() => {
        let live = true;
        void commands.skillStudio(plane, workspace, null).then((answer) => {
          if (!live) return;
          if (answer.status === "error") setSaid(answer.error);
          else {
            const on = answer.data.skills.filter((skill) => skill.on).length;
            setSaid(skillsSummary(on, answer.data.skills.length - on));
          }
        });
        return () => {
          live = false;
        };
      }, [changed]);
      return {
        control: (ids) => (
          <p
            id={ids.id}
            aria-describedby={ids.describedBy}
            className="honest"
            data-testid="workspace-skills-summary"
          >
            {said ?? "Counting…"}
          </p>
        ),
      };
    },
  };
}

/**
 * **Settings' Plugins, once the Skill studio is on** (SK-6, #2769; spec #2762, decision 35):
 * every harness plugin choice is the studio's, so the group is one summary row, counted from
 * what the core surveyed, and the link that opens the studio on the same chats. `workspace` is
 * `null` at the Project level.
 */
export function pluginsSummaryGroup(
  id: "project.plugins" | "workspace.plugins",
  harnesses: readonly HarnessPlugins[],
  workspace: string | null,
): SettingsGroup {
  return {
    id,
    label: "Plugins",
    help: "The plugins the chats started here have on. They are chosen in the Skill studio.",
    settings: [
      {
        id: `${id}.summary`,
        label: "In its chats",
        help: "Counted from this machine's installs and the project's choices, as a chat starting here gets them.",
        writes: "nothing",
        useControl: () => ({
          control: (ids) => (
            <p
              id={ids.id}
              aria-describedby={ids.describedBy}
              className="honest"
              data-testid={`${id}-summary`}
            >
              {pluginsSummary(harnesses)}
            </p>
          ),
        }),
      },
    ],
    links: [
      {
        label: "Open in Skill studio",
        action: workspace === null ? SKILL_STUDIO_SHOW : `${SKILL_STUDIO_IN}${workspace}`,
      },
    ],
  };
}

/** The one line the Plugins summary says: for each program, how many of its plugins this
 *  project turned on and off, and how many it leaves to the program. */
export function pluginsSummary(harnesses: readonly HarnessPlugins[]): string {
  const lines = harnesses.map((harness) => {
    const chosen = harness.plugins.filter((it) => it.installed && it.pinned === null);
    const on = chosen.filter((it) => it.state === "on").length;
    const off = chosen.filter((it) => it.state === "off").length;
    const left = chosen.length - on - off;
    if (chosen.length === 0) return `${harness.title}: none installed.`;
    return `${harness.title}: ${counted(on, "turned on", "turned on")}, ${counted(off, "turned off", "turned off")}, ${counted(left, "left to it", "left to it")}.`;
  });
  return lines.join(" ");
}
