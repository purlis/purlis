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
import type { About } from "./planeChanged";
import type { ViewRef } from "./tabs";

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
 * **A change to the template of artifact folder `folder` of workspace `workspace`** (ADR 0120
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
