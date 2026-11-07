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
    const found = inACheckout(folder);
    if (found === undefined) return { reason: "Its file is outside the project." };
    const { workspace, repo, piece, rest } = found;
    return { place: { workspace, repo, piece: piece ?? null }, path: `${rest.join("/")}/${inner}` };
  }
  return { place: PLANE_ROOT, path: `${folder}/${inner}` };
}

/** **Where a repo skill's folder is**, from its path in the project: in a clone's own folder, or
 *  in a branch folder of one (`workspaces/<workspace>/.worktrees/<clone>/<piece>/…`, #4326).
 *
 *  A clone's own folder is `workspaces/<workspace>/repos/<clone>/…` for one cloned from structure
 *  version 6 on, and `workspaces/<workspace>/<clone>/…` for one cloned before it (ADR 0114), as
 *  the core's `repofolder` has them. An old clone named `repos` is told from the folder by what
 *  follows it: a clone's name never starts with a dot, and the folder a clone keeps its skills in
 *  (`.claude/skills`) does. */
export function inACheckout(
  folder: string,
): { workspace: string; repo: string; piece?: string; rest: string[] } | undefined {
  const [top, workspace, repo, ...rest] = folder.split("/");
  if (top !== "workspaces" || !workspace || !repo) return undefined;
  if (repo === BRANCH_FOLDERS) {
    const [clone, piece, ...inside] = rest;
    if (!clone || !piece || inside.length === 0) return undefined;
    return { workspace, repo: clone, piece, rest: inside };
  }
  const [first, ...below] = rest;
  if (repo === REPOS && first && !first.startsWith(".")) {
    return below.length === 0 ? undefined : { workspace, repo: first, rest: below };
  }
  return rest.length === 0 ? undefined : { workspace, repo, rest };
}

/** The folder of a workspace that holds its branch folders (the core's `worktree::DIR_NAME`). */
const BRANCH_FOLDERS = ".worktrees";

/** The folder of a workspace its clones go in (the core's `repofolder::REPOS`). */
const REPOS = "repos";
