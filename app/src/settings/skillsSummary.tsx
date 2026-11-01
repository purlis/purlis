// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId } from "../bindings";
import { counted } from "../dispatches";
import { usePlaneChanged } from "../planeChanged";
import { OPEN_SKILL_STUDIO, SKILL_NEEDS_IN, SKILL_STUDIO_IN } from "../skillStudio";
import { budgetSaid } from "../SkillStudioRouting";
import { Button } from "../ui/button/Button";
import { key, numberAt, type Control } from "./fileControls";
import { askSettingsAction } from "./links";
import type { LiveSetting, Setting, SettingsGroup } from "./groups";

/**
 * **Settings › Workspace › Skills** (#2767; spec #2762, decision 35; #3695, the amendment's S10):
 * one line of counts for the skills this workspace's chats get, and the way into the Skill
 * studio on them. Every skill choice is the studio's, so this row only counts: how many skills
 * its chats get, how many are left out, and how many things need the person, which opens the
 * studio's Needs you. Its link runs the catalogue's `skills.studio:<ws>`, which lands on the
 * Library, the row the workspace's menu offers too. Behind the studio's flag.
 */
export function workspaceSkillsGroup(
  plane: PlaneId,
  workspace: string,
  budget: readonly Setting[] = [],
): SettingsGroup {
  return {
    id: "workspace.skills",
    label: "Skills",
    help: "The skills this workspace's chats get. They are chosen in the Skill studio.",
    settings: [summarySetting(plane, workspace), ...budget],
    after: [budgetInForce("workspace.skills.budget.in-force", plane, workspace)],
    links: [{ label: OPEN_SKILL_STUDIO, action: `${SKILL_STUDIO_IN}${workspace}` }],
  };
}

/**
 * **Settings › Skills at the project** (#4610): the skill budget of every chat in the project,
 * which a workspace's own can override, and what it comes to on each program. Behind the studio's
 * flag.
 */
export function projectSkillsGroup(plane: PlaneId, budget: readonly Setting[]): SettingsGroup {
  return {
    id: "project.skills",
    label: "Skills",
    help: "How many skills a chat lists before the router moves the rest into an index skill. Pin a skill in the Skill studio to keep it listed.",
    settings: budget,
    after: [budgetInForce("project.skills.budget.in-force", plane, null)],
  };
}

/** **The two numbers of `settings.skills.budget`**: an empty box leaves each program its own. */
export function budgetControls(): Control[] {
  return [
    numberAt(
      key("skills", "budget", "skills"),
      "Most skills in a chat's list",
      "Leave it empty for each program's own number.",
    ),
    numberAt(
      key("skills", "budget", "chars"),
      "Most characters in a chat's list",
      "Leave it empty for each program's own number. More characters cost more of each chat's context, about 91 tokens a skill.",
    ),
  ];
}

/** What each program's budget comes to here, and what a program would drop of it, the core's. */
function budgetInForce(id: string, plane: PlaneId, workspace: string | null): LiveSetting {
  return {
    id,
    label: "In force",
    help: "What a chat starting here gets on each program.",
    writes: "nothing",
    useControl: () => {
      const [said, setSaid] = useState<{ line: string; past: readonly string[] }>();
      const changed = usePlaneChanged([plane], undefined);
      useEffect(() => {
        let live = true;
        void commands.skillStudio(plane, workspace, null).then((answer) => {
          if (!live) return;
          if (answer.status === "error") setSaid({ line: answer.error, past: [] });
          else if (answer.data.router !== null)
            setSaid({
              line: budgetSaid(answer.data.router.budget),
              past: answer.data.router.past_limits,
            });
        });
        return () => {
          live = false;
        };
      }, [changed]);
      return {
        control: (ids) => (
          <div id={ids.id} aria-describedby={ids.describedBy} data-testid="skill-budget-in-force">
            <p className="honest">{said?.line ?? "Reading…"}</p>
            {said?.past.map((line) => (
              <p key={line} className="honest" data-testid="skill-budget-past-limit">
                {line}
              </p>
            ))}
          </div>
        ),
      };
    },
  };
}

/** The one line the group says: how many skills the workspace's chats get, and how many not. */
export function skillsSummary(on: number, leftOut: number): string {
  if (on === 0 && leftOut === 0) return "No skills reach this workspace's chats yet.";
  const gets = `Its chats get ${counted(on, "skill", "skills")}`;
  if (leftOut === 0) return `${gets}.`;
  return `${gets}, and ${counted(leftOut, "more is", "more are")} left out, each with why in the studio.`;
}

/** What the line's way into Needs you says: "1 needs you", "2 need you". */
export function needYouSaid(needing: number): string {
  return `${needing} ${needing === 1 ? "needs you" : "need you"}`;
}

function summarySetting(plane: PlaneId, workspace: string): LiveSetting {
  return {
    id: "workspace.skills.summary",
    label: "In its chats",
    help: "Counted from the project's, the workspace's and the local skills, as a chat starting here gets them.",
    writes: "nothing",
    useControl: () => {
      const [said, setSaid] = useState<{ line: string; needing: number }>();
      const changed = usePlaneChanged([plane], undefined);
      useEffect(() => {
        let live = true;
        void commands.skillStudio(plane, workspace, null).then((answer) => {
          if (!live) return;
          if (answer.status === "error") setSaid({ line: answer.error, needing: 0 });
          else {
            const on = answer.data.skills.filter((skill) => skill.on).length;
            setSaid({
              line: skillsSummary(on, answer.data.skills.length - on),
              needing: answer.data.needs.length,
            });
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
            {said?.line ?? "Counting…"}
            {said !== undefined && said.needing > 0 && (
              <>
                {" · "}
                <Button
                  inline
                  variant="ghost"
                  data-testid="workspace-skills-need-you"
                  onPress={() => askSettingsAction(plane, `${SKILL_NEEDS_IN}${workspace}`)}
                >
                  {needYouSaid(said.needing)}
                </Button>
              </>
            )}
          </p>
        ),
      };
    },
  };
}
