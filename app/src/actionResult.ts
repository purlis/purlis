// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { Said } from "./actions";

/**
 * **What an action answered, in the person's words** (#3344, the sidebar-views prototype's Q3 A).
 *
 * The window once joined the core's CLI lines into the line under the title bar, word for word:
 * "Workspace 'git-ci-cd' ready (LIVE — charter + manifest + memory committed + shared +
 * auto-saved) → workspaces/git-ci-cd/ ⬢ No vision yet — ask the developer what this workspace is
 * for…". The second of those is written for an agent in a terminal, not for the person. Now a
 * known result is said here, in the app's own words: a short title, one plain sentence, its ways
 * on, and the core's lines kept behind Details, less every line written for an agent.
 */

/** What a result's notice draws: its title, its one sentence, and what the core said. */
export type Result = { title: string; says: string | undefined; details: string | undefined };

/** **A line the core writes for an agent**, never shown to the person: its `⬢` lines, which tell
 *  the agent reading a terminal what to ask its developer. */
function forAnAgent(line: string): boolean {
  return line.trimStart().startsWith("⬢") || /\bask the developer\b/i.test(line);
}

/** The core's lines a person may read: every one but those written for an agent. */
export function forThePerson(lines: readonly string[]): string[] {
  return lines.filter((line) => line.trim() !== "" && !forAnAgent(line));
}

/** What a result's notice draws, for any `Said`: its own title where it has one, else its words. */
export function resultOf(said: Said): Result {
  return said.title === undefined
    ? { title: said.words, says: undefined, details: said.details }
    : { title: said.title, says: said.words, details: said.details };
}

/** What the core says when a workspace is made with no vision (`wscmd/create.rs`). */
const NO_VISION = /no vision yet/i;

/**
 * **A workspace made** (`purlis workspace create`, `wscmd/create.rs`): "Workspace git-ci-cd is
 * ready", and whether it has a vision yet, with Open; the core's own lines in its details. A pin
 * the store refused is said after the sentence: the workspace is made all the same.
 */
export function workspaceMade(
  name: string,
  lines: readonly string[],
  { onOpen, pinRefused }: { onOpen: () => void; pinRefused?: string },
): Said {
  const vision = lines.some((line) => NO_VISION.test(line))
    ? "It has no vision yet. Add one so its chats know what it is for."
    : "Its vision is recorded in its workspace.md.";
  const kept = forThePerson(lines);
  return {
    from: "workspace.create",
    refused: false,
    title: `Workspace ${name} is ready`,
    words: pinRefused === undefined ? vision : `${vision} ${pinRefused}`,
    details: kept.length === 0 ? undefined : kept.join("\n"),
    ways: [{ label: `Open ${name}`, onPress: onOpen }],
  };
}
