// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { contentsOf, type Tabs, type ViewRef } from "./tabs";
import { zoomedOf } from "./tiling";

/**
 * **What a tab wears on the strip** (#4762; `prototype/one-tab-strip` variant A, accepted on
 * #4582): its focused pane's combined persona and state icon and its name, `⊟+N` where N other
 * panes share the tab, and every pane's name, in reading order, on hover.
 *
 * - A chat's pane: the chat's icon (`ChatPersonaState`) and the name its row has; the first
 *   pane, the tab's own name, which a rename sets and which says the task a pane shows in its
 *   chat's place (#1486), as before.
 * - A view's pane: the view's mark and its name, and no state: a view is doing nothing.
 * - A pane with nothing in it: **New chat**, what pressing in it would do.
 *
 * Pure, so what a tab says is one thing to test; the strip draws it.
 */
export type TabFace = {
  /** What the focused pane holds, which the tab's icon is. */
  lead: { kind: "chat"; session: number } | { kind: "view"; view: ViewRef } | { kind: "empty" };
  /** The focused pane's name. */
  name: string;
  /** How many other panes share the tab: `⊟+N`, none for a tab of one pane. */
  more: number;
  /** Every pane's name, in reading order (each split's first child first): the tooltip. */
  panes: string[];
  /** One pane is zoomed to fill the tab for now (#4765): the tab wears the zoom mark. */
  zoomed?: true;
};

/** What a pane is called: a chat as its row names it, a view by its title. A view with no
 *  name of its own, such as DevTools beside its browser, answers nothing, and is called what
 *  its tab is. */
export type Naming = {
  chat: (session: number) => string;
  view: (view: ViewRef, pane: number) => string | undefined;
};

/** What an empty pane's tab says. */
export const NEW_CHAT = "New chat";

type Lead = TabFace["lead"];

/** The face of tab `id`. A tab with no pane to read wears an empty pane's. */
export function faceOf(tabs: Tabs, id: number, naming: Naming): TabFace {
  const panes = contentsOf(tabs, id).map(({ pane, content }): { pane: number; lead: Lead } => {
    if (content.kind === "session")
      return { pane, lead: { kind: "chat", session: content.session } };
    if (content.kind === "view") return { pane, lead: { kind: "view", view: content.view } };
    return { pane, lead: { kind: "empty" } };
  });
  const tab = tabs.byId[id];
  // **The first pane is the tab's own**: its name is the tab's, which a rename sets and which
  // says the task a chat's pane shows in its place (#1486). The others are named for what
  // they hold.
  const first = panes[0]?.pane;
  const nameOf = ({ pane, lead }: { pane: number; lead: Lead }) =>
    tab !== undefined && pane === first
      ? tab.name
      : lead.kind === "chat"
        ? naming.chat(lead.session)
        : lead.kind === "view"
          ? (naming.view(lead.view, pane) ?? tab?.name ?? lead.view.view)
          : NEW_CHAT;
  const focused = panes.find((one) => one.pane === tab?.focused) ?? panes[0];
  if (focused === undefined)
    return { lead: { kind: "empty" }, name: NEW_CHAT, more: 0, panes: [NEW_CHAT] };
  return {
    lead: focused.lead,
    name: nameOf(focused),
    more: panes.length - 1,
    panes: panes.map(nameOf),
    ...(zoomedOf(tabs, id) === undefined ? {} : { zoomed: true }),
  };
}
