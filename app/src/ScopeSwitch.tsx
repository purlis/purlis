// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Segment, SegmentedControl } from "./ui/choice/Radio";

/** One scope a {@link ScopeSwitch} offers. */
export type ScopeOffer<S extends string> = {
  scope: S;
  /** Its words on the segment. */
  says: string;
  /** How many things it lists, drawn after its words where it is given. */
  count?: number;
  /** Its accessible name and tooltip, where the words and the number alone do not say it. */
  name?: string;
  /** Its tooltip, where it says more than its name: whose it is, or why it is out of reach. */
  tip?: string;
  /** Out of reach: greyed, never picked, its `tip` saying why, to a pointer and, as its
   *  description, to the keyboard and a screen reader. */
  disabled?: boolean;
};

/**
 * **The scope switch of a side view** (#1679): a few scopes, one picked, at the top of the view.
 * The Chats view's This tab, Workspace and All, and the Inbox's projects, project and workspace
 * are this one piece, so the two read and move alike.
 *
 * The segmented control (`ui/choice`, #2907) at the chrome's size, as Settings' level switcher
 * is at the control's: the arrow keys move the pick along. A pick is drawn at once and kept, with
 * no button: it writes nothing a second pick does not undo. A segment's count is beside its words
 * and in its name, never the only thing that tells two segments apart.
 */
export function ScopeSwitch<S extends string>({
  label,
  scopes,
  scope,
  onPick,
  countAs,
}: {
  /** What the group is called: "Show the chats of". */
  label: string;
  scopes: readonly ScopeOffer<S>[];
  scope: S;
  onPick: (to: S) => void;
  /** `round`: its counts are what waits on the person (Asks' chips, #6342), each the badge
   *  family's round count, drawn only above 0. */
  countAs?: "round";
}) {
  return (
    <SegmentedControl
      size="sm"
      className="scope-switch"
      aria-label={label}
      value={scope}
      onChange={(to) => {
        const one = scopes.find((each) => each.scope === to);
        if (one !== undefined) onPick(one.scope);
      }}
    >
      {scopes.map((one) => (
        <Segment
          key={one.scope}
          value={one.scope}
          aria-label={one.name}
          tooltip={one.tip ?? one.name}
          count={one.count}
          countAs={countAs}
          isDisabled={one.disabled}
          description={one.disabled === true ? one.tip : undefined}
        >
          {one.says}
        </Segment>
      ))}
    </SegmentedControl>
  );
}
