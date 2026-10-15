// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from "react";
import type { ViewRef } from "./tabs";

/**
 * **Open on a proposed change too long for a row** (#1975, PC-7): it asks the project's window
 * to open the change's own tab, which shows it whole and has Apply. A Notice on a pane and an
 * Inbox row are drawn far from the window's tabs, so they ask by an event the window hears, as
 * Remove… on a workspace's question does (`removeWorkspaceAsk.ts`).
 */
export const OPEN_PROPOSAL_ASK = "purlis-open-proposed-change";

/** What a row asked of a project's window: which project, which proposal, and its file. */
export type OpenProposalAsk = { plane: string; id: number; path: string };

/** The view of proposal `id`. */
export const proposalView = (id: number): ViewRef => ({
  from: null,
  view: PROPOSED_CHANGE_VIEW,
  key: String(id),
});

/** The view a proposed change is drawn in. */
export const PROPOSED_CHANGE_VIEW = "proposed-change";

/** What its tab says. */
export const proposalTitle = (path: string): string => `Change to ${path}`;

/** Asks the window of project `plane` to open proposal `id`'s tab. */
export function askOpenProposal(plane: string, id: number, path: string): void {
  window.dispatchEvent(
    new CustomEvent<OpenProposalAsk>(OPEN_PROPOSAL_ASK, { detail: { plane, id, path } }),
  );
}

/** Opens a proposal's tab through `open` whenever a row of `plane` asks for it. */
export function useOpenProposalAsked(
  plane: string,
  open: (view: ViewRef, title: string) => void,
): void {
  useEffect(() => {
    const heard = (event: Event) => {
      const ask = (event as CustomEvent<OpenProposalAsk>).detail;
      if (ask?.plane === plane && typeof ask.id === "number" && typeof ask.path === "string")
        open(proposalView(ask.id), proposalTitle(ask.path));
    };
    window.addEventListener(OPEN_PROPOSAL_ASK, heard);
    return () => window.removeEventListener(OPEN_PROPOSAL_ASK, heard);
  }, [plane, open]);
}
