// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pinned artifact's tab** (ADR 0122 §2, #5952): a view purlis draws itself,
 * `{ from: null, view: "artifact", key }`, opened through `openView` like any view, so pressing
 * its icon again brings its tab forward. A private tab: the tab is this machine's (ADR 0040),
 * the pin the workspace's.
 *
 * **Its key names the workspace and the folder** (`<workspace>/<folder>`): a view's key is
 * unique per project (`tabs.viewKey`), and two workspaces may each pin a folder of one name.
 * A workspace's name holds no `/` (`purlis_core::contain::workspace_name_ok`), so the first `/`
 * parts the two.
 */
import type { ArtifactFresh, ArtifactTheme } from "./bindings";
import type { About } from "./planeChanged";
import type { ViewRef } from "./tabs";
import { property, TOKENS } from "./theme/theme";
import { crosses } from "./ui/popover/offPage";

/** The view's name, as `ViewRef.view`. */
export const ARTIFACT_VIEW = "artifact";

/** The view of artifact folder `folder` of workspace `workspace`. */
export function artifactView(workspace: string, folder: string): ViewRef {
  return { from: null, view: ARTIFACT_VIEW, key: `${workspace}/${folder}` };
}

/** The workspace and folder an artifact's view is of, or nothing for any other view. */
export function artifactOf(view: ViewRef): { workspace: string; folder: string } | undefined {
  if (view.from !== null || view.view !== ARTIFACT_VIEW) return undefined;
  const at = view.key.indexOf("/");
  if (at <= 0 || at === view.key.length - 1) return undefined;
  return { workspace: view.key.slice(0, at), folder: view.key.slice(at + 1) };
}

/** What the window hears when Esc or F6 is pressed in an artifact's page (`artifacts.rs`). */
export const ARTIFACT_KEY = "artifact-key";

/** Which of purlis's keys was pressed in the page. */
export type ArtifactKeyPressed = { label: string; key: "escape" | "f6" | "f6-back" };

/**
 * **Whether something purlis draws covers the page in `frame`** (#6226). A native webview is
 * drawn above the window's own page, so it would cover whatever the window draws over it: it is
 * put away while
 *
 * - **a modal dialog or an alert dialog is up**, wherever it stands: its scrim dims the whole
 *   window and holds the keyboard, and a live page over it would do neither;
 * - **a menu, popover or non-modal dialog crosses the frame**: one opened from the page's own
 *   bar, which hangs over the page, or one from elsewhere the popover family could not keep
 *   clear of it (`keptOffPages`), which would otherwise be drawn under the page.
 *
 * A surface kept clear of the frame, as a chat row's card beside the sidebar is, leaves the page
 * shown, and a tooltip is never counted: it comes and goes under the pointer.
 */
export function coveredByAnOverlay(frame: DOMRect, root: ParentNode = document): boolean {
  for (const one of root.querySelectorAll(MODALS)) {
    const at = one.getBoundingClientRect();
    if (at.width > 0 && at.height > 0) return true;
  }
  for (const one of root.querySelectorAll(SURFACES)) {
    const at = one.getBoundingClientRect();
    if (at.width > 0 && at.height > 0 && crosses(at, frame)) return true;
  }
  return false;
}

const MODALS = [
  '[data-scope="dialog"][data-part="scrim"]',
  '[data-scope="dialog"][data-part="modal"]',
  '[role="alertdialog"]',
  '[role="dialog"][aria-modal="true"]',
].join(",");

const SURFACES = ['[role="dialog"]', '[role="menu"]', '[data-scope="popover"]'].join(",");

/** The most icons an artifact group draws (ADR 0122 §2); more go to its show-more (#5953). */
export const MOST_DRAWN = 8;

/**
 * **The pins an artifact group draws, and the ones past them** (#5953): at most
 * {@link MOST_DRAWN} drawn, in order; every pin past them, which a merge can bring, kept for
 * the group's show-more, so none is ever dropped.
 */
export function drawnAndMore<T>(pins: readonly T[]): { drawn: T[]; more: T[] } {
  return { drawn: pins.slice(0, MOST_DRAWN), more: pins.slice(MOST_DRAWN) };
}

/** **The order after pin `id` moves one place** (`-1` up, `1` down), or nothing at an end. */
export function movedBy(ids: readonly string[], id: string, by: -1 | 1): string[] | undefined {
  const from = ids.indexOf(id);
  const to = from + by;
  if (from < 0 || to < 0 || to >= ids.length) return undefined;
  const order = [...ids];
  [order[from], order[to]] = [order[to], order[from]];
  return order;
}

/** **The order after pin `moved` is dropped on pin `onto`**: where it landed, or nothing. */
export function pinOrderAfterDrop(
  ids: readonly string[],
  moved: string,
  onto: string,
): string[] | undefined {
  const from = ids.indexOf(moved);
  const to = ids.indexOf(onto);
  if (from < 0 || to < 0 || from === to) return undefined;
  const order = ids.filter((one) => one !== moved);
  order.splice(to, 0, moved);
  return order;
}

/** What every window hears when a window changed a workspace's pins (`artifacts.rs`). */
export const ARTIFACT_PINS_MOVED = "artifact-pins-moved";

/** Which workspace's pins changed. */
export type ArtifactPinsMoved = { plane: string; workspace: string };
/**
 * **The theme the window draws now, as an artifact's page is handed it** (#5955): read off the
 * document's root, where `theme.apply` writes it, so the page is drawn in what the person sees,
 * tint and contrast included. Its appearance is purlis's, never the operating system's.
 */
export function drawnTheme(root: HTMLElement = document.documentElement): ArtifactTheme {
  const values: Record<string, string> = {};
  for (const token of TOKENS) {
    const value = root.style.getPropertyValue(property(token)).trim();
    if (value !== "") values[token] = value;
  }
  return { appearance: root.style.colorScheme === "light" ? "light" : "dark", values };
}

/** Calls `told` with the theme the window draws each time it draws another one. */
export function followDrawnTheme(
  told: (theme: ArtifactTheme) => void,
  root: HTMLElement = document.documentElement,
): () => void {
  let was = JSON.stringify(drawnTheme(root));
  const watch = new MutationObserver(() => {
    const now = drawnTheme(root);
    const key = JSON.stringify(now);
    if (key === was) return;
    was = key;
    told(now);
  });
  watch.observe(root, { attributes: true, attributeFilter: ["style"] });
  return () => watch.disconnect();
}

/**
 * **What an artifact's freshness says, and whether it is stale at `now`** (#5955): "Updated 3
 * hours ago", from the time its data declares, else its commit or write time. Nothing for an
 * artifact whose freshness could not be read.
 */
export function freshness(
  fresh: ArtifactFresh | null | undefined,
  now: number = Date.now(),
): { said: string; stale: boolean } | undefined {
  if (fresh === null || fresh === undefined || fresh.at === null) return undefined;
  const stale = fresh.stale_at !== null && now >= fresh.stale_at;
  return { said: `Updated ${ago(now - fresh.at)}`, stale };
}

/** How long ago `ms` milliseconds is, in words. */
function ago(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return "just now";
  const say = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"} ago`;
  if (minutes < 60) return say(minutes, "minute");
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return say(hours, "hour");
  return say(Math.floor(hours / 24), "day");
}

/**
 * **What every window of a plane hears when an artifact's approval changed on this machine**
 * (`artifacts.rs`, #5959): each tab of it loads its page again, under its new standing.
 */
export const APPROVALS_CHANGED = "artifact-approvals-changed";

/** Which artifact's approval changed. */
export type ArtifactApprovalChanged = { workspace: string; folder: string };

/** Whether `heard` is of the artifact folder `folder` of workspace `workspace`. */
export function approvalOf(
  heard: ArtifactApprovalChanged,
  workspace: string,
  folder: string,
): boolean {
  return heard.workspace === workspace && heard.folder === folder;
}

/**
 * **A change to the template of artifact folder `folder` of workspace `workspace`** (ADR 0122
 * §4): any file of its folder but its data (`data.json`, `data/`), which never asks again. Its
 * page is torn down and opened again, without its script until approved anew.
 */
export function templateOf(workspace: string, folder: string): About {
  const inside = `workspaces/${workspace}/artifacts/${folder}/`;
  return (change) => {
    if (!change.path.startsWith(inside)) return false;
    const key = change.path.slice(inside.length);
    return key !== "data.json" && !key.startsWith("data/");
  };
}
