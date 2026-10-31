// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The table family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/table-family` (error: the table family has moved, #2911): a raw table element
 * (`<table>`, `<tr>`, `<td>` and the rest), a table's or a grid's role written by hand, the old
 * table classes (`ui-table`, `ui-table-*`, `ui-cell-actions`, `ui-num`) and `data-scope="table"`.
 * A table of data is the family's `Table` (React Aria's grid) and a table of controls its
 * `ControlTable` (a plain table whose fields keep their keys).
 */

import { moved } from "../familyLint.js";

const tableFamily = moved({
  description: "A table is the shared table family",
  scopes: ["table"],
  classes: ["ui-table", "ui-table-*", "ui-cell-actions", "ui-num"],
  roles: ["table", "grid", "row", "rowgroup", "cell", "gridcell", "columnheader", "rowheader"],
  elements: ["table", "thead", "tbody", "tfoot", "tr", "th", "td", "caption", "colgroup", "col"],
  message:
    "a table: draw a table of data as `<Table>`, `<TableHeader>`, `<TableColumn>`, `<TableBody>`, `<TableRow>` and `<TableCell>` from `ui/table/Table` (with `<TableBar>` over one that chooses), and a table of controls as `<ControlTable>` and its parts from `ui/table/ControlTable` (#2911).",
});

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "table-family": tableFamily,
};
