// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { paneNamesOf } from "./panePlaces";
import { panesOf, type Tabs } from "./tabs";

/**
 * **The one polite line about a pane the keyboard did not ask for** (#4767, ruled on #4699).
 *
 * Where the keyboard follows a change into the pane it was asked into (a sidebar press shown in
 * the focused pane, ⌘⌥ and an arrow), the terminal's own name says it ("steward 4, working,
 * terminal") and nothing more is said. Where a press finds its chat already on screen in another
 * pane of the tab and jumps there (a press, ⌘⇧H), the reader is told which pane, once. An agent
 * never moves a pane (D4), so nothing an agent does is said here.
 */
export function paneSaidOf(
  before: Tabs,
  next: Tabs,
  session: number,
  nameOf: (session: number) => string,
): string | undefined {
  const id = next.inFront;
  if (id === undefined || before.inFront !== id) return undefined;
  const was = before.byId[id];
  const tab = next.byId[id];
  if (was === undefined || tab === undefined || was.focused === tab.focused) return undefined;
  const there = panesOf(before, id).some(
    (one) => one.pane === tab.focused && one.session === session,
  );
  const place = there ? paneNamesOf(tab.layout).get(tab.focused) : undefined;
  return place === undefined
    ? undefined
    : `${nameOf(session)} is already in the ${place}; moved there.`;
}
