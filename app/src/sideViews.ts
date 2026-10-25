// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The views a side shows, one at a time** (ADR 0038 as amended 2026-10-10, #1673).
 *
 * A module of its own, with nothing imported, so the catalogue (`actions.ts`) can name a view on
 * its palette rows without importing the layout module, which imports what imports the
 * catalogue. Where each view is and which is open is `regions.ts`'s.
 */
/**
 * **One of purlis's own views**: the left side's Chats and Explorer (#1673), Search and Changes
 * (#1676), and Repos in the left half of the Panel (#1737), Changes beside it since #1739, and
 * the right side's Inbox (#1692), Personas and Vaults (#1678), with Memory, Todos and Sessions in
 * the right half of the Panel (#1738). Data, as a region is: the catalogue says which region
 * holds it, the placement says which is open, and
 * `RegionFrame` draws the bar and the views from that.
 */
export type OwnViewId =
  | "chats"
  | "explorer"
  | "search"
  | "skills"
  | "changes"
  | "repos"
  | "inbox"
  | "todos"
  | "memory"
  | "personas"
  | "sessions"
  | "vaults";

/**
 * **An approved extension's panel, as a view of its own** (#1678, spec #1671 story 33): its
 * panel key (`ext/<extension>/<id>`, `purlis_core::panel::Panel::key`) behind `panel:`. Not a
 * closed set: which panels exist is what the approved extensions contribute, so the bar learns
 * them as the window does and the layout file keeps one by name.
 */
export type PanelViewId = `panel:${string}`;

/** A view of a side: purlis's own, or an extension's panel. */
export type ViewId = OwnViewId | PanelViewId;

/** Where a view can be on its side (#1740, P-3): its sidebar, or its half of the Panel. */
export type ViewPlace = "sidebar" | "panel";

/** The view that shows the panel `key`. */
export const panelView = (key: string): PanelViewId => `panel:${key}`;

/** The panel key a panel's view shows. */
export const panelKeyOf = (view: PanelViewId): string => view.slice("panel:".length);

/** Whether `view` is an extension's panel rather than one of purlis's own views. */
export const isPanelView = (view: string): view is PanelViewId =>
  view.startsWith("panel:") && view.length > "panel:".length;

/**
 * **purlis's own views behind a preview** (SK-55, #3266): the Skills view, only while the Skill
 * studio's preview is on. While it is off the view is drawn nowhere and its place keeps it, as an
 * extension's panel is kept while its extension contributes nothing: the side opens on its first
 * view, and the person's pick comes back with the preview.
 */
export const PREVIEWED_VIEWS: readonly OwnViewId[] = ["skills"];

/** **Whether `view` is drawn only while it is present now**: an extension's panel while it is
 *  contributed, a view behind a preview while that preview is on. */
export const drawnOnlyPresent = (view: string): boolean =>
  isPanelView(view) || (PREVIEWED_VIEWS as readonly string[]).includes(view);

/** The right sidebar's own views, in its bar's order (#1678, #1738): the attention region's. The
 *  Inbox is first (#1692, I-2): what waits on the person heads the "for you" side, and the side
 *  opens on it. */
export const RIGHT_SIDEBAR_VIEWS: readonly OwnViewId[] = ["inbox", "personas", "vaults"];

/** The right side's views in its half of the Panel, in its Lower icons' order (#1738, P-2):
 *  what you keep, out of the sidebar so it never hides what is asking for you. Memory first, as
 *  the half opens on it (P-14). */
export const RIGHT_HALF_VIEWS: readonly OwnViewId[] = ["memory", "todos", "sessions"];

/** **Every one of the right side's own views**, wherever it lives: the sidebar's and the Panel
 *  half's. What the window draws a right-hand panel for. */
export const ATTENTION_VIEWS: readonly OwnViewId[] = [...RIGHT_SIDEBAR_VIEWS, ...RIGHT_HALF_VIEWS];

/** What a view is called: its tab's `aria-label`, and the palette row that opens it. */
export const VIEWS: Record<OwnViewId, { name: string }> = {
  chats: { name: "Chats" },
  explorer: { name: "Explorer" },
  // #1676: the Search tab's content, in the side; and the repos' state, which the bottom
  // region drew until it went (B-7).
  search: { name: "Search" },
  // #3266: this workspace's skills by level, behind the Skill studio's preview.
  skills: { name: "Skills" },
  changes: { name: "Changes" },
  // #1737: Explorer's *Repos and branches*, a view of its own in the left half of the Panel.
  repos: { name: "Repos" },
  inbox: { name: "Inbox" },
  todos: { name: "Todos" },
  memory: { name: "Memory" },
  personas: { name: "Personas" },
  sessions: { name: "Sessions" },
  vaults: { name: "Vaults" },
};
