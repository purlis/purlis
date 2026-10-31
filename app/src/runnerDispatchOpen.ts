// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import type { ViewRef } from "./tabs";

/**
 * **Open on a runner chat's held open or dispatch**: it asks the project's window to open the
 * ask's own tab, which shows it whole with Approve. A row cuts the brief, so Approve is pressed
 * only there. Asked by an event the window hears, as Open on a runner chat's change is
 * (`runnerWriteOpen.ts`).
 */
export const OPEN_RUNNER_DISPATCH_ASK = "purlis-open-runner-dispatch";

/** What a row asked of a project's window: which project, and which held ask. */
export type OpenRunnerDispatchAsk = { plane: string; id: number };

/** The view a runner chat's held open or dispatch is drawn in. */
export const RUNNER_DISPATCH_VIEW = "runner-dispatch";

/** The view of held ask `id`. */
export const runnerDispatchView = (id: number): ViewRef => ({
  from: null,
  view: RUNNER_DISPATCH_VIEW,
  key: String(id),
});

/** What its tab says. */
export const RUNNER_DISPATCH_TITLE = "Dispatch from a runner";

/** Asks the window of project `plane` to open held ask `id`'s tab. */
export function askOpenRunnerDispatch(plane: string, id: number): void {
  window.dispatchEvent(
    new CustomEvent<OpenRunnerDispatchAsk>(OPEN_RUNNER_DISPATCH_ASK, { detail: { plane, id } }),
  );
}

/** Opens a held ask's tab through `open` whenever a row of `plane` asks for it. */
export function useOpenRunnerDispatchAsked(
  plane: string,
  open: (view: ViewRef, title: string) => void,
): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<OpenRunnerDispatchAsk>).detail;
      if (ask?.plane === plane && typeof ask.id === "number")
        open(runnerDispatchView(ask.id), RUNNER_DISPATCH_TITLE);
    };
    window.addEventListener(OPEN_RUNNER_DISPATCH_ASK, heard);
    return () => window.removeEventListener(OPEN_RUNNER_DISPATCH_ASK, heard);
  }, [plane, open]);
}
