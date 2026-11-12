// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { openSync, readFileSync, readdirSync, readSync, statSync } from "node:fs";
import { join } from "node:path";
import { expect, it } from "vitest";
import { writeShell } from "./harness.js";

it("writing the scenarios' shell again leaves a chat still reading the old one its whole script", () => {
  const shell = writeShell("/nowhere/fake-harness-old");
  // A chat's `/bin/sh` holding the script open, as one still starting does.
  const reading = openSync(shell, "r");

  writeShell("/nowhere/fake-harness-new");

  // It reads, from the start, the script it opened, whole: not one truncated under it.
  const read = Buffer.alloc(64 * 1024);
  const got = read.subarray(0, readSync(reading, read, 0, read.length, 0)).toString();
  expect(got).toContain("/nowhere/fake-harness-old");
  expect(got).toContain("--interactive");
  // And the next chat runs the new one, executable.
  expect(readFileSync(shell, "utf8")).toContain("/nowhere/fake-harness-new");
  expect(statSync(shell).mode & 0o111).not.toBe(0);
});

it("leaves nothing beside the shell once it is written", () => {
  const shell = writeShell("/nowhere/fake-harness");
  const where = join(shell, "..");
  expect(readdirSync(where).filter((name) => name.endsWith(".new"))).toEqual([]);
});
