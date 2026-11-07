// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { chatOf, contentsOf, type Tabs } from "./tabs";

/**
 * **What a tab's Rename renames** (#5343, spec #5339), from what the tab holds and nothing else:
 *
 * - **one chat**: the chat, since the tab shows its name; the dialog's "Only this tab" names the
 *   tab instead ({@link nameTab});
 * - **several panes**: the tab only, whatever the panes hold, so a chat is never renamed by
 *   accident;
 * - **a view or an empty pane alone**: nothing. A view's tab says the view's own name.
 *
 * A split that closes back to one chat is a one-chat tab again: this reads the layout as it is.
 */
export type TabRenaming = { kind: "chat"; session: number } | { kind: "tab"; panes: number };

export function renameOf(tabs: Tabs, id: number): TabRenaming | undefined {
  const held = contentsOf(tabs, id);
  if (held.length > 1) return { kind: "tab", panes: held.length };
  const session = chatOf(tabs, id);
  return session === undefined ? undefined : { kind: "chat", session };
}

/**
 * Tab `id` with `own` as **its own name** (`Tab.own`), which no chat's rename, split or close
 * replaces; nothing, or a blank, clears it, and the tab is its first pane's again. Window
 * state: the core is told nothing. Answers `tabs` itself where nothing changes.
 */
export function nameTab(tabs: Tabs, id: number, own: string | undefined): Tabs {
  const tab = tabs.byId[id];
  if (tab === undefined) return tabs;
  const given = own?.trim() || undefined;
  if (tab.own === given) return tabs;
  const { own: _was, ...rest } = tab;
  return { ...tabs, byId: { ...tabs.byId, [id]: given === undefined ? rest : { ...rest, own: given } } };
}

/**
 * **`tabs` after chat `session` was renamed `label`** (the core's answer: a blank is `null`, the
 * default back): every tab whose first pane is that chat is called by it, as a tab is called by
 * its first pane (`tabNameOf`). A tab it is only a later pane of keeps its name, and a tab's own
 * name stays over it. Answers `tabs` itself where no tab is named for that chat.
 */
export function chatRenamed(tabs: Tabs, session: number, label: string | null): Tabs {
  let byId: Tabs["byId"] | undefined;
  for (const id of tabs.order) {
    const tab = tabs.byId[id];
    if (chatOf(tabs, id) !== session) continue;
    const name = label ?? tab.defaultName;
    if (tab.name === name) continue;
    byId ??= { ...tabs.byId };
    byId[id] = { ...tab, name };
  }
  return byId === undefined ? tabs : { ...tabs, byId };
}
