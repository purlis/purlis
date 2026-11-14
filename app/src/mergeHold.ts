// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands, type MergeShown, type Shown } from "./bindings";
import { asksMoved } from "./asks";

/**
 * **The question a high-risk ask's confirmation is asked of** (Q242, Q268): the ask, the
 * sentence it asks once more, and what its answer is bound to. A confirmation holds only while
 * this is the same, so a row that moves under it takes it back.
 *
 * A held merge's is what its row shows of the request (T1-40): its head and every fact the forge
 * gave, so a refresh of any of them (Q248), the ruleset state among them (R-150), asks from the
 * start. The time of the read is no part of it: a re-read that finds the same is the same
 * question, of newer facts.
 */
export function confirmationOf(ask: Shown, facts?: MergeShown): string {
  const path = ask.answer;
  const bound =
    path.via === "merge-hold"
      ? JSON.stringify([path.repo, path.number, path.head, ...said(facts ?? path.facts)])
      : JSON.stringify(path);
  return [ask.ask, ask.risk?.confirm ?? "", bound].join("\n");
}

/** What a merge row shows of `facts`, but when they were read. */
const said = (facts: MergeShown) => [
  facts.title,
  facts.author,
  facts.base,
  facts.checks,
  facts.ruleset,
];

/**
 * **A held merge read again before Merge asks once more** (Q248): the question to ask once
 * more, of what the forge says now; or why it can't be asked. Neither where the request moved
 * to another head: its row is another ask now (Q242), which asks from the start.
 */
export async function rereadMerge(
  plane: string,
  ask: Shown,
): Promise<{ question?: string; refused?: string }> {
  const path = ask.answer;
  if (path.via !== "merge-hold") return { question: confirmationOf(ask) };
  const read = await commands.rereadHeldMerge(
    plane,
    ask.session,
    path.repo,
    path.number,
    path.head,
  );
  // The ask shows what the forge said now: the list is read again.
  asksMoved(plane);
  if (read.status === "error") return { refused: String(read.error) };
  if (read.data.head !== path.head) return {};
  return { question: confirmationOf(ask, read.data.facts) };
}

/** **purlis's own question for a hold that arrived without its risk** (Q268, Q269): asked once
 *  more on the pane and on the row alike, so a merge always takes two presses. */
export function fallbackConfirm(hold: Shown): string {
  const path = hold.answer;
  return path.via === "merge-hold"
    ? `Merge request ${path.number} of ${path.repo} at ${path.head.slice(0, 7)}? There is no undo.`
    : "Merge this request? There is no undo.";
}
