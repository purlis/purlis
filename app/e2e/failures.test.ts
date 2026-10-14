// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { mkdtempSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { coreLogTail } from "./failures";

/**
 * **A failed scenario test carries the core's last log lines** (`failures.ts`, #1949): the
 * annotation is read without the job's log, so the log's end goes with it.
 */
describe("the core's log at a failure", () => {
  it("is the last lines of the newest of the app's daily logs", () => {
    const dir = mkdtempSync(join(tmpdir(), "logs-"));
    writeFileSync(join(dir, "charter.2026-10-13.log"), "old one\nold two\n");
    utimesSync(join(dir, "charter.2026-10-13.log"), 1, 1);
    const lines = Array.from({ length: 60 }, (_, n) => `line ${n}`);
    writeFileSync(join(dir, "charter.2026-10-14.log"), `${lines.join("\n")}\n`);
    writeFileSync(join(dir, "panics.log"), "not this one\n");

    expect(coreLogTail(dir, 40)).toEqual(lines.slice(-40));
  });

  it("is nothing where the app wrote no log", () => {
    expect(coreLogTail(mkdtempSync(join(tmpdir(), "logs-")), 40)).toEqual([]);
    expect(coreLogTail(join(tmpdir(), "no-such-folder-q389"), 40)).toEqual([]);
  });
});
