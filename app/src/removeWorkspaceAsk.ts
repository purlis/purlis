// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";

/**
 * **A question asks the project's window to open Delete workspace** (qw356): the window's own
 * dialog, which says what goes and that there is no undo, lists what the workspace holds that
 * nothing else has, and is the one place a workspace is removed from, on the person's press.
 * Remove… on a chat's question about a workspace opens it, from the chat's pane or the Inbox,
 * and removes nothing itself.
 *
 * A Notice on a pane is drawn far from the window's own state, so it asks by an event the
 * project's window hears, as a link out of Settings does (`settings/links.ts`). It carries a
 * project and a workspace's name; the dialog reads everything else from the core.
 */
export const REMOVE_WORKSPACE_ASK = "purlis-remove-workspace-ask";

/** What a Notice asked of a project's window: which project, and the workspace. */
export type RemoveWorkspaceAsk = { plane: string; workspace: string };

/** Asks the window of the project `plane` to open Delete workspace on `workspace`. */
export function askRemoveWorkspace(plane: string, workspace: string): void {
  window.dispatchEvent(
    new CustomEvent<RemoveWorkspaceAsk>(REMOVE_WORKSPACE_ASK, { detail: { plane, workspace } }),
  );
}

/** Opens Delete workspace through `open` whenever a Notice of `plane` asks for it. */
export function useRemoveWorkspaceAsked(plane: string, open: (workspace: string) => void): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<RemoveWorkspaceAsk>).detail;
      if (ask?.plane === plane && typeof ask.workspace === "string") open(ask.workspace);
    };
    window.addEventListener(REMOVE_WORKSPACE_ASK, heard);
    return () => window.removeEventListener(REMOVE_WORKSPACE_ASK, heard);
  }, [plane, open]);
}
