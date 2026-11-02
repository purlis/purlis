// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The slow-answer and stall log's most recent entries, for Copy diagnostics** (#4774), or
 * `null` where this build keeps no such log.
 *
 * The seam the log comes in by: #4725 records every answer of 1 s or longer and every stall of
 * 200 ms or longer to a machine-local file. Until its reader lands this answers `null`, and the
 * report leaves the section out rather than failing. Wiring it in is this one body: ask the
 * reader, and answer its entries, one line each, as written.
 */
export async function recentSlowLog(): Promise<readonly string[] | null> {
  return null;
}
