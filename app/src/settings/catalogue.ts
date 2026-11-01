// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Level } from "./groups";

/**
 * **Every Settings group by its address and its label, without building a level** (#1201).
 *
 * A group's id and label live inside its level's builder (`you.tsx`, `project.ts`,
 * `workspace.tsx`, and the pages `dispatch.tsx`, `GrantedList.tsx`, `thisMachine.tsx`, `browserLimits.tsx`, `pools.tsx`, `notifications.tsx`, `accounts.tsx`, `profiles.tsx`, `previews.tsx` and `skillsSummary.tsx` add),
 * which needs the level's files read before it can say anything. The palette lists one row per
 * group before anything is read, so it reads this list instead. `catalogue.test.ts` reads the
 * builders' sources and fails on a group this list lacks, misnames or keeps after it went.
 *
 * In each level's order, as its group nav lists them. A profile's own page
 * (`project.harness.profile.<name>`) is not here: there is one per profile, so the palette
 * builds its rows from the project's profiles (`profilePageRows` in `actions.ts`, #1201) rather
 * than from this static list. The Persona level has no groups of its own yet.
 */
export type CataloguedGroup = { id: string; label: string };

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
    { id: "you.notifications", label: "Notifications" },
    { id: "you.machine", label: "This machine" },
    { id: "you.machine.browsers", label: "Browsers" },
    { id: "you.machine.pools", label: "Pools" },
    { id: "you.accounts", label: "Accounts" },
    { id: "you.profiles", label: "Profiles" },
    { id: "you.previews", label: "Previews" },
  ],
  project: [
    { id: "project.general", label: "General" },
    { id: "project.saving", label: "Saving" },
    { id: "project.harness", label: "Harness & profiles" },
    // Behind the Skill studio's flag (#4610): offered only while it is on.
    { id: "project.skills", label: "Skills" },
    { id: "project.sandbox", label: "Sandbox" },
    { id: "project.sandbox.mine", label: "Your hosts" },
    { id: "project.sandbox.yours", label: "Your level" },
    { id: "project.sandbox.personas", label: "Persona hosts" },
    { id: "project.sandbox.network", label: "Network" },
    { id: "project.dispatch", label: "Dispatch" },
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
    // Behind the Skill studio's flag (#2767): offered only while it is on.
    { id: "workspace.skills", label: "Skills" },
  ],
};
