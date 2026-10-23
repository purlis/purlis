// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import type { ViewRef } from "./tabs";

/**
 * **Open on a runner chat's change too long for its row** (#2046): it asks the project's window
 * to open the change's own tab, which shows it whole and has Apply where it may be applied. A
 * Notice on a pane and an Inbox row are drawn far from the window's tabs, so they ask by an
 * event the window hears, as Open on a proposed change does (`proposedChangeOpen.ts`).
 */
export const OPEN_RUNNER_WRITE_ASK = "purlis-open-runner-write";

/** What a row asked of a project's window: which project, and which held change. */
export type OpenRunnerWriteAsk = { plane: string; id: number };

/** The view a runner chat's change is drawn in. */
export const RUNNER_WRITE_VIEW = "runner-write";

/** The view of held change `id`. */
export const runnerWriteView = (id: number): ViewRef => ({
  from: null,
  view: RUNNER_WRITE_VIEW,
  key: String(id),
});

/** What its tab says. */
export const RUNNER_WRITE_TITLE = "Change from a runner";

/** Asks the window of project `plane` to open held change `id`'s tab. */
export function askOpenRunnerWrite(plane: string, id: number): void {
  window.dispatchEvent(
    new CustomEvent<OpenRunnerWriteAsk>(OPEN_RUNNER_WRITE_ASK, { detail: { plane, id } }),
  );
}

/** Opens a held change's tab through `open` whenever a row of `plane` asks for it. */
export function useOpenRunnerWriteAsked(
  plane: string,
  open: (view: ViewRef, title: string) => void,
): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<OpenRunnerWriteAsk>).detail;
      if (ask?.plane === plane && typeof ask.id === "number")
        open(runnerWriteView(ask.id), RUNNER_WRITE_TITLE);
    };
    window.addEventListener(OPEN_RUNNER_WRITE_ASK, heard);
    return () => window.removeEventListener(OPEN_RUNNER_WRITE_ASK, heard);
  }, [plane, open]);
}
