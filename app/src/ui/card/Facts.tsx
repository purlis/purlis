// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Facts, as label and value** (#3673, the card family): what a screen says it knows about one
 * thing, read only, a line a fact: a skill's Overview (who gets it, where it lives, its owner,
 * its grade, its runs), a harness skill's item, a source's header, the grade plan's "What it may
 * spend". A screen draws its facts with these and never writes its own `dl` grid.
 *
 * - **A `dl`**: each fact a `div` of its label (`dt`) and its value (`dd`), so a screen reader
 *   hears each label with its value, and the list as one.
 * - **The label column** stands at the start, one width for every fact (the `facts-label` part,
 *   in the secondary tier); **the value column** takes the rest (`facts-value`, in the primary
 *   tier). A value may hold a badge, a link or one button (a fix, where the fact says something
 *   is left out).
 * - **Where it is narrow** (the facts' own room, under 24rem) each label stands over its value.
 *
 * Drawn by the card family's parts (`card.css`), which a theme reaches (`card.facts-label`,
 * `card.facts-value`).
 */
import type { ReactNode } from "react";

/** **A list of facts**: its `Fact`s, one a line, named for a reader where the page does not. */
export function Facts({
  children,
  className,
  "aria-label": label,
  "aria-labelledby": labelledBy,
}: {
  children: ReactNode;
  /** A place for it in its view: a margin, a width. Never a look. */
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
}) {
  return (
    <dl
      data-scope="card"
      data-part="facts"
      className={className}
      aria-label={label}
      aria-labelledby={labelledBy}
    >
      {children}
    </dl>
  );
}

/** **One fact**: its label, and its value (words, a badge, a link, or one button). */
export function Fact({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div data-scope="card" data-part="fact">
      <dt data-scope="card" data-part="facts-label">
        {label}
      </dt>
      <dd data-scope="card" data-part="facts-value">
        {children}
      </dd>
    </div>
  );
}
