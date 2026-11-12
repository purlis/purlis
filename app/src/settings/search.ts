// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { PlaneId, SettingsFile, SettingsWhich } from "../bindings";
import { anyPoolKept } from "../pools";
import { heldIn, type Files } from "./driver";
import { NO_EXTENSIONS } from "./fileControls";
import { inAFile, LEVELS, type FileSetting, type Level, type SettingsGroup } from "./groups";
import { projectGroups } from "./project";
import { workspaceGroups } from "./workspace";
import { youGroups } from "./you";

/**
 * **One settings search across every level** (#2920, N27; #2814·4).
 *
 * A level's groups are built by its builder from the level's files as read (`project.ts`,
 * `workspace.tsx`, `you.tsx`). Built over files nothing has been read into, the same builders
 * name every setting a level has, by the address, label and help the level draws it with, and
 * ask the core nothing. That is what the palette lists a row for, one per setting, and what the
 * Settings search finds at the levels other than the one on screen: one list, and one rule for
 * finding in it ({@link found}).
 *
 * What only a read can say is not in it: a collection's entries (a profile's own page, a forge),
 * which are found under their group at the level on screen, and whether a value is set.
 */

/** A level and the groups it holds, in the level's order. */
export type LevelGroups = { level: Level; groups: readonly SettingsGroup[] };

/** The levels a Settings tab offers (`SettingsTab`): You always, Project in a project, and
 *  Workspace about one of its workspaces. Persona has no groups yet. */
export function offeredLevels(plane?: string, workspace?: string): Level[] {
  return LEVELS.map((one) => one.id).filter(
    (level) =>
      level === "you" ||
      (level === "project" && plane !== undefined) ||
      (level === "workspace" && plane !== undefined && workspace !== undefined),
  );
}

/** A file purlis has read nothing into: there, parsed, and holding no value. */
function blank(which: SettingsWhich, file: string): SettingsFile {
  return { which, file, exists: true, text: "", refusals: [], parsed: true, fields: [] };
}

/**
 * **Every setting of every level offered**, at `plane` and `workspace`, built over blank files.
 * The Skills groups come only while the Skill studio is on, as Settings offers them; where a
 * workspace's chats run, only once this machine keeps a pool.
 */
export function everySetting({
  plane,
  workspace,
  skillStudio = false,
}: {
  plane?: string;
  workspace?: string;
  skillStudio?: boolean;
}): LevelGroups[] {
  return offeredLevels(plane, workspace).map((level) => ({
    level,
    groups:
      level === "you"
        ? youGroups(plane)
        : level === "project"
          ? projectGroups(
              {
                shared: blank("shared", "charter.toml"),
                local: blank("local", "charter.local.toml"),
                extensions: NO_EXTENSIONS,
                theme: undefined,
                saving: undefined,
                sandbox: undefined,
                entries: {},
                plane: plane as PlaneId,
              },
              undefined,
              skillStudio,
            )
          : workspaceGroups(
              {
                plane: plane as PlaneId,
                workspace: workspace ?? "",
                settings: {
                  ...blank("shared", "workspace.json"),
                  workspace: workspace ?? "",
                  live: false,
                },
                extensions: NO_EXTENSIONS,
                theme: undefined,
                poolsKept: anyPoolKept(),
              },
              () => {},
              skillStudio,
            ),
  }));
}

/**
 * **The groups and settings a search's words leave** (SE-21, V89c): a group whose own label or
 * help holds the words, or one of its other words (`also`, #5688), keeps every setting; any
 * other group keeps the settings whose label or help holds them, and is left out when none does.
 * Case is ignored, and so is space around the words; with none, every group is left as it is.
 */
export function narrowed(
  groups: readonly SettingsGroup[],
  filter: string,
): readonly SettingsGroup[] {
  const words = filter.trim().toLocaleLowerCase();
  if (words === "") return groups;
  const holds = (...texts: string[]) =>
    texts.some((text) => text.toLocaleLowerCase().includes(words));
  return groups.flatMap((one) => {
    if (holds(one.label, one.help, ...(one.also ?? []))) return [one];
    // Where a value is kept is said once in its group's header (DS-8), and still finds it.
    const settings = one.settings.filter((setting) =>
      holds(setting.label, setting.help, (inAFile(setting) && setting.kept) || ""),
    );
    const after = one.after?.filter((setting) => holds(setting.label, setting.help));
    // A group a search finds only by what it explains is found too (#1340).
    return settings.length > 0 || (after?.length ?? 0) > 0 ? [{ ...one, settings, after }] : [];
  });
}

/** {@link narrowed} at every level: the levels where something is left, in their order. */
export function found(levels: readonly LevelGroups[], words: string): LevelGroups[] {
  if (words.trim() === "") return [...levels];
  return levels.flatMap((one) => {
    const groups = narrowed(one.groups, words);
    return groups.length > 0 ? [{ level: one.level, groups }] : [];
  });
}

/** **Whether a file setting is set at this level** (R53): one of the level's files holds it. */
export function isSetHere(setting: FileSetting, files: Files): boolean {
  return heldIn(setting, files) !== undefined;
}
