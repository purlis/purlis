// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act } from "@testing-library/react";
import { tipOf } from "../tooltip/tooltip.testkit";

/**
 * **A data table's cells, as a test reads them** (#2911): a table of data is a grid, so a row's
 * cells are the cell that names it (`rowheader`) and its `gridcell`s, in the columns' order.
 */
export function cellsOf(row: Element): HTMLElement[] {
  return Array.from(row.querySelectorAll<HTMLElement>('[role="rowheader"], [role="gridcell"]'));
}

/**
 * **What a cell's mark or button says in its tooltip** (`tipOf`), read as the keyboard reaches
 * it in a grid: along its row. A grid hands the keyboard coming from outside it back to its row,
 * as one Tab stop does, so the row is focused first and the read is made from inside the grid.
 */
export function tipInTable(target: Element | null | undefined): string | null {
  const row = target?.closest<HTMLElement>('[role="row"]');
  if (row !== null && row !== undefined) act(() => row.focus());
  return tipOf(target);
}
