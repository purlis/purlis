// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Level } from "./groups";

/** What else a person calls You › Reports (#5688): the palette and the filter find it by each. */
export const REPORTS_ALSO: readonly string[] = [
  "crash",
  "crash reports",
  "telemetry",
  "error reporting",
  "bug report",
  "diagnostics",
  "privacy",
];

/** What else a person calls You › Attention (#6019), as its builder lists them. */
export const ATTENTION_ALSO: readonly string[] = [
  "notifications",
  "quiet",
  "quiet hours",
  "stuck",
  "sound",
  "do not disturb",
];

/**
 * **Every Settings group by its address and its label, without building a level** (#1201).
 *
 * A group's id and label live inside its level's builder (`you.tsx`, `project.ts`,
 * `workspace.tsx`, and the pages `dispatch.tsx`, `GrantedList.tsx`, `thisMachine.tsx`, `browserLimits.tsx`, `pools.tsx`, `notifications.tsx`, `accounts.tsx`, `profiles.tsx` and `skillsSummary.tsx` add),
 * which needs the level's files read before it can say anything. The palette lists one row per
 * group before anything is read, so it reads this list instead. `catalogue.test.ts` reads the
 * builders' sources and fails on a group this list lacks, misnames or keeps after it went.
 *
 * In each level's order, as its group nav lists them. A profile's own page
 * (`project.harness.profile.<name>`) is not here: there is one per profile, so the palette
 * builds its rows from the project's profiles (`profilePageRows` in `actions.ts`, #1201) rather
 * than from this static list. The Persona level has no groups of its own yet.
 */
export type CataloguedGroup = {
  id: string;
  label: string;
  /** Other words the palette finds the group by, never drawn (#5688): what a person may call
   *  it that its label does not say. The group's builder holds the same list, for the filter. */
  also?: readonly string[];
};

export const SETTINGS_GROUPS: Readonly<
  Record<Exclude<Level, "persona">, readonly CataloguedGroup[]>
> = {
  you: [
    { id: "you.text", label: "Text" },
    { id: "you.theme", label: "Theme" },
    { id: "you.look", label: "Density and fonts" },
    { id: "you.motion", label: "Motion and contrast" },
    { id: "you.colours", label: "Your colours" },
    { id: "you.editor", label: "Editor" },
    { id: "you.chats", label: "Chats list" },
    { id: "you.notifications", label: "Attention", also: ATTENTION_ALSO },
    { id: "you.reports", label: "Reports", also: REPORTS_ALSO },
    { id: "you.machine", label: "This machine" },
    { id: "you.machine.browsers", label: "Browsers" },
    { id: "you.machine.pools", label: "Pools" },
    { id: "you.accounts", label: "Accounts" },
    { id: "you.profiles", label: "Profiles" },
  ],
  project: [
    { id: "project.general", label: "General" },
    { id: "project.saving", label: "Saving" },
    { id: "project.harness", label: "Harness & profiles" },
    { id: "project.skills", label: "Skills" },
    // Behind the same flag (#6031).
    {
      id: "project.skill-sources",
      label: "Skill sources",
      also: ["skill policy", "skill_policy", "allow list", "block list", "minimum age"],
    },
    { id: "project.skill-acts", label: "Recorded skill acts", also: ["skill audit", "audit"] },
    { id: "project.sandbox", label: "Sandbox" },
    { id: "project.sandbox.mine", label: "Your hosts" },
    { id: "project.sandbox.yours", label: "Your level" },
    { id: "project.sandbox.personas", label: "Persona hosts" },
    { id: "project.sandbox.network", label: "Network" },
    { id: "project.dispatch", label: "Dispatch" },
    { id: "project.runs", label: "Runs on" },
    { id: "project.forges", label: "Forges" },
    { id: "project.extensions", label: "Extensions" },
    { id: "project.theme", label: "Theme" },
    { id: "project.appearance", label: "Appearance" },
  ],
  workspace: [
    { id: "workspace.live", label: "Live" },
    { id: "workspace.repos", label: "Repos" },
    { id: "workspace.dispatch", label: "Dispatch" },
    { id: "workspace.extensions", label: "Extensions" },
    { id: "workspace.theme", label: "Theme" },
    { id: "workspace.appearance", label: "Appearance" },
    { id: "workspace.skills", label: "Skills" },
  ],
};
