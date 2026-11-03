// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { RowDetails, StartOptions } from "./bindings";

/**
 * **The picker's two answers made one** (#4736): the first answer's rows, each with the details
 * the second answered for it by name — its approval, its sandbox and what it is billed to. A row
 * the second answer does not name keeps the first's, which say none: the start's own gate asks
 * them again whatever the picker drew.
 */
export function withDetails(options: StartOptions, details: readonly RowDetails[]): StartOptions {
  const byName = new Map(details.map((row) => [row.name, row]));
  return {
    ...options,
    profiles: options.profiles.map((row) => {
      const known = byName.get(row.name);
      return known === undefined
        ? row
        : {
            ...row,
            approval: known.approval,
            sandbox: known.sandbox,
            billed_to: known.billed_to,
          };
    }),
  };
}
