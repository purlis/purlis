// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { chmodSync, renameSync, rmSync, writeFileSync } from "node:fs";

/**
 * **Writes `text` to `path` whole, never in place** (#6283, #6237): to a file beside it, then
 * renamed over it, so whoever reads `path` reads the old file or the new one and never half of
 * either. `mode` is given to the new file before it takes the name.
 *
 * The scenarios' shells are written again by spec after spec while a chat started from the old
 * one still runs, or is still starting. Written in place, a `/bin/sh` reading the script as the
 * write truncated it could run a cut `exec fake-harness --synthetic 4096 \`: no sentinel and no
 * `--interactive`, so the harness wrote its output and left, and the switch to its project
 * waited a minute for a sentinel that was never printed. On Linux the same rewrite failed with
 * `ETXTBSY` when it met the kernel executing the script. A rename leaves the running program its
 * own file and gives the next one the new file.
 */
export function writtenWhole(path: string, text: string, mode?: number): void {
  const beside = `${path}.${process.pid}.${Math.random().toString(36).slice(2)}.new`;
  try {
    writeFileSync(beside, text);
    if (mode !== undefined) chmodSync(beside, mode);
    renameSync(beside, path);
  } catch (error) {
    rmSync(beside, { force: true });
    throw error;
  }
}
