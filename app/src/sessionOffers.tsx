// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useEffect, useMemo, type ReactNode } from "react";
import {
  catalogued,
  sessionOffers,
  type Catalogued,
  type ListedRecord,
  type Offer,
} from "./actions";
import type { PlaneId } from "./bindings";
import { useWorkspacePanels, type WorkspacePanels } from "./workspacePanels";

/**
 * **The session rows of the place in front, outside the project view** (#1160, D-1160-3).
 *
 * A record saved in the workspace in front is drawn by the Sessions panel that lists it. Its
 * Open and Resume rows were the catalogue's, and the catalogue is built by the project view, so
 * every record written drew that whole view again for two palette rows. They are built here
 * now, from the panels' own store: what draws them — the Sessions panel's menu, a record tab's
 * Resume, the palette — reads them from here, and the project view is not drawn for them.
 */
export const SessionOffers = createContext<Catalogued>(new Map());

/** The session rows lent below, by id: what a Sessions row's menu and a record's tab read. */
export function useSessionOffers(): Catalogued {
  return useContext(SessionOffers);
}

const NONE: readonly ListedRecord[] = [];

/**
 * Lends the session rows of the place in front to everything under it, and hands them up for
 * the palette (`onRows`). The focused workspace's records come from its panels' store, read
 * here and not in the project view; the plane root's are the view's own (`rootRecords`), which
 * it reads anyway while the root is in front.
 *
 * **Its children are the view's**: drawn again by the view, never by a record written here, so
 * only what reads the rows is drawn for one.
 */
export function SessionOffersLent({
  plane,
  store,
  workspace,
  atRoot,
  rootRecords,
  onRows,
  children,
}: {
  plane: PlaneId;
  store: WorkspacePanels;
  /** The focused workspace, or `undefined` while none is. */
  workspace: string | undefined;
  /** Whether the plane root is the place in front. */
  atRoot: boolean;
  rootRecords: readonly ListedRecord[] | undefined;
  /** Hands the rows to the window, for its palette. */
  onRows?: (plane: PlaneId, rows: readonly Offer[]) => void;
  children: ReactNode;
}) {
  const inWorkspace = useWorkspacePanels(store, workspace, (state) => state.panels?.sessions);
  const records = (atRoot ? rootRecords : inWorkspace) ?? NONE;
  const rows = useMemo(() => sessionOffers(records), [records]);
  const found = useMemo(() => catalogued(rows), [rows]);
  useEffect(() => {
    onRows?.(plane, rows);
  }, [onRows, plane, rows]);
  // Gone with the view: the window keeps no rows for a project it no longer draws.
  useEffect(() => () => onRows?.(plane, []), [onRows, plane]);
  return <SessionOffers.Provider value={found}>{children}</SessionOffers.Provider>;
}

/**
 * The catalogue with the session rows in it, for the palette: a row of the same id — the
 * generic Resume of an open record's tab — gives way to the place's own, which says more. One
 * row per id either way.
 */
export function withSessionRows(offers: readonly Offer[], rows: readonly Offer[]): Offer[] {
  if (rows.length === 0) return offers as Offer[];
  const theirs = new Set(rows.map((row) => row.id));
  return [...offers.filter((offer) => !theirs.has(offer.id)), ...rows];
}
