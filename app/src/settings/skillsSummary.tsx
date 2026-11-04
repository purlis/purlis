// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type StudioBudget } from "../bindings";
import { counted } from "../dispatches";
import { SETTINGS, usePlaneChanged } from "../planeChanged";
import { OPEN_SKILL_STUDIO, SKILL_NEEDS_IN, SKILL_STUDIO_IN } from "../skillStudio";
import { SkillBudgetTable, writeBudgetTo } from "../SkillBudget";
import { Button } from "../ui/button/Button";
import { askSettingsAction } from "./links";
import { skillActsRow, skillControlsRows } from "./skillControls";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **Settings › Workspace › Skills** (#2767; spec #2762, decision 35; #3695, the amendment's S10):
 * one line of counts for the skills this workspace's chats get, and the way into the Skill
 * studio on them. Every skill choice is the studio's, so this row only counts: how many skills
 * its chats get, how many are left out, and how many things need the person, which opens the
 * studio's Needs you. Its link runs the catalogue's `skills.studio:<ws>`, which lands on the
 * Library, the row the workspace's menu offers too.
 */
export function workspaceSkillsGroup(plane: PlaneId, workspace: string): SettingsGroup {
  return {
    id: "workspace.skills",
    label: "Skills",
    help: "The skills this workspace's chats get. They are chosen in the Skill studio.",
    settings: [
      summarySetting(plane, workspace),
      budgetInForce("workspace.skills.budget.in-force", plane, workspace),
    ],
    links: [{ label: OPEN_SKILL_STUDIO, action: `${SKILL_STUDIO_IN}${workspace}` }],
  };
}

/**
 * **Settings › Skills at the project** (#4610): the skill budget of every chat in the project,
 * which a workspace's own can override, and what it comes to on each program.
 */
export function projectSkillsGroup(plane: PlaneId): SettingsGroup {
  return {
    id: "project.skills",
    label: "Skills",
    help: "How many skills a chat lists before the router moves the rest into an index skill. Pin a skill in the Skill studio to keep it listed.",
    // A row, not `after`: Settings offers no group without one.
    settings: [budgetInForce("project.skills.budget.in-force", plane, null)],
  };
}

/**
 * **Settings › Skill sources at the project** (#6031, `prototype/skill-policy` A): this project's
 * `[skill_policy]` read only, each key saying it is set in `file`, the team's shared file, which
 * only a reviewed change to it changes. Behind the studio's flag.
 */
export function projectSkillSourcesGroup(plane: PlaneId, file: string): SettingsGroup {
  return {
    id: "project.skill-sources",
    label: "Skill sources",
    help: "This project's controls over the sources its skills install from: which are allowed, which are blocked, and how old a commit must be.",
    also: ["skill policy", "skill_policy", "allow list", "block list", "minimum age"],
    kept: `Kept in ${file}, which your team sees. Read only here: change it by a reviewed change to that file.`,
    settings: skillControlsRows(plane),
  };
}

/**
 * **Settings › Recorded skill acts** (#6031, SK-11): each install, update and removal of a skill
 * from a source this machine's audit recorded, newest first. Behind the studio's flag.
 */
export function projectSkillActsGroup(plane: PlaneId): SettingsGroup {
  return {
    id: "project.skill-acts",
    label: "Recorded skill acts",
    help: "Every install, update and removal of a skill from a source is recorded where a project has skill controls.",
    also: ["skill audit", "audit"],
    settings: [skillActsRow(plane)],
  };
}

/**
 * **What each program's budget comes to here and where each number comes from, with Change… and
 * Reset to default** (#5353): the same table the Routing page draws, and what a program would drop
 * of it, the core's. `settings.skills.budget`'s numbers are written here, so a read-only project
 * turns the table's buttons off with the project's sentence.
 */
function budgetInForce(id: string, plane: PlaneId, workspace: string | null): LiveSetting {
  const level = { workspace, persona: null };
  const write = writeBudgetTo(plane, level);
  return {
    id,
    label: "In force",
    help: "What a chat starting here gets on each program, and where each number comes from. More characters cost more of each chat's context, about 91 tokens a skill.",
    writes: "per control",
    useControl: () => {
      const [said, setSaid] = useState<{
        refused?: string;
        budget: readonly StudioBudget[];
        past: readonly string[];
        extensionRouter: boolean;
      }>();
      const changed = usePlaneChanged([plane], SETTINGS);
      const [written, setWritten] = useState(0);
      useEffect(() => {
        let live = true;
        void commands.skillStudio(plane, workspace, null).then((answer) => {
          if (!live) return;
          if (answer.status === "error")
            setSaid({ refused: answer.error, budget: [], past: [], extensionRouter: false });
          else if (answer.data.router !== null)
            setSaid({
              budget: answer.data.router.budget,
              past: answer.data.router.past_limits,
              extensionRouter: answer.data.router.extension !== null,
            });
        });
        return () => {
          live = false;
        };
      }, [changed, written]);
      return {
        control: (ids) => (
          <div id={ids.id} aria-describedby={ids.describedBy} data-testid="skill-budget-in-force">
            {said === undefined && <p className="honest">Reading…</p>}
            {said?.refused !== undefined && <p className="honest">{said.refused}</p>}
            {said !== undefined && said.budget.length > 0 && (
              <SkillBudgetTable
                plane={plane}
                budget={said.budget}
                level={level}
                extensionRouter={said.extensionRouter}
                onWrite={(one, told) =>
                  write(one, {
                    refused: told.refused,
                    done: () => {
                      told.done();
                      setWritten((was) => was + 1);
                    },
                  })
                }
              />
            )}
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
