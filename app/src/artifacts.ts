// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A pinned artifact's tab** (ADR 0120 §2, #5952): a view purlis draws itself,
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
import type { ViewRef } from "./tabs";
import { property, TOKENS } from "./theme/theme";

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
 * **Whether something purlis draws floats over `frame`**: a dialog, a menu, a popover's list.
 * A native webview is drawn above the window's page, so it would cover them; it is put away
 * while one is over its frame. A tooltip is not counted: it comes and goes under the pointer.
 */
export function coveredByAnOverlay(frame: DOMRect, root: ParentNode = document): boolean {
  for (const one of root.querySelectorAll(OVERLAYS)) {
    const at = one.getBoundingClientRect();
    if (at.width === 0 || at.height === 0) continue;
    const apart =
      at.right <= frame.left ||
      at.left >= frame.right ||
      at.bottom <= frame.top ||
      at.top >= frame.bottom;
    if (!apart) return true;
  }
  return false;
}

const OVERLAYS = [
  '[role="dialog"]',
  '[role="alertdialog"]',
  '[role="menu"]',
  '[data-scope="popover"]',
  '[data-scope="dialog"][data-part="modal"]',
].join(",");

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
