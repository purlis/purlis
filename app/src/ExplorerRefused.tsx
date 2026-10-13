// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { CircleSlash } from "lucide-react";
import type { ChangeMark, RefusedFile } from "./bindings";

/** The most paths named and not read that the explorer draws under one branch; the rest are
 *  counted. The core lists up to 10,000, and a tree of them would bury the branch's files. */
export const REFUSED_DRAWN = 50;

/**
 * **What a branch changed that purlis names and does not read, in the explorer** (#704): a
 * path with a `.git` folder on it, or one that is not a plain path, drawn under the branch's
 * files with its mark and the core's sentence for why.
 *
 * Text, never a row of the tree: it has no tree entry to hang on (a refused path is not a
 * folder listing's entry), and nothing opens, compares, stages, drags or picks it, so it is no
 * button, link or tab stop either. Narrowed by the *Filter files* box as file rows are.
 */
export function NamedNotRead({
  refused,
  filter,
  said,
}: {
  refused: readonly RefusedFile[];
  /** What the file rows are narrowed to, lower-cased; `""` narrows nothing. */
  filter: string;
  /** The words a mark is said in. */
  said: Record<ChangeMark, string>;
}) {
  const kept =
    filter === ""
      ? refused
      : refused.filter((one) => one.path.toLocaleLowerCase().includes(filter));
  if (kept.length === 0) return null;
  const past = kept.length - REFUSED_DRAWN;
  return (
    <div role="note" aria-label="Named and not read">
      {kept.slice(0, REFUSED_DRAWN).map((one) => (
        <p
          key={one.path}
          className="none ui-text-secondary"
          data-mark={one.mark}
          data-testid="explorer-refused-row"
        >
          <CircleSlash className="node-icon" aria-hidden="true" />
          <span>
            <span className="refused-path ui-text-code">{one.path}</span> {said[one.mark]}
            {one.uncommitted && " · not committed"} · {one.why}
          </span>
        </p>
      ))}
      {past > 0 && (
        <p className="none ui-text-secondary">{`${past.toLocaleString("en")} more named and not read`}</p>
      )}
    </div>
  );
}
