// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { MergeShown, RulesetShown } from "./bindings";

/** **The gap beside merge protection** (R-151, phase 1's wording): what purlis's hold can't stop. */
export const MERGE_GAP = "A script that merges with the chat's token gets past purlis.";

/** **The ruleset state, as purlis read it** (R-147), bound to that read (R-150). */
const RULESET_SAID: Record<RulesetShown, (base: string) => string> = {
  protects: (base) => `A ruleset protects ${base}.`,
  none: (base) => `No ruleset on ${base}. That's fine: the merge still waits for you.`,
  unknown: (base) => `purlis could not tell whether a ruleset protects ${base}.`,
};

/** A time of day, as a row says when the forge was read. */
const clock = (secs: number) =>
  new Date(secs * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

/**
 * **What a merge row shows of its request** (T1-40; R-120, R-147, R-151, Q248), on its row in
 * Asks and on its chat's pane alike: the branch it merges into and who opened it, the checks as
 * of the forge's read, the ruleset state purlis read with the gap beside it, and the title.
 *
 * Every fact is the forge's, read with the app's own login. **The title comes last, as text the
 * chat may have written** (Q248): it is drawn as plain text, never as markdown or a link, and
 * nothing purlis says follows it, so no title can read as purlis's words.
 */
export function MergeFacts({ facts }: { facts: MergeShown }) {
  return (
    <span className="merge-facts">
      {`Into ${facts.base}, opened by ${facts.author}. Checks ${facts.checks} as of ${clock(facts.read_at)}. `}
      <span className="merge-protection">{`${RULESET_SAID[facts.ruleset](facts.base)} ${MERGE_GAP}`}</span>
      {" Title, as the chat may have written it: "}
      <span className="merge-title">{facts.title}</span>
    </span>
  );
}
