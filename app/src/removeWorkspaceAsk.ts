// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";

/**
 * **A Notice asks the project's window to open Delete workspace** (qw356): the window's own
 * dialog, which lists what the workspace holds that nothing else has and is where a removal
 * the core refused is read and, on a second press, forced.
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

/**
 * **A workspace was removed on a chat's question** (qw356), from its Notice or the Inbox: the
 * project's window is told, with the words the core said of the removal, so what follows a
 * removal from the window's own dialog follows this one too.
 */
export const WORKSPACE_REMOVED_ON_ASK = "purlis-workspace-removed-on-ask";

/** What the window is told of it: which project, the workspace, and the core's words. */
export type WorkspaceRemovedOnAsk = { plane: string; workspace: string; words: readonly string[] };

/** Tells the window of the project `plane` that `workspace` was removed, and what was said. */
export function tellWorkspaceRemoved(
  plane: string,
  workspace: string,
  words: readonly string[],
): void {
  window.dispatchEvent(
    new CustomEvent<WorkspaceRemovedOnAsk>(WORKSPACE_REMOVED_ON_ASK, {
      detail: { plane, workspace, words },
    }),
  );
}

/** Runs `after` whenever a workspace of `plane` was removed on a chat's question. */
export function useWorkspaceRemovedOnAsk(
  plane: string,
  after: (workspace: string, words: readonly string[]) => void,
): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const told = (event as CustomEvent<WorkspaceRemovedOnAsk>).detail;
      if (told?.plane === plane && typeof told.workspace === "string" && Array.isArray(told.words))
        after(told.workspace, told.words);
    };
    window.addEventListener(WORKSPACE_REMOVED_ON_ASK, heard);
    return () => window.removeEventListener(WORKSPACE_REMOVED_ON_ASK, heard);
  }, [plane, after]);
}
