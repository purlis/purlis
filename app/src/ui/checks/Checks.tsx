// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The checks family** (#4481, spec #4469, ADR 0104): a list of things checked, each with its
 * verdict. A screen imports {@link CheckList} and {@link CheckLine} and never writes
 * `data-scope="checks"` (`purlis/checks-family`).
 *
 * A verdict is told by its mark and its words, never by colour alone (WCAG 1.4.1): each line says
 * its verdict to a reader before its title.
 *
 * **Drawn by its parts** (ADR 0103): the `root` list, each `item` (its `data-variant` is its
 * verdict), its `mark`, its `title`, what is said `more` under it, and a `trail` at its end.
 * `checks.css` draws the family default and a theme's parts reach the same attributes.
 */
import type { ReactNode } from "react";
import { VisuallyHidden } from "react-aria";
import { AlertTriangle, Check, Circle, Loader2, Minus, X } from "lucide-react";

/** A check's verdicts, in the order the manifest names them. */
export const CHECK_VERDICTS = ["ok", "missing", "running", "todo", "warn", "skip"] as const;
export type CheckVerdict = (typeof CHECK_VERDICTS)[number];

/** What a reader hears of a verdict before the line's title. */
const SAID: Record<CheckVerdict, string> = {
  ok: "Passed",
  missing: "Failed",
  running: "Running",
  todo: "Not yet",
  warn: "Needs a look",
  skip: "Skipped",
};

const MARK: Record<CheckVerdict, ReactNode> = {
  ok: <Check aria-hidden size={16} />,
  missing: <X aria-hidden size={16} />,
  running: <Loader2 aria-hidden size={16} className="spinning" />,
  todo: <Circle aria-hidden size={10} />,
  warn: <AlertTriangle aria-hidden size={16} />,
  skip: <Minus aria-hidden size={16} />,
};

/** **What was checked**: `label` is what a reader hears the list by. */
export function CheckList({
  "aria-label": label,
  className,
  children,
}: {
  "aria-label": string;
  /** A place for it in its view: a margin, a width. Never a look. */
  className?: string;
  children: ReactNode;
}) {
  return (
    <ul aria-label={label} className={className} data-scope="checks" data-part="root">
      {children}
    </ul>
  );
}

/** One line: its verdict, what was checked, more under it, and something at its end. */
export function CheckLine({
  verdict,
  title,
  trail,
  children,
}: {
  verdict: CheckVerdict;
  title: ReactNode;
  trail?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <li data-scope="checks" data-part="item" data-variant={verdict}>
      <span data-scope="checks" data-part="mark">
        {MARK[verdict]}
        <VisuallyHidden>{SAID[verdict]}: </VisuallyHidden>
      </span>
      <div data-scope="checks" data-part="words">
        <div data-scope="checks" data-part="title">
          {title}
        </div>
        {children && (
          <div data-scope="checks" data-part="more">
            {children}
          </div>
        )}
      </div>
      {trail && (
        <div data-scope="checks" data-part="trail">
          {trail}
        </div>
      )}
    </li>
  );
}
