// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, type ReactNode } from "react";
import { dequal } from "dequal/lite";
import { useSyncExternalStoreWithSelector } from "use-sync-external-store/with-selector";
import type { Catalogued } from "./actions";

/**
 * **A project's catalogue, held outside the views that read it** (FS-W2, #7910, epic #7889).
 *
 * The view builds the catalogue again whenever one of its inputs moves, and a press on a tab is
 * one: the rows that switch to each tab, move it and close the others say which tab is in front.
 * Handed down as a prop, the new catalogue drew every view that reads any row of it, the Changes
 * view, the five panels and the explorer, though none of the rows they draw had moved.
 *
 * Held here instead, each reader takes the rows it reads by id through
 * `useSyncExternalStoreWithSelector` ([`useCatalogueRows`]), and is drawn again only when one of
 * those rows says something else. This is the shape `workspacePanels.ts` gave the panels (#1160).
 */
export class CatalogueRows {
  #listeners = new Set<() => void>();
  #rows: Catalogued;

  constructor(rows: Catalogued) {
    this.#rows = rows;
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  };

  /** The catalogue as it stands. */
  readonly now = (): Catalogued => this.#rows;

  /** Holds `rows` as the catalogue from now on, and tells every reader. */
  hold(rows: Catalogued): void {
    if (rows === this.#rows) return;
    this.#rows = rows;
    for (const listener of this.#listeners) listener();
  }
}

/** Which rows a reader reads, by id. */
export type RowsRead = (id: string) => boolean;

/**
 * **The rows of the catalogue a reader reads**, the ones `reads` keeps, in the catalogue's order.
 * The reader is drawn again only when one of them moved, was added or went away: what is handed
 * back is the same map until then.
 *
 * A row is compared by what it says (`dequal`), since the catalogue makes every row anew each
 * time it is built. `reads` is read at each draw, so a reader that reads other rows now (another
 * workspace, another repo) is handed those at once.
 */
export function useCatalogueRows(store: CatalogueRows, reads: RowsRead): Catalogued {
  const select = useCallback(
    (rows: Catalogued): Catalogued => new Map([...rows].filter(([id]) => reads(id))),
    [reads],
  );
  return useSyncExternalStoreWithSelector(store.subscribe, store.now, store.now, select, sameRows);
}

/**
 * **A view drawn from the rows it reads** ([`useCatalogueRows`]): `draw` is called with them,
 * and the map it is handed is the same one until one of them moves, so a view memoised on its
 * props is not drawn for a catalogue that moved elsewhere. `draw` is the caller's own closure,
 * as `WorkspaceRegion`'s is.
 */
export function CatalogueRegion({
  store,
  reads,
  draw,
}: {
  store: CatalogueRows;
  reads: RowsRead;
  draw: (rows: Catalogued) => ReactNode;
}): ReactNode {
  return draw(useCatalogueRows(store, reads));
}

/** Two selections the same: the same ids in the same order, each row saying the same. */
function sameRows(one: Catalogued, other: Catalogued): boolean {
  if (one.size !== other.size) return false;
  const others = other.entries();
  for (const [id, row] of one) {
    const [otherId, otherRow] = others.next().value ?? [];
    if (id !== otherId || !dequal(row, otherRow)) return false;
  }
  return true;
}
