// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useSyncExternalStore } from "react";

import type { PlaneId, WindowDraft } from "./bindings";
import { onAMac } from "./tabKeys";

/**
 * **The theme draft the window draws now** (#2332, spec #2313): what the core's last answer to
 * `theme_in_force` said of the drafts of the chats working where the window is. The core decides
 * everything about a draft (`purlis_core::theme::drafts`); the window only names it on the
 * workspace's strip and offers Keep, Revert and Compare.
 */
export type DraftHere = {
  plane: PlaneId;
  /** The workspace it is drawn in, `null` at the plane root. */
  workspace: string | null;
  draft: WindowDraft;
};

let current: DraftHere | null = null;
const listeners = new Set<() => void>();

/** What the core said of the draft where the window is: `null` when there is none. */
export function setDraftHere(next: DraftHere | null): void {
  if (sameDraft(current, next)) return;
  current = next;
  for (const one of listeners) one();
}

function sameDraft(a: DraftHere | null, b: DraftHere | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.plane === b.plane &&
    a.workspace === b.workspace &&
    JSON.stringify(a.draft) === JSON.stringify(b.draft)
  );
}

/** The draft where the window is, as last said. */
export function draftHere(): DraftHere | null {
  return current;
}

/** {@link draftHere}, as a component reads it. */
export function useDraftHere(): DraftHere | null {
  return useSyncExternalStore(
    (changed) => {
      listeners.add(changed);
      return () => listeners.delete(changed);
    },
    () => current,
    () => current,
  );
}

/**
 * **The revert key** (spec #2313): ⌘⇧⌫ on macOS, Ctrl+Shift+Backspace elsewhere. Reserved: it
 * drops the drafts where the window is at once, even when a draft has made the window hard to
 * read.
 */
export function isRevertKey(
  event: Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">,
  mac: boolean = onAMac(),
): boolean {
  if (event.key !== "Backspace" || !event.shiftKey || event.altKey) return false;
  return mac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

/** The revert key as a person reads it. */
export function revertKeySaid(mac: boolean = onAMac()): string {
  return mac ? "⌘⇧⌫" : "Ctrl+Shift+Backspace";
}

/** Where Keep puts `draft`, in the words the strip and the close question say it in. */
export function keptToSaid(draft: Pick<WindowDraft, "kept_to" | "workspace">): string {
  return draft.kept_to === "project"
    ? "Keep puts it in the project's themes, for every workspace"
    : `Keep puts it in the workspace ${draft.workspace ?? ""}'s themes`;
}

/**
 * Keep's own words for `draft`: where it goes, and what it replaces there; and, where the folder
 * it replaces is not what the draft started from (#2708), that it changed since.
 */
export function keepSays(
  draft: Pick<WindowDraft, "kept_to" | "workspace" | "replaces" | "folder"> &
    Partial<Pick<WindowDraft, "changed">>,
): string {
  const since = draft.changed === true ? ", which changed since this draft started" : "";
  const replacing = draft.replaces ? `, replacing the ${draft.folder} that is there${since}` : "";
  return `${keptToSaid(draft)}${replacing}.`;
}

/** For tests: no draft. */
export function forgetDraftHere(): void {
  current = null;
  listeners.clear();
}
