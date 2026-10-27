// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import { ButtonRow } from "../button/Button";

/**
 * **The bar over a table of many rows** (#2911, R65 "collections are tables"; the settings
 * prototype's hosts table): its filter at the start (`ListFilter`, from eight rows, R164), and at
 * its end either what the table offers (Confirm all, Add) or, while rows are chosen, how many and
 * what can be done to them all (Confirm, Remove). The filter never moves when a row is chosen:
 * only the end of the bar changes, and the count is said to a reader as it changes.
 */
export function TableBar({
  filter,
  chosen = 0,
  bulk,
  actions,
  label,
}: {
  /** The table's filter, drawn at the bar's start. */
  filter?: ReactNode;
  /** How many rows are chosen. */
  chosen?: number;
  /** What can be done to every chosen row: small buttons, shown while any is chosen. */
  bulk?: ReactNode;
  /** What the table offers while no row is chosen: small buttons. */
  actions?: ReactNode;
  /** What the bar is about, said to a reader: "Hosts". */
  label: string;
}) {
  const choosing = chosen > 0;
  return (
    <div data-scope="table" data-part="bar" role="group" aria-label={label}>
      {filter}
      <span data-part="bar-end">
        {/* Said as it changes, and never moves the filter (#2800). */}
        <span data-part="chosen" aria-live="polite">
          {choosing ? `${chosen} selected` : ""}
        </span>
        {choosing
          ? bulk !== undefined && <ButtonRow size="sm">{bulk}</ButtonRow>
          : actions !== undefined && <ButtonRow size="sm">{actions}</ButtonRow>}
      </span>
    </div>
  );
}
