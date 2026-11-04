// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a skill's files are, as a tab opens them** (#3682): a skill's page, its Files tab
 * (#5183) and its checks each open a file of the skill's this way. A module of its own, so the
 * Files tab's chunk reads it without importing the page that loads the chunk.
 */
import type { StudioSkill } from "./bindings";
import { PLANE_ROOT, type Place } from "./pieceViews";

/** **The file a skill's text is in**, as a tab opens it, or why there is none to open; or, with
 *  `inner`, another file in its folder (`evals/triggers.json`). */
export function skillFileOf(
  skill: StudioSkill,
  inner = "SKILL.md",
): { place: Place; path: string } | { reason: string } {
  const { folder } = skill;
  if (skill.level === "built-in")
    return {
      reason: "It ships with purlis, inside the app, and is not one of the project's files.",
    };
  if (folder === "")
    return {
      reason: "It was installed from a source: its files are kept with the project's pins.",
    };
  if (folder.startsWith("/")) return { reason: "Its file is outside the project." };
  if (skill.level === "repo") {
    // `workspaces/<workspace>/<clone>/…`, as the core says a clone's folder.
    const [top, workspace, repo, ...rest] = folder.split("/");
    if (top !== "workspaces" || workspace === undefined || repo === undefined || rest.length === 0)
      return { reason: "Its file is outside the project." };
    return { place: { workspace, repo, piece: null }, path: `${rest.join("/")}/${inner}` };
  }
  return { place: PLANE_ROOT, path: `${folder}/${inner}` };
}
