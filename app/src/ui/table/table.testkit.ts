// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, waitFor, type screen } from "@testing-library/react";
import { tipOf } from "../tooltip/tooltip.testkit";

/**
 * **A data table found by its name once a row is in it** (#6372): React Aria draws the grid
 * a render before it fills in its collection, so a grid that `findByRole` returns may hold no
 * rows yet, and a test that reads its headers or cells at once reads an empty grid. This finds
 * the grid, then waits, inside `act`, until a cell of a row is in it: a table's rows are headed
 * (`rowheader`), a list's are not, and both hold `gridcell`s.
 */
export async function findGrid(
  scope: Pick<typeof screen, "findByRole">,
  name: string,
): Promise<HTMLElement> {
  const grid = await scope.findByRole("grid", { name });
  await waitFor(() => {
    if (cellsOf(grid).length === 0) throw new Error(`the grid "${name}" has no rows yet`);
  });
  return grid;
}

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
