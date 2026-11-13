// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pane foot** (#5202, spec #5191, look A; ADR 0104): the quiet line under a chat's
 * terminal that says where the chat runs and how full its context is, wrapped here once. A screen
 * imports these and never writes `data-scope="pane-foot"` (`purlis/pane-foot-family`).
 *
 * - **`PaneFoot`**: the line, its `PaneFootStart` (harness · workspace) and its `PaneFootEnd`
 *   (the gauge). It is the tab strip's height at the density in force, so a compact appearance
 *   makes it compact, and it is drawn whether or not the gauge is known yet, so the terminal
 *   never moves for it.
 * - **`PaneFootSeparator`**: the `·` that joins two parts, a mark a reader is not given.
 * - **`PaneFootGauge`**: one labelled number, `ctx 62%`, and for `ctx` its fill as a thin bar
 *   under the number, in the core's tone for it. The number says the value; the bar is a mark a
 *   reader is not given, so the value is never told by its colour or length alone (WCAG 1.4.1).
 *   A number with no bar carries a warning tone itself.
 *
 * **Drawn by its parts** (ADR 0103): the `root`, its `start` and `end`, a `separator`, a `gauge`
 * with its `label`, `value`, `track` and the `fill` on it. `pane-foot.css` draws the family
 * default and a theme's `pane-foot` parts reach the same attributes.
 */
import type { ReactNode } from "react";

/** The core's tone for a gauge's value (`purlis_core::usage::gauge`), or `ended`, a chat's last
 *  value, greyed (#5205). */
export type PaneFootTone = "ok" | "warn" | "bad" | "ended";

/** **The pane foot**: the line under a chat's terminal, holding its {@link PaneFootStart} and
 *  its {@link PaneFootEnd}. */
export function PaneFoot({
  children,
  className,
  label,
}: {
  children: ReactNode;
  /** A place for it in its pane. Never a look. */
  className?: string;
  /** What a screen reader hears it as, naming the chat (spec #5191 story 39). */
  label: string;
}) {
  return (
    <div
      data-scope="pane-foot"
      data-part="root"
      className={className === undefined ? "ui-text-meta" : `${className} ui-text-meta`}
      role="group"
      aria-label={label}
    >
      {children}
    </div>
  );
}

/** **Where the chat runs**, at the foot's start: harness · workspace. Cut short before the end
 *  is. */
export function PaneFootStart({ children }: { children: ReactNode }) {
  return (
    <div data-scope="pane-foot" data-part="start">
      {children}
    </div>
  );
}

/** **Its gauge**, at the foot's end, which never gives way. */
export function PaneFootEnd({ children }: { children?: ReactNode }) {
  return (
    <div data-scope="pane-foot" data-part="end">
      {children}
    </div>
  );
}

/** **The `·` between two parts of the foot**: a mark, which a reader is not given. */
export function PaneFootSeparator() {
  return (
    <span data-scope="pane-foot" data-part="separator" aria-hidden="true">
      ·
    </span>
  );
}

/** **One gauge on the foot**: its label, its value and, where it has one, its fill as a thin
 *  bar under them. */
export function PaneFootGauge({
  label,
  value,
  fill,
  tone,
}: {
  /** Its name: `ctx`, `cache`. */
  label: string;
  /** Its value as figures: `62%`. */
  value: string;
  /** How full it is, from 0 to 100, and the core's tone for it: drawn as the bar. */
  fill?: { percent: number; tone: PaneFootTone };
  /** The core's tone for a number with no bar: a warning colours it, and ok leaves it plain. */
  tone?: PaneFootTone;
}) {
  return (
    <span data-scope="pane-foot" data-part="gauge">
      <span data-scope="pane-foot" data-part="label">
        {label}
      </span>{" "}
      <span
        data-scope="pane-foot"
        data-part="value"
        data-tone={tone === undefined || tone === "ok" ? undefined : tone}
      >
        {value}
      </span>
      {fill && (
        <span data-scope="pane-foot" data-part="track" aria-hidden="true">
          <span
            data-scope="pane-foot"
            data-part="fill"
            data-tone={fill.tone}
            style={{ inlineSize: `${Math.max(0, Math.min(100, fill.percent))}%` }}
          />
        </span>
      )}
    </span>
  );
}
