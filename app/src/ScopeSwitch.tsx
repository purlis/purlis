// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import * as RadioGroup from "@radix-ui/react-radio-group";
import { useArrowPick } from "./settings/components";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** One scope a {@link ScopeSwitch} offers. */
export type ScopeOffer<S extends string> = {
  scope: S;
  /** Its words on the segment. */
  says: string;
  /** How many things it lists, drawn after its words where it is given. */
  count?: number;
  /** Its accessible name and tooltip, where the words and the number alone do not say it. */
  name?: string;
};

/**
 * **The scope switch of a side view** (#1679): a few scopes, one picked, at the top of the view.
 * The Chats view's This tab, Workspace and All, and the Inbox's projects, project and workspace
 * are this one piece, so the two read and move alike.
 *
 * A radio group drawn as one row of segments, as Settings' level switcher is (Radix's, with the
 * settings set's arrow repair, `useArrowPick`). A pick is drawn at once and kept, with no
 * button: it writes nothing a second pick does not undo. A segment's count is beside its words
 * and in its name, never the only thing that tells two segments apart.
 */
export function ScopeSwitch<S extends string>({
  label,
  scopes,
  scope,
  onPick,
}: {
  /** What the group is called: "Show the chats of". */
  label: string;
  scopes: readonly ScopeOffer<S>[];
  scope: S;
  onPick: (to: S) => void;
}) {
  const { arrowing, listen } = useArrowPick();
  return (
    <RadioGroup.Root
      className="ui-levels ui-levels-small"
      aria-label={label}
      orientation="horizontal"
      value={scope}
      onValueChange={(to) => {
        const one = scopes.find((each) => each.scope === to);
        if (one !== undefined) onPick(one.scope);
      }}
      {...listen}
    >
      {scopes.map((one) => (
        <Tooltip key={one.scope} tip={one.name}>
          <RadioGroup.Item
            className="ui-level"
            value={one.scope}
            aria-label={one.name}
            onFocus={() => {
              if (arrowing.current && one.scope !== scope) onPick(one.scope);
            }}
          >
            {one.count === undefined ? (
              one.says
            ) : (
              <>
                <span className="ui-level-says">{one.says}</span>
                <span className="ui-level-count">{one.count}</span>
              </>
            )}
          </RadioGroup.Item>
        </Tooltip>
      ))}
    </RadioGroup.Root>
  );
}
