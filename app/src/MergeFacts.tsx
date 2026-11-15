// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { LookShown, MergeShown, RulesetShown } from "./bindings";

/** **The gap beside merge protection** (R-151, phase 1's wording): what purlis's hold can't stop. */
export const MERGE_GAP = "A script that merges with the chat's token gets past purlis.";

/** **The ruleset state, as purlis read it** (R-147), bound to that read (R-150). */
const RULESET_SAID: Record<RulesetShown, (base: string) => string> = {
  protects: (base) => `A ruleset protects ${base}.`,
  none: (base) => `No ruleset on ${base}. That's fine: the merge still waits for you.`,
  unknown: (base) => `purlis could not tell whether a ruleset protects ${base}.`,
};

/** **What the person has seen of the head** (T1-41; R-122, R-123), as the core says it. A merge
 *  takes two presses whichever it is (Q268): this says what the second is asked over. The
 *  approving reviewer's login (Q277) comes after purlis's own words, as plain text. */
const LOOK_SAID: Record<LookShown, (facts: MergeShown) => string> = {
  viewed: () => "You viewed these changes in purlis at this head.",
  approved: ({ approved_by }) =>
    approved_by === null
      ? "Approved at this head by a reviewer who isn't its author, an app or a bot."
      : `Approved at this head by ${approved_by}.`,
  "not-viewed": ({ since_viewed: since }) =>
    since === null
      ? "You haven't viewed these changes in purlis."
      : `${since} ${since === 1 ? "commit" : "commits"} since you looked.`,
};

/** **A head the chat didn't push** (R-124), by the pushes purlis recorded for it. */
export const NOT_PUSHED_HERE = "This chat didn't push this head, by the pushes purlis recorded.";

/** A time of day, as a row says when the forge was read. */
const clock = (secs: number) =>
  new Date(secs * 1000).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

/**
 * **What a merge row shows of its request** (T1-40; R-120, R-147, R-151, Q248), on its row in
 * Asks and on its chat's pane alike: the branch it merges into and who opened it, the checks as
 * of the forge's read, the ruleset state purlis read with the gap beside it, and the title.
 *
 * Beside them, what the person has seen of the head (T1-41): viewed in purlis, approved on the
 * forge by someone else, or neither, with the commits since they looked; and a flag where the
 * chat didn't push the head, by the pushes purlis recorded.
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
      {` ${LOOK_SAID[facts.look](facts)}`}
      {!facts.pushed_here && ` ${NOT_PUSHED_HERE}`}
      {" Title, as the chat may have written it: "}
      <span className="merge-title">{facts.title}</span>
    </span>
  );
}
