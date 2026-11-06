// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { pictureOf, type Picture } from "./layoutPicture";
import { contentsOf, tabNameOf, type Tabs, type ViewRef } from "./tabs";
import { zoomedOf } from "./tiling";

/**
 * **What a tab wears on the strip** (#4762; `prototype/one-tab-strip` variant A, accepted on
 * #4582; amended by #5342, spec #5339): **its name is its first pane's** ({@link tabNameOf}),
 * whichever pane has the keyboard, so a split or a move between panes never renames it; **its
 * mark is its most urgent pane's**, chosen from {@link TabFace.chats} by the one state order
 * (`shownState.urgentOf`), so a pane that needs the person can't hide behind the focused one;
 * a small picture of its layout where it is split (`layoutPicture.ts`); and, on hover, how many
 * panes it holds and every pane's name, in reading order.
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
  /** What the first pane holds: the tab's icon where no pane's chat has a state to show. */
  lead: { kind: "chat"; session: number } | { kind: "view"; view: ViewRef } | { kind: "empty" };
  /** The tab's name: its own, or its first pane's. */
  name: string;
  /**
   * **Every chat its panes hold, in reading order**, for its mark to choose the most urgent of:
   * each pane's own chat, then a task it shows in that chat's place (#1486), so a task on screen
   * reaches the tab's mark (#5342). No view and no empty pane: neither has a state.
   */
  chats: number[];
  /** **A split tab's small picture of its layout** (#5342), drawn in place of `+N`; none for a
   *  tab of one pane. */
  picture?: Picture;
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
  const lead = panes[0];
  if (lead === undefined)
    return { lead: { kind: "empty" }, name: NEW_CHAT, chats: [], panes: [NEW_CHAT] };
  const chats = panes.flatMap(({ pane, lead }) => {
    if (lead.kind !== "chat") return [];
    const shown = tab?.shows?.[pane];
    return shown === undefined || shown === lead.session ? [lead.session] : [lead.session, shown];
  });
  const picture = tab === undefined ? undefined : pictureOf(tab.layout);
  return {
    lead: lead.lead,
    name: tab === undefined ? NEW_CHAT : tabNameOf(tab),
    chats,
    panes: panes.map(nameOf),
    ...(picture === undefined ? {} : { picture }),
    ...(zoomedOf(tabs, id) === undefined ? {} : { zoomed: true }),
  };
}
