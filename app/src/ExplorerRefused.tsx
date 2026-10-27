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
 * **A line of the tree, never a row to act on** (`TreeLine`, #2911): it has no tree entry to hang
 * on (a refused path is not a folder listing's entry), and nothing opens, compares, stages,
 * drags or picks it, so it holds no button or link; the arrows read it in its place, as every
 * line of a tree is read. Narrowed by the *Filter files* box as file rows are.
 */
export function namedNotRead(
  refused: readonly RefusedFile[],
  /** What the file rows are narrowed to, lower-cased; `""` narrows nothing. */
  filter: string,
): { drawn: readonly RefusedFile[]; past: number } {
  const kept =
    filter === ""
      ? refused
      : refused.filter((one) => one.path.toLocaleLowerCase().includes(filter));
  return { drawn: kept.slice(0, REFUSED_DRAWN), past: Math.max(0, kept.length - REFUSED_DRAWN) };
}

/** What one such line says: its path, its mark in words, and why. */
export function namedNotReadSaid(one: RefusedFile, said: Record<ChangeMark, string>): string {
  return `${one.path} ${said[one.mark]}${one.uncommitted ? " · not committed" : ""} · ${one.why}`;
}

/** One such line's words, as the line draws them. */
export function NamedNotReadWords({
  one,
  said,
}: {
  one: RefusedFile;
  /** The words a mark is said in. */
  said: Record<ChangeMark, string>;
}) {
  return (
    <>
      <CircleSlash className="node-icon" aria-hidden="true" />
      <span>
        <span className="refused-path ui-text-code">{one.path}</span> {said[one.mark]}
        {one.uncommitted && " · not committed"} · {one.why}
      </span>
    </>
  );
}
