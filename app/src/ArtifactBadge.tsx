// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ArtifactPinShown } from "./bindings";
import { ActivityCount } from "./ui/activity/ActivityItem";

/** The largest number a badge draws as it is; anything above is drawn as this and a plus. */
export const MOST_SHOWN = 99;

/**
 * **The number an artifact's icon shows** (spec #5948 story 22, #5965): what its data declares
 * (`data.json`'s `badge`, `purlis_core::artifact::badge`), and nothing else. It is drawn where an
 * activity item draws its count, and is read again with the data, since the pins are.
 *
 * - **A zero or no number draws nothing**, as a count of nothing does.
 * - **Past {@link MOST_SHOWN} it is drawn "99+"**, so the badge stays the size of the bar's other
 *   counts; its sentence says the whole number.
 * - **Stale data dims it with the icon** (`activity.css`), and its sentence says so: the number
 *   is as old as the data that declared it.
 * - **Its sentence is the icon's description**, never its name, which is the artifact's (axe's
 *   label-content-name match): hidden here, and named by the icon's `aria-describedby`
 *   ({@link badgeDescribes}).
 */
export function ArtifactBadge({ pin, stale }: { pin: ArtifactPinShown; stale: boolean }) {
  const count = shown(pin);
  if (count === undefined) return null;
  return (
    <span id={badgeId(pin)} aria-hidden="true">
      <ActivityCount
        count={count}
        said={badgeSaid(count, stale)}
        drawn={count > MOST_SHOWN ? `${MOST_SHOWN}+` : undefined}
      />
    </span>
  );
}

/** The ids an artifact icon's `aria-describedby` names: those it already has, and its badge's. */
export function badgeDescribes(
  pin: ArtifactPinShown,
  others: string | undefined,
): string | undefined {
  const ids = [others, shown(pin) === undefined ? undefined : badgeId(pin)].filter(
    (id): id is string => id !== undefined && id !== "",
  );
  return ids.length === 0 ? undefined : ids.join(" ");
}

/** What a badge of `count` tells a screen reader, under the artifact's name. */
export function badgeSaid(count: number, stale: boolean): string {
  return stale ? `Its data shows ${count}, and is out of date` : `Its data shows ${count}`;
}

/** The number a pin's badge draws, or nothing for none or a zero. */
function shown(pin: ArtifactPinShown): number | undefined {
  const count = pin.badge ?? 0;
  return count > 0 ? count : undefined;
}

/** The badge's element id: one word, whatever a hand-written pins file calls the pin. */
function badgeId(pin: ArtifactPinShown): string {
  return `artifact-badge-${pin.id.replace(/[^A-Za-z0-9_-]/g, "_")}`;
}
